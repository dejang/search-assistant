//! The model downloader. `ModelHub` does the listing and fetching, so this owns
//! only local staging and the status the webview renders.

mod files;
mod hub;
mod progress;

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use hf_hub::HFError;
use tauri::{AppHandle, Emitter, Manager, State};

use self::files::{carries_quant, discard_empty, files, holds_a_model, promote, quant_files};
pub use self::hub::{HfHub, ModelHub};
pub use self::progress::{DownloadPhase, DownloadStatus};
use self::progress::{ProgressReporter, Sink};

struct DownloadGuard<'a> {
    downloading: &'a AtomicBool,
}

impl Drop for DownloadGuard<'_> {
    fn drop(&mut self) {
        self.downloading.store(false, Ordering::SeqCst);
    }
}

/// Why the model is not on disk, in the one form the webview can show.
#[derive(Debug, thiserror::Error)]
pub enum ModelStoreError {
    #[error("could not download the model: {0}")]
    Download(#[from] HFError),
    #[error("could not reach the model folder: {0}")]
    Io(#[from] std::io::Error),
    #[error("{repo} holds no {quant} model to download")]
    NothingToFetch { repo: String, quant: String },
    #[error("{repo} finished, but the {quant} model is not on disk")]
    NotOnDisk { repo: String, quant: String },
}

/// The causes are not serializable, and the webview only ever shows the
/// sentence, so that is all that crosses the boundary.
impl serde::Serialize for ModelStoreError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

/// The app's copy of one model, and the only thing that knows where it lives.
pub struct ModelStore<H = HfHub> {
    owner: String,
    name: String,
    quant: String,
    dir: PathBuf,
    hub: H,
    sink: Sink,
    downloading: AtomicBool,
}

impl ModelStore<HfHub> {
    /// Mount the store, and fetch the model in the background unless it is
    /// already on disk. No data directory means no downloader, not a window that
    /// never opens: the commands then refuse cleanly and the row stays hidden.
    pub fn start(app: &AppHandle, owner: &str, name: &str, quant: &str) {
        let Some(store) = Self::mounted(app, owner, name, quant) else {
            return;
        };
        let store = Arc::new(store);
        let needed = !store.is_downloaded();
        app.manage(Arc::clone(&store));
        if needed {
            drop(tauri::async_runtime::spawn(async move {
                let _ = store.download().await;
            }));
        }
    }

    /// A store for `owner/name`, keeping the `quant` files under the app's own
    /// data directory. `None` when the platform gives us no such directory.
    #[must_use]
    fn mounted(app: &AppHandle, owner: &str, name: &str, quant: &str) -> Option<Self> {
        let dir = app.path().app_data_dir().ok()?.join("models");
        let handle = app.clone();
        Some(
            Self::with_hub(owner, name, quant, dir, HfHub::new()).on_status(move |status| {
                let _ = handle.emit("model://download", status);
            }),
        )
    }
}

impl<H: ModelHub> ModelStore<H> {
    /// Build a store for `owner/name` and `quant` backed by `dir` and `hub`.
    #[must_use]
    pub fn with_hub(
        owner: impl Into<String>,
        name: impl Into<String>,
        quant: impl Into<String>,
        dir: PathBuf,
        hub: H,
    ) -> Self {
        let quant = quant.into();
        let downloaded = holds_a_model(&dir, &quant);
        Self {
            owner: owner.into(),
            name: name.into(),
            quant,
            dir,
            hub,
            sink: Sink::new(downloaded),
            downloading: AtomicBool::new(false),
        }
    }

    /// Subscribe `listener` to every status change.
    #[must_use]
    pub fn on_status(mut self, listener: impl Fn(&DownloadStatus) + Send + Sync + 'static) -> Self {
        self.sink = self.sink.with_listener(listener);
        self
    }

    /// Is the model on disk? This is what the app asks at start-up.
    #[must_use]
    fn is_downloaded(&self) -> bool {
        holds_a_model(&self.dir, &self.quant)
    }

    #[must_use]
    pub fn model_files(&self) -> Vec<PathBuf> {
        let token = self.quant.to_lowercase();
        files(&self.dir)
            .into_iter()
            .filter(|path| carries_quant(path, &token))
            .collect()
    }

    /// The status a freshly mounted webview should render.
    #[must_use]
    pub fn status(&self) -> DownloadStatus {
        self.sink.snapshot()
    }

    /// Fetch the model when not already running.
    /// # Errors
    /// Anything the Hub or the disk can refuse; also published as `failed`.
    pub async fn download(&self) -> Result<(), ModelStoreError> {
        if self.downloading.swap(true, Ordering::SeqCst) {
            return Ok(());
        }
        let _guard = DownloadGuard {
            downloading: &self.downloading,
        };
        match self.fetch().await {
            Ok(()) => Ok(()),
            Err(error) => self.failed(error),
        }
    }

    async fn fetch(&self) -> Result<(), ModelStoreError> {
        std::fs::create_dir_all(&self.dir)?;
        self.sink.publish(|status| {
            status.phase = DownloadPhase::Downloading;
            status.downloaded_bytes = 0;
            status.total_bytes = 0;
            status.message = None;
            true
        });
        discard_empty(&self.dir);

        let listing = self.hub.list_files(&self.owner, &self.name).await?;
        let patterns = quant_files(&listing, &self.quant);
        if patterns.is_empty() {
            return Err(ModelStoreError::NothingToFetch {
                repo: self.repo_id(),
                quant: self.quant.clone(),
            });
        }

        let staging = self.dir.join(".staging");
        let _ = std::fs::remove_dir_all(&staging);
        std::fs::create_dir_all(&staging)?;

        let reporter = Arc::new(ProgressReporter::new(self.sink.clone()));
        let fetched = self
            .hub
            .download(&self.owner, &self.name, patterns, staging.clone(), reporter)
            .await;
        if let Err(error) = fetched {
            let _ = std::fs::remove_dir_all(&staging);
            return Err(error);
        }

        promote(&staging, &self.dir)?;
        if !self.is_downloaded() {
            return Err(ModelStoreError::NotOnDisk {
                repo: self.repo_id(),
                quant: self.quant.clone(),
            });
        }
        self.sink.publish(|status| {
            status.phase = DownloadPhase::Present;
            status.message = None;
            true
        });
        Ok(())
    }

    fn failed(&self, error: ModelStoreError) -> Result<(), ModelStoreError> {
        let message = error.to_string();
        self.sink.publish(|status| {
            status.phase = DownloadPhase::Failed;
            status.message = Some(message.clone());
            true
        });
        Err(error)
    }

    fn repo_id(&self) -> String {
        format!("{}/{}", self.owner, self.name)
    }
}

/// Mirrors [`ModelStore::download`], which also publishes the failure.
#[tauri::command]
/// # Errors
/// Mirrors `ModelStore::download`, which also publishes the failure.
pub async fn model_download(store: State<'_, Arc<ModelStore>>) -> Result<(), ModelStoreError> {
    store.download().await
}

/// The download status a freshly mounted webview should render.
#[must_use]
#[tauri::command]
pub fn model_download_status(store: State<'_, Arc<ModelStore>>) -> DownloadStatus {
    store.status()
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, Ordering};

    use super::DownloadGuard;

    #[test]
    fn dropping_the_download_guard_clears_the_in_progress_flag() {
        let downloading = AtomicBool::new(true);
        {
            let _guard = DownloadGuard {
                downloading: &downloading,
            };
            assert!(downloading.load(Ordering::SeqCst));
        }
        assert!(!downloading.load(Ordering::SeqCst));
    }
}
