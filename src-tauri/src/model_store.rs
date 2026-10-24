//! The model downloader. `hf-hub` does the listing, the fetching, the resume
//! and the skip-if-present, so this owns only the status the webview renders.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use hf_hub::progress::{DownloadEvent, ProgressEvent, ProgressHandler};
use hf_hub::{HFClient, HFError};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

/// Where the download stands, spelled the way the webview reads it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum DownloadPhase {
    /// Nothing fetched yet, and nothing on disk.
    Absent,
    /// Bytes are on their way.
    Downloading,
    /// The model is on disk.
    Present,
    /// The last attempt gave up; `message` says why.
    Failed,
}

/// What the progress row renders.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadStatus {
    pub phase: DownloadPhase,
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
    pub message: Option<String>,
}

impl DownloadStatus {
    fn absent() -> Self {
        Self {
            phase: DownloadPhase::Absent,
            downloaded_bytes: 0,
            total_bytes: 0,
            message: None,
        }
    }

    fn present() -> Self {
        Self {
            phase: DownloadPhase::Present,
            downloaded_bytes: 0,
            total_bytes: 0,
            message: None,
        }
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

/// The Hub client, with the timeouts `hf-hub` does not set for you.
///
/// `read_timeout` applies to each read and resets when a byte arrives, so a dead
/// connection fails the download instead of hanging the bar forever, while a
/// slow but living several-gigabyte transfer is never cut off.
fn hub_client() -> Result<HFClient, ModelStoreError> {
    let http = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(30))
        .read_timeout(Duration::from_secs(120))
        .build()
        .map_err(|source| ModelStoreError::Download(source.into()))?;
    Ok(HFClient::builder().client(http).build()?)
}

/// The published status, and the webview that renders it.
#[derive(Clone)]
struct Sink {
    status: Arc<Mutex<DownloadStatus>>,
    app: AppHandle,
}

impl Sink {
    /// Move the status, and tell the webview when it actually changed.
    fn publish(&self, update: impl FnOnce(&mut DownloadStatus) -> bool) {
        let mut guard = self.status.lock().unwrap_or_else(PoisonError::into_inner);
        if !update(&mut guard) {
            return;
        }
        let published = guard.clone();
        let _ = self.app.emit("model://download", &published);
    }
}

/// Turns `hf-hub`'s per-file progress into one byte count that never goes back.
struct ProgressReporter {
    sink: Sink,
    total_bytes: AtomicU64,
    /// Whole percent last handed to the webview. `hf-hub` reports on every chunk
    /// of the byte stream, so without this a several-gigabyte file means hundreds
    /// of thousands of IPC emits, and a slow handler slows the transfer itself.
    shown_percent: AtomicU64,
    file_bytes: Mutex<HashMap<String, u64>>,
}

impl ProgressHandler for ProgressReporter {
    fn on_progress(&self, event: &ProgressEvent) {
        let ProgressEvent::Download(download) = event else {
            return;
        };
        match download {
            DownloadEvent::Start { total_bytes, .. } => {
                self.total_bytes.fetch_max(*total_bytes, Ordering::Relaxed);
                self.shown_percent.store(u64::MAX, Ordering::Relaxed);
            }
            DownloadEvent::Progress { files } => {
                let mut seen = self
                    .file_bytes
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner);
                for file in files {
                    seen.insert(file.filename.clone(), file.bytes_completed);
                }
                let downloaded: u64 = seen.values().sum();
                let total = self.total_bytes.load(Ordering::Relaxed);
                if !worth_showing(&self.shown_percent, percent_of(downloaded, total)) {
                    return;
                }
                self.sink
                    .publish(|status| apply_progress(status, downloaded, total));
            }
            _ => {}
        }
    }
}

/// Delete files `hf-hub` would mistake for a finished download. It creates the
/// destination before the first byte, and later skips any file that merely
/// exists, so a transfer that died at zero bytes would be on disk forever.
fn discard_empty(dir: &Path) {
    files(dir)
        .into_iter()
        .filter(|path| path.metadata().is_ok_and(|meta| meta.len() == 0))
        .for_each(|path| {
            let _ = std::fs::remove_file(path);
        });
}

/// Whether this percent is news. `hf-hub` reports on every chunk of the stream,
/// and a handler that emits on each one slows the transfer it is reporting.
fn worth_showing(shown: &AtomicU64, percent: u64) -> bool {
    shown.swap(percent, Ordering::Relaxed) != percent
}

/// Whole percent of a transfer, or zero while the size is still unknown.
fn percent_of(downloaded: u64, total: u64) -> u64 {
    if total == 0 {
        return 0;
    }
    (downloaded * 100) / total
}

