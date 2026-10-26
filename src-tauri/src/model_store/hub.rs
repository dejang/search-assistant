//! Abstraction over the model repository hub (`hf-hub` in production).

use std::future::Future;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use hf_hub::HFClient;
use hf_hub::progress::ProgressHandler;

use super::ModelStoreError;

/// Source of remote model listings and snapshot downloads.
pub trait ModelHub: Send + Sync {
    /// List every relative file path in `owner/name`.
    fn list_files(
        &self,
        owner: &str,
        name: &str,
    ) -> impl Future<Output = Result<Vec<String>, ModelStoreError>> + Send;

    /// Download the files matching `patterns` from `owner/name` into `dir`.
    fn download(
        &self,
        owner: &str,
        name: &str,
        patterns: Vec<String>,
        dir: PathBuf,
        progress: Arc<dyn ProgressHandler>,
    ) -> impl Future<Output = Result<(), ModelStoreError>> + Send;
}

/// The Hugging Face Hub client with explicit connect and read timeouts.
#[derive(Clone)]
pub struct HfHub {
    endpoint: Option<String>,
    connect_timeout: Duration,
    read_timeout: Duration,
    retries: usize,
}

impl Default for HfHub {
    fn default() -> Self {
        Self::new()
    }
}

impl HfHub {
    /// Production Hub client with 30s connect and 120s read timeouts.
    #[must_use]
    pub fn new() -> Self {
        Self {
            endpoint: None,
            connect_timeout: Duration::from_secs(30),
            read_timeout: Duration::from_secs(120),
            retries: 3,
        }
    }

    /// Hub client pointed at a custom `endpoint` with `timeout` and no retries.
    #[must_use]
    pub fn with_endpoint(endpoint: impl Into<String>, timeout: Duration) -> Self {
        Self {
            endpoint: Some(endpoint.into()),
            connect_timeout: timeout,
            read_timeout: timeout,
            retries: 0,
        }
    }

    fn client(&self) -> Result<HFClient, ModelStoreError> {
        let http = reqwest::Client::builder()
            .connect_timeout(self.connect_timeout)
            .read_timeout(self.read_timeout)
            .build()
            .map_err(|source| ModelStoreError::Download(source.into()))?;
        let mut builder = HFClient::builder()
            .client(http)
            .retry_max_attempts(self.retries);
        if let Some(ref endpoint) = self.endpoint {
            builder = builder.endpoint(endpoint.clone());
        }
        Ok(builder.build()?)
    }
}

impl ModelHub for HfHub {
    async fn list_files(&self, owner: &str, name: &str) -> Result<Vec<String>, ModelStoreError> {
        let client = self.client()?;
        let repo = format!("{owner}/{name}");
        log::debug!(
            "hub: listing {repo} (retries {}, connect {:?}, read {:?})",
            self.retries,
            self.connect_timeout,
            self.read_timeout
        );
        let listing = client.model(owner, name).info().send().await?;
        let files: Vec<String> = listing
            .siblings
            .unwrap_or_default()
            .into_iter()
            .map(|sibling| sibling.rfilename)
            .collect();
        log::debug!("hub: {repo} lists {} files", files.len());
        Ok(files)
    }

    async fn download(
        &self,
        owner: &str,
        name: &str,
        patterns: Vec<String>,
        dir: PathBuf,
        progress: Arc<dyn ProgressHandler>,
    ) -> Result<(), ModelStoreError> {
        let repo = format!("{owner}/{name}");
        let wanted = patterns.len();
        log::info!(
            "hub: downloading {wanted} files of {repo} into {}",
            dir.display()
        );
        let started = std::time::Instant::now();
        let client = self.client()?;
        let fetched = client
            .model(owner, name)
            .snapshot_download()
            .allow_patterns(patterns)
            .local_dir(dir.clone())
            .max_workers(5)
            .progress(progress)
            .send()
            .await;
        match fetched {
            Ok(landed) => {
                log::info!(
                    "hub: {repo} landed at {} in {:?}",
                    landed.display(),
                    started.elapsed()
                );
                Ok(())
            }
            Err(err) => {
                log::error!(
                    "hub: {repo} failed after {:?}: {}",
                    started.elapsed(),
                    crate::clipped(&err.to_string(), 300)
                );
                Err(err.into())
            }
        }
    }
}
