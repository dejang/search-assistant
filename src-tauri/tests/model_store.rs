#[cfg(test)]
mod integration {
    use std::future::IntoFuture;
    use std::net::SocketAddr;
    use std::path::PathBuf;
    use std::pin::Pin;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};
    use std::task::{Context, Poll};
    use std::time::Duration;

    use axum::Router;
    use axum::body::{Body, Bytes};
    use axum::extract::State;
    use axum::http::{HeaderMap, HeaderValue, StatusCode};
    use axum::response::{IntoResponse, Json};
    use axum::routing::get;
    use hf_hub::progress::{DownloadEvent, FileProgress, FileStatus, ProgressHandler};
    use research_buddy_lib::model_store::{
        DownloadPhase, DownloadStatus, HfHub, ModelHub, ModelStore, ModelStoreError,
    };
    use serde_json::{Value, json};

    struct MockHub {
        model_file: String,
        fail_next: AtomicBool,
    }

    impl MockHub {
        fn new(fail_next: bool) -> Self {
            Self {
                model_file: "Qwen3-8B-Q4_K_M.gguf".to_string(),
                fail_next: AtomicBool::new(fail_next),
            }
        }

        fn simulate_download(
            &self,
            patterns: &[String],
            dir: &std::path::Path,
            progress: &dyn ProgressHandler,
        ) -> Result<(), ModelStoreError> {
            assert_eq!(patterns, [self.model_file.as_str()]);
            let total_bytes = 200;
            progress.on_progress(
                &DownloadEvent::Start {
                    total_files: 1,
                    total_bytes,
                }
                .into(),
            );

            let target = dir.join(&self.model_file);
            std::fs::write(&target, vec![b'a'; 100])?;
            progress.on_progress(
                &DownloadEvent::Progress {
                    files: vec![FileProgress {
                        filename: self.model_file.clone(),
                        bytes_completed: 100,
                        total_bytes,
                        status: FileStatus::InProgress,
                    }],
                }
                .into(),
            );

            if self.fail_next.swap(false, Ordering::SeqCst) {
                return Err(ModelStoreError::Io(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "stream timed out after 100 bytes",
                )));
            }

            std::fs::write(&target, vec![b'a'; 200])?;
            progress.on_progress(
                &DownloadEvent::Progress {
                    files: vec![FileProgress {
                        filename: self.model_file.clone(),
                        bytes_completed: 200,
                        total_bytes,
                        status: FileStatus::Complete,
                    }],
                }
                .into(),
            );
            progress.on_progress(&DownloadEvent::Complete.into());
            Ok(())
        }
    }

    impl ModelHub for MockHub {
        fn list_files(
            &self,
            _owner: &str,
            _name: &str,
        ) -> impl Future<Output = Result<Vec<String>, ModelStoreError>> + Send {
            std::future::ready(Ok(vec![self.model_file.clone(), "README.md".to_string()]))
        }

        fn download(
            &self,
            _owner: &str,
            _name: &str,
            patterns: Vec<String>,
            dir: PathBuf,
            progress: Arc<dyn ProgressHandler>,
        ) -> impl Future<Output = Result<(), ModelStoreError>> + Send {
            std::future::ready(self.simulate_download(&patterns, &dir, progress.as_ref()))
        }
    }

    #[tokio::test]
    async fn happy_path_reports_progress_and_marks_model_present() {
        let dir = tempfile::TempDir::new().unwrap();
        let seen: Arc<Mutex<Vec<DownloadStatus>>> = Arc::new(Mutex::new(Vec::new()));
        let recorded = Arc::clone(&seen);

        let store = ModelStore::with_hub(
            "Qwen",
            "Qwen3-8B-GGUF",
            "Q4_K_M",
            dir.path().to_path_buf(),
            MockHub::new(false),
        )
        .on_status(move |status| {
            recorded.lock().unwrap().push(status.clone());
        });

        assert_eq!(store.status().phase, DownloadPhase::Absent);
        assert_eq!(store.model_files().len(), 0);

        store.download().await.expect("download to succeed");

        assert_eq!(store.status().phase, DownloadPhase::Present);
        assert_eq!(store.model_files().len(), 1);

        let events = seen.lock().unwrap();
        assert!(
            events.iter().any(|s| {
                s.phase == DownloadPhase::Downloading
                    && s.downloaded_bytes == 100
                    && s.total_bytes == 200
            }),
            "expected mid-stream progress event, got: {events:?}"
        );
        assert_eq!(events.last().map(|s| s.phase), Some(DownloadPhase::Present));
    }

    #[tokio::test]
    async fn mid_download_failure_cleans_up_partial_file_and_recovers_on_retry() {
        let dir = tempfile::TempDir::new().unwrap();
        let seen: Arc<Mutex<Vec<DownloadStatus>>> = Arc::new(Mutex::new(Vec::new()));
        let recorded = Arc::clone(&seen);

        let store = ModelStore::with_hub(
            "Qwen",
            "Qwen3-8B-GGUF",
            "Q4_K_M",
            dir.path().to_path_buf(),
            MockHub::new(true),
        )
        .on_status(move |status| {
            recorded.lock().unwrap().push(status.clone());
        });

        let err = store
            .download()
            .await
            .expect_err("first attempt must fail mid-stream");
        assert!(err.to_string().contains("timed out"));

        let failed_status = store.status();
        assert_eq!(failed_status.phase, DownloadPhase::Failed);
        assert!(
            failed_status
                .message
                .as_deref()
                .is_some_and(|msg| msg.contains("timed out"))
        );
        assert!(
            store.model_files().is_empty(),
            "partial file from failed transfer must not remain in model directory"
        );

        store
            .download()
            .await
            .expect("retry after mid-way failure must succeed");
        assert_eq!(store.status().phase, DownloadPhase::Present);
        assert_eq!(store.model_files().len(), 1);
    }

    #[derive(Clone)]
    struct HubServerState {
        commit_sha: &'static str,
        model_file: &'static str,
        stall_mid_stream: Arc<AtomicBool>,
    }

    async fn repo_info(State(state): State<HubServerState>) -> Json<Value> {
        Json(json!({
            "id": "Qwen/Qwen3-8B-GGUF",
            "sha": state.commit_sha,
            "siblings": [
                { "rfilename": state.model_file },
                { "rfilename": "README.md" }
            ]
        }))
    }

    async fn repo_tree(State(state): State<HubServerState>) -> Json<Value> {
        Json(json!([
            {
                "type": "file",
                "oid": state.commit_sha,
                "path": state.model_file,
                "size": 400
            },
            {
                "type": "file",
                "oid": state.commit_sha,
                "path": "README.md",
                "size": 12
            }
        ]))
    }

    fn file_headers(commit_sha: &'static str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert("etag", HeaderValue::from_static("\"etag-q4\""));
        headers.insert("x-repo-commit", HeaderValue::from_static(commit_sha));
        headers.insert("content-length", HeaderValue::from_static("400"));
        headers
    }

    async fn head_file(State(state): State<HubServerState>) -> impl IntoResponse {
        (StatusCode::OK, file_headers(state.commit_sha))
    }

    async fn resolve_file(State(state): State<HubServerState>) -> impl IntoResponse {
        let stall = state.stall_mid_stream.swap(false, Ordering::SeqCst);
        let stream = ChunkedModelStream::new(stall);
        (
            StatusCode::OK,
            file_headers(state.commit_sha),
            Body::from_stream(stream),
        )
    }

    struct ChunkedModelStream {
        step: u8,
        stall: bool,
        delay: Pin<Box<tokio::time::Sleep>>,
    }

    impl ChunkedModelStream {
        fn new(stall: bool) -> Self {
            let wait = if stall {
                Duration::from_millis(600)
            } else {
                Duration::from_millis(10)
            };
            Self {
                step: 0,
                stall,
                delay: Box::pin(tokio::time::sleep(wait)),
            }
        }
    }

    impl futures_core::Stream for ChunkedModelStream {
        type Item = Result<Bytes, std::io::Error>;

        fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
            match self.step {
                0 => {
                    self.step = 1;
                    Poll::Ready(Some(Ok(Bytes::from(vec![b'x'; 200]))))
                }
                1 => match self.delay.as_mut().poll(cx) {
                    Poll::Pending => Poll::Pending,
                    Poll::Ready(()) if self.stall => {
                        self.step = 2;
                        Poll::Ready(Some(Err(std::io::Error::new(
                            std::io::ErrorKind::TimedOut,
                            "stream stalled",
                        ))))
                    }
                    Poll::Ready(()) => {
                        self.step = 2;
                        Poll::Ready(Some(Ok(Bytes::from(vec![b'y'; 200]))))
                    }
                },
                _ => Poll::Ready(None),
            }
        }
    }

    async fn spawn_axum_hub(stall_first: bool) -> String {
        let state = HubServerState {
            commit_sha: "0123456789abcdef0123456789abcdef01234567",
            model_file: "Qwen3-8B-Q4_K_M.gguf",
            stall_mid_stream: Arc::new(AtomicBool::new(stall_first)),
        };
        let app = Router::new()
            .route("/api/models/{owner}/{name}", get(repo_info))
            .route("/api/models/{owner}/{name}/revision/{rev}", get(repo_info))
            .route("/api/models/{owner}/{name}/tree/{rev}", get(repo_tree))
            .route(
                "/{owner}/{name}/resolve/{rev}/{file}",
                get(resolve_file).head(head_file),
            )
            .with_state(state);

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr: SocketAddr = listener.local_addr().unwrap();
        drop(tokio::task::spawn(axum::serve(listener, app).into_future()));
        format!("http://{addr}")
    }

    #[tokio::test]
    async fn axum_hf_hub_reports_timeout_mid_stream_and_succeeds_on_retry() {
        let endpoint = spawn_axum_hub(true).await;
        let dir = tempfile::TempDir::new().unwrap();
        let seen: Arc<Mutex<Vec<DownloadStatus>>> = Arc::new(Mutex::new(Vec::new()));
        let recorded = Arc::clone(&seen);

        let hub = HfHub::with_endpoint(endpoint, Duration::from_millis(200));
        let store = ModelStore::with_hub(
            "Qwen",
            "Qwen3-8B-GGUF",
            "Q4_K_M",
            dir.path().to_path_buf(),
            hub,
        )
        .on_status(move |status| {
            recorded.lock().unwrap().push(status.clone());
        });

        let err = store
            .download()
            .await
            .expect_err("stalled HTTP stream must time out");
        assert!(matches!(err, ModelStoreError::Download(_)));
        assert_eq!(store.status().phase, DownloadPhase::Failed);
        assert!(store.status().message.is_some());
        assert!(
            store.model_files().is_empty(),
            "timed-out HTTP stream must not leave a corrupt model on disk"
        );

        {
            let events = seen.lock().unwrap();
            assert!(
                events
                    .iter()
                    .any(|s| s.phase == DownloadPhase::Downloading && s.downloaded_bytes == 200),
                "progress for first chunk must be reported before timeout: {events:?}"
            );
        }

        store
            .download()
            .await
            .expect("second download over HTTP must succeed");
        assert_eq!(store.status().phase, DownloadPhase::Present);
        let files = store.model_files();
        assert_eq!(files.len(), 1);
        assert_eq!(std::fs::metadata(&files[0]).unwrap().len(), 400);
    }
}