/// Fold progress into the published status. A late report cannot pull a
/// finished or failed download back to `downloading`.
fn apply_progress(status: &mut DownloadStatus, downloaded: u64, total: u64) -> bool {
    if matches!(status.phase, DownloadPhase::Present | DownloadPhase::Failed) {
        return false;
    }
    status.phase = DownloadPhase::Downloading;
    let grew = status.downloaded_bytes < downloaded;
    status.downloaded_bytes = status.downloaded_bytes.max(downloaded);
    status.total_bytes = status.total_bytes.max(total);
    grew
}

/// The app's copy of one model, and the only thing that knows where it lives.
pub struct ModelStore {
    owner: String,
    name: String,
    quant: String,
    dir: PathBuf,
    sink: Sink,
    downloading: AtomicBool,
}

impl ModelStore {
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
        let downloaded = holds_a_model(&dir, quant);
        Some(Self {
            owner: owner.into(),
            name: name.into(),
            quant: quant.into(),
            dir,
            sink: Sink {
                status: Arc::new(Mutex::new(if downloaded {
                    DownloadStatus::present()
                } else {
                    DownloadStatus::absent()
                })),
                app: app.clone(),
            },
            downloading: AtomicBool::new(false),
        })
    }

    /// Is the model on disk? This is what the app asks at start-up.
    #[must_use]
    fn is_downloaded(&self) -> bool {
        holds_a_model(&self.dir, &self.quant)
    }

    /// The status a freshly mounted webview should render.
    #[must_use]
    pub fn status(&self) -> DownloadStatus {
        self.sink
            .status
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Fetch the model. The app calls this when it is not downloaded.
    ///
    /// # Errors
    ///
    /// Anything the Hub or the disk can refuse. The same failure is published
    /// as a `failed` status, so a webview that did not await this sees it too.
    pub async fn download(&self) -> Result<(), ModelStoreError> {
        if self.downloading.swap(true, Ordering::SeqCst) {
            return Ok(());
        }
        let outcome = match self.fetch().await {
            Ok(()) => Ok(()),
            Err(error) => self.failed(error),
        };
        self.downloading.store(false, Ordering::SeqCst);
        outcome
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

        let client = hub_client()?;
        let repo = client.model(&self.owner, &self.name);
        let listing = repo.info().send().await?;
        let patterns = quant_files(&listing.siblings.unwrap_or_default(), &self.quant);
        if patterns.is_empty() {
            return Err(ModelStoreError::NothingToFetch {
                repo: self.repo_id(),
                quant: self.quant.clone(),
            });
        }

        let reporter = Arc::new(ProgressReporter {
            sink: self.sink.clone(),
            total_bytes: AtomicU64::new(0),
            shown_percent: AtomicU64::new(u64::MAX),
            file_bytes: Mutex::new(HashMap::new()),
        });
        let fetched = repo
            .snapshot_download()
            .allow_patterns(patterns)
            .local_dir(self.dir.clone())
            .max_workers(5)
            .progress(reporter)
            .send()
            .await;
        match fetched {
            Ok(_) if self.is_downloaded() => {
                self.sink.publish(|status| {
                    status.phase = DownloadPhase::Present;
                    status.message = None;
                    true
                });
                Ok(())
            }
            Ok(_) => Err(ModelStoreError::NotOnDisk {
                repo: self.repo_id(),
                quant: self.quant.clone(),
            }),
            Err(error) => Err(ModelStoreError::Download(error)),
        }
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

/// Whether `dir` holds a model of this quant, at any depth.
fn holds_a_model(dir: &Path, quant: &str) -> bool {
    let token = quant.to_lowercase();
    files(dir).iter().any(|path| {
        path.extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("gguf"))
            && path
                .file_name()
                .is_some_and(|name| name.to_string_lossy().to_lowercase().contains(&token))
            && path.metadata().is_ok_and(|meta| meta.len() > 0)
    })
}

/// The repository paths carrying this quant. Taking every one of them is what
/// makes a split model arrive whole: its shards all match, so all are fetched.
fn quant_files(siblings: &[hf_hub::repository::RepoSibling], quant: &str) -> Vec<String> {
    let token = quant.to_lowercase();
    siblings
        .iter()
        .filter(|sibling| {
            let lower = sibling.rfilename.to_lowercase();
            Path::new(&sibling.rfilename)
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("gguf"))
                && !lower.contains("mmproj")
                && !lower.contains("imatrix")
                && lower.contains(&token)
        })
        .map(|sibling| sibling.rfilename.clone())
        .collect()
}

/// Every file under `dir`, at any depth. Directory links are not followed, so a
/// loop in the folder cannot overflow the stack.
fn files(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let entries: Vec<std::fs::DirEntry> = entries.flatten().collect();
    let nested = |entry: &std::fs::DirEntry| entry.file_type().is_ok_and(|kind| kind.is_dir());
    let mut found: Vec<PathBuf> = entries
        .iter()
        .filter(|entry| !nested(entry))
        .map(std::fs::DirEntry::path)
        .collect();
    found.extend(
        entries
            .iter()
            .filter(|entry| nested(entry))
            .map(std::fs::DirEntry::path)
            .flat_map(|child| files(&child)),
    );
    found
}

