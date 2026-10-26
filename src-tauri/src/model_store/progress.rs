//! Download status and throttled progress reporting.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use hf_hub::progress::{DownloadEvent, ProgressEvent, ProgressHandler};
use serde::Serialize;

type StatusListener = Arc<dyn Fn(&DownloadStatus) + Send + Sync>;

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
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadStatus {
    pub phase: DownloadPhase,
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
    pub message: Option<String>,
}

impl DownloadStatus {
    pub(super) fn absent() -> Self {
        Self {
            phase: DownloadPhase::Absent,
            downloaded_bytes: 0,
            total_bytes: 0,
            message: None,
        }
    }

    pub(super) fn present() -> Self {
        Self {
            phase: DownloadPhase::Present,
            downloaded_bytes: 0,
            total_bytes: 0,
            message: None,
        }
    }
}

/// The published status and the optional subscriber that renders it.
#[derive(Clone)]
pub(super) struct Sink {
    status: Arc<Mutex<DownloadStatus>>,
    listener: Option<StatusListener>,
}

impl Sink {
    pub(super) fn new(downloaded: bool) -> Self {
        let initial = if downloaded {
            DownloadStatus::present()
        } else {
            DownloadStatus::absent()
        };
        Self {
            status: Arc::new(Mutex::new(initial)),
            listener: None,
        }
    }

    pub(super) fn with_listener(
        mut self,
        listener: impl Fn(&DownloadStatus) + Send + Sync + 'static,
    ) -> Self {
        self.listener = Some(Arc::new(listener));
        self
    }

    pub(super) fn snapshot(&self) -> DownloadStatus {
        self.status
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Move the status, and notify the listener when it actually changed.
    pub(super) fn publish(&self, update: impl FnOnce(&mut DownloadStatus) -> bool) {
        let published = {
            let mut guard = self.status.lock().unwrap_or_else(PoisonError::into_inner);
            if !update(&mut guard) {
                return;
            }
            guard.clone()
        };
        if let Some(ref listener) = self.listener {
            listener(&published);
        }
    }
}

/// Turns `hf-hub`'s per-file progress into one byte count that never goes back.
pub(super) struct ProgressReporter {
    sink: Sink,
    total_bytes: AtomicU64,
    /// Whole percent last handed to the listener. `hf-hub` reports on every
    /// chunk of the byte stream, so without this a several-gigabyte file means
    /// hundreds of thousands of emits, and a slow handler slows the transfer.
    shown_percent: AtomicU64,
    file_bytes: Mutex<HashMap<String, u64>>,
}

impl ProgressReporter {
    pub(super) fn new(sink: Sink) -> Self {
        Self {
            sink,
            total_bytes: AtomicU64::new(0),
            shown_percent: AtomicU64::new(u64::MAX),
            file_bytes: Mutex::new(HashMap::new()),
        }
    }
}

impl ProgressHandler for ProgressReporter {
    fn on_progress(&self, event: &ProgressEvent) {
        let ProgressEvent::Download(download) = event else {
            return;
        };
        match download {
            DownloadEvent::Start { total_bytes, .. } => {
                log::info!("model: a {total_bytes} byte transfer started");
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
    downloaded.saturating_mul(100) / total
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

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicU64;

    use super::{DownloadPhase, DownloadStatus, apply_progress, percent_of, worth_showing};

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