/// Fetch the model in the background unless it is already on disk.
///
/// # Errors
///
/// Mirrors [`ModelStore::download`], which also publishes the failure.
#[tauri::command]
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
    use super::*;

    fn sibling(path: &str) -> hf_hub::repository::RepoSibling {
        hf_hub::repository::RepoSibling {
            rfilename: path.into(),
            size: None,
            lfs: None,
        }
    }

    #[test]
    fn every_file_of_the_quant_is_fetched_so_a_split_arrives_whole() {
        let listing = [
            sibling("Qwen3-8B-Q4_K_M.gguf"),
            sibling("Qwen3-8B-Q6_K.gguf"),
            sibling("mmproj-Qwen3-8B-Q4_K_M.gguf"),
            sibling("Qwen3-8B-Q4_K_M-00001-of-00002.gguf"),
            sibling("Qwen3-8B-Q4_K_M-00002-of-00002.gguf"),
            sibling("README.md"),
        ];

        let mut chosen = quant_files(&listing, "Q4_K_M");
        chosen.sort();

        assert_eq!(
            chosen,
            vec![
                "Qwen3-8B-Q4_K_M-00001-of-00002.gguf",
                "Qwen3-8B-Q4_K_M-00002-of-00002.gguf",
                "Qwen3-8B-Q4_K_M.gguf",
            ]
        );
    }

    #[test]
    fn a_folder_holding_the_model_of_this_quant_counts_as_downloaded() {
        let dir = tempfile::TempDir::new().unwrap();
        let nested = dir.path().join("gguf");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(nested.join("Qwen3-8B-Q4_K_M.gguf"), [0; 4]).unwrap();

        assert!(holds_a_model(dir.path(), "Q4_K_M"));
        assert!(!holds_a_model(dir.path(), "Q2_K"));
    }

    #[test]
    fn an_empty_file_is_not_a_downloaded_model() {
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::write(dir.path().join("Qwen3-8B-Q4_K_M.gguf"), []).unwrap();

        assert!(
            !holds_a_model(dir.path(), "Q4_K_M"),
            "a file cut short by an interrupted write"
        );
    }

    #[test]
    fn the_status_serializes_as_the_webview_reads_it() {
        let status = DownloadStatus {
            phase: DownloadPhase::Failed,
            downloaded_bytes: 7,
            total_bytes: 9,
            message: Some("boom".into()),
        };

        let json = serde_json::to_value(status).unwrap();

        assert_eq!(json["phase"], "failed");
        assert_eq!(json["downloadedBytes"], 7);
        assert_eq!(json["totalBytes"], 9);
        assert_eq!(json["message"], "boom");
    }

    #[test]
    fn a_transfer_that_died_at_zero_bytes_is_not_left_to_be_skipped() {
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::write(dir.path().join("Qwen3-8B-Q4_K_M.gguf"), []).unwrap();
        std::fs::write(dir.path().join("config.json"), [0; 8]).unwrap();

        discard_empty(dir.path());

        assert!(
            !dir.path().join("Qwen3-8B-Q4_K_M.gguf").exists(),
            "hf-hub skips any file that exists, so this one is never fetched"
        );
        assert!(dir.path().join("config.json").exists());
    }

    #[test]
    fn the_bar_is_redrawn_once_a_percent_at_most() {
        let total = 5_000_000_000;

        assert_eq!(percent_of(0, 0), 0, "an unknown size is not zero percent");
        assert_eq!(percent_of(1_000, total), 0);
        assert_eq!(percent_of(50_000_000, total), 1);
        assert_eq!(percent_of(total, total), 100);
    }

    #[test]
    fn a_late_report_cannot_pull_a_finished_download_back() {
        let mut done = DownloadStatus::present();

        assert!(
            !apply_progress(&mut done, 100, 100),
            "a stray report would put the bar back to downloading forever"
        );
        assert_eq!(done.phase, DownloadPhase::Present);

        let mut running = DownloadStatus::absent();
        assert!(apply_progress(&mut running, 40, 100));
        assert!(
            !apply_progress(&mut running, 40, 100),
            "the same byte count says nothing new"
        );
        apply_progress(&mut running, 20, 100);
        assert_eq!(running.downloaded_bytes, 40, "progress never moves back");
    }

    #[test]
    fn a_percent_already_shown_is_not_news() {
        let shown = AtomicU64::new(u64::MAX);

        assert!(worth_showing(&shown, 0), "the first report always counts");
        assert!(!worth_showing(&shown, 0), "the next 1000 chunks do not");
        assert!(worth_showing(&shown, 1), "a new percent does");
        assert!(!worth_showing(&shown, 1));
    }
}
