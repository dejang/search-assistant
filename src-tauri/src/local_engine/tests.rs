use super::bridge::EngineClient;
use super::server::Server;
use super::{EngineError, EnginePhase, EngineStatus, LocalEngine, Sampling, ServerConfig};
use rig_core::http_client::{Bytes, StatusCode};
use serde_json::{Value, json};

#[cfg(test)]
mod contract {
    use super::*;

    async fn wire_reply(outcome: Result<(StatusCode, Bytes), EngineError>) -> (StatusCode, Value) {
        let response = EngineClient::reply::<Bytes>(outcome);
        let status = response.status();
        let bytes = response
            .into_body()
            .await
            .expect("the engine's reply to be readable");
        (
            status,
            serde_json::from_slice(&bytes).expect("a json reply"),
        )
    }

    #[test]
    fn each_engine_failure_names_the_status_the_agent_reports() {
        let refused = EngineError::Request {
            reason: "too long".into(),
        };
        let died = EngineError::Generation {
            reason: "metal said no".into(),
        };
        let absent = EngineError::Load {
            reason: "no file".into(),
        };

        assert_eq!(refused.status(), StatusCode::BAD_REQUEST);
        assert_eq!(died.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(absent.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(
            EngineError::Stopped.status(),
            StatusCode::SERVICE_UNAVAILABLE
        );
        assert_eq!(
            EngineError::Unavailable.status(),
            StatusCode::SERVICE_UNAVAILABLE
        );
        assert_eq!(
            EngineError::TimedOut {
                timeout: std::time::Duration::from_secs(120),
            }
            .status(),
            StatusCode::GATEWAY_TIMEOUT
        );
    }

    fn answered(status: StatusCode, document: &Value) -> (StatusCode, Bytes) {
        (
            status,
            Bytes::from(serde_json::to_vec(document).expect("json")),
        )
    }

    #[tokio::test]
    async fn the_servers_answer_reaches_the_wire_as_it_was_sent() {
        let completion = json!({ "choices": [{ "message": { "content": "Paris" } }] });

        let (status, body) = wire_reply(Ok(answered(StatusCode::OK, &completion))).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, completion);
    }

    #[tokio::test]
    async fn a_server_refusal_keeps_its_own_status() {
        let refusal = json!({ "error": { "message": "the request exceeds the context size" } });

        let (status, body) = wire_reply(Ok(answered(StatusCode::BAD_REQUEST, &refusal))).await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body, refusal);
    }

    #[tokio::test]
    async fn an_engine_failure_reaches_the_wire_as_an_openai_error() {
        let (status, body) = wire_reply(Err(EngineError::Request {
            reason: "the local model answers no /v1/embeddings".into(),
        }))
        .await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"]["type"], "invalid_request_error");
        assert!(
            body["error"]["message"]
                .as_str()
                .is_some_and(|text| text.contains("/v1/embeddings")),
            "the sentence the webview shows carries the cause"
        );
    }

    #[tokio::test]
    async fn a_dead_engine_is_retryable_and_a_broken_answer_is_not() {
        let (unavailable, _) = wire_reply(Err(EngineError::Unavailable)).await;
        let (load, _) = wire_reply(Err(EngineError::Load {
            reason: "no chat template".into(),
        }))
        .await;
        let (generation, _) = wire_reply(Err(EngineError::Generation {
            reason: "halted mid-sentence".into(),
        }))
        .await;

        assert_eq!(unavailable, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(load, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(generation, StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn only_a_quiet_and_stale_idle_timer_frees_the_weights() {
        assert!(LocalEngine::quiescent(0, 7, 7));
        assert!(!LocalEngine::quiescent(0, 7, 8));
        assert!(!LocalEngine::quiescent(1, 7, 7));
        assert!(!LocalEngine::quiescent(3, 7, 8));
    }

    #[test]
    fn the_status_serializes_as_the_webview_reads_it() {
        let status = EngineStatus {
            phase: EnginePhase::Failed,
            message: Some("the model could not be loaded".into()),
        };

        let json = serde_json::to_value(status).expect("a serializable status");

        assert_eq!(json["phase"], "failed");
        assert_eq!(json["message"], "the model could not be loaded");
    }

    #[test]
    fn a_status_that_says_the_same_thing_is_not_news() {
        assert_eq!(
            EngineStatus::of(EnginePhase::Ready),
            EngineStatus::of(EnginePhase::Ready)
        );
        assert_ne!(
            EngineStatus::of(EnginePhase::Ready),
            EngineStatus {
                phase: EnginePhase::Ready,
                message: Some("later".into()),
            }
        );
    }
}

#[cfg(test)]
mod process {
    use super::*;
    use std::path::{Path, PathBuf};
    use std::time::{Duration, Instant};

    fn config(binary: &str) -> ServerConfig {
        ServerConfig {
            binary: PathBuf::from(binary),
            slots: 1,
            slot_context: 512,
            cache_mib: 0,
            sampling: Sampling {
                temperature: 0.7,
                top_p: 0.8,
                top_k: 20,
                min_p: 0.0,
            },
            log: None,
        }
    }

    #[tokio::test]
    async fn a_server_that_exits_never_reports_ready() {
        let server = Server::spawn(&config("/usr/bin/false"), Path::new("model.gguf"))
            .expect("a process to start");

        let ready = server.ready().await;

        assert!(
            matches!(ready, Err(EngineError::Load { .. })),
            "an exited server is a failed load"
        );
    }

    #[tokio::test]
    async fn a_request_to_an_exited_server_reports_it_stopped() {
        let server = Server::spawn(&config("/usr/bin/false"), Path::new("model.gguf"))
            .expect("a process to start");
        let _ = server.ready().await;

        let answer = server.chat(Bytes::from_static(b"{}")).await;

        assert!(matches!(answer, Err(EngineError::Stopped)));
    }

    #[test]
    fn a_missing_binary_is_a_failed_load() {
        let spawned = Server::spawn(&config("/nowhere/llama-server"), Path::new("model.gguf"));

        assert!(matches!(spawned, Err(EngineError::Load { .. })));
    }

    #[tokio::test]
    async fn stopping_ends_a_server_that_would_run_forever() {
        let server = Server::spawn(&config("/usr/bin/yes"), Path::new("model.gguf"))
            .expect("a process to start");

        server.stop();

        assert!(
            matches!(
                server.chat(Bytes::from_static(b"{}")).await,
                Err(EngineError::Stopped)
            ),
            "a stopped server answers nothing"
        );
    }

    fn child(parent: u32, name: &str) -> Option<String> {
        let out = std::process::Command::new("pgrep")
            .args(["-P", &parent.to_string(), "-x", name])
            .output()
            .expect("pgrep to run");
        String::from_utf8_lossy(&out.stdout)
            .split_whitespace()
            .next()
            .map(str::to_owned)
    }

    fn running(pattern: &str) -> Option<String> {
        let out = std::process::Command::new("pgrep")
            .args(["-f", pattern])
            .output()
            .expect("pgrep to run");
        String::from_utf8_lossy(&out.stdout)
            .split_whitespace()
            .next()
            .map(str::to_owned)
    }

    #[test]
    fn stopping_right_after_starting_leaves_nothing_running() {
        let model = format!("stop-test-{}.gguf", std::process::id());
        for step in 0..40 {
            let server = Server::spawn(&config("/usr/bin/yes"), Path::new(&model))
                .expect("a process to start");
            std::thread::sleep(Duration::from_micros(step * 50));
            server.stop();
        }

        assert!(running(&model).is_none(), "a server outlived stop()");
    }

    fn alive(pid: &str) -> bool {
        std::process::Command::new("kill")
            .args(["-0", pid])
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok_and(|status| status.success())
    }

    #[test]
    fn the_server_dies_with_the_app_that_started_it() {
        let mut watcher = Server::watched(Path::new("/bin/sleep"))
            .arg("30")
            .spawn()
            .expect("a process to start");
        let started = Instant::now();
        let server = loop {
            if let Some(pid) = child(watcher.id(), "sleep") {
                break pid;
            }
            assert!(started.elapsed() < Duration::from_secs(5), "no server ran");
            std::thread::sleep(Duration::from_millis(20));
        };

        drop(watcher.stdin.take());
        let closed = Instant::now();
        while watcher.try_wait().expect("a status").is_none() {
            assert!(
                closed.elapsed() < Duration::from_secs(5),
                "the server outlived the app's end of the pipe"
            );
            std::thread::sleep(Duration::from_millis(20));
        }

        assert!(!alive(&server), "the server ends with the app");
    }
}

#[cfg(test)]
mod real_model {
    use super::*;
    use std::path::PathBuf;
    use std::time::Instant;

    fn path(variable: &str, fallback: PathBuf) -> PathBuf {
        std::env::var_os(variable).map_or(fallback, PathBuf::from)
    }

    fn served() -> ServerConfig {
        ServerConfig {
            binary: path(
                "RESEARCH_BUDDY_LLAMA_SERVER",
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("binaries/llama-server-aarch64-apple-darwin"),
            ),
            slots: 4,
            slot_context: 16_384,
            cache_mib: 2_048,
            sampling: Sampling {
                temperature: 0.7,
                top_p: 0.8,
                top_k: 20,
                min_p: 0.0,
            },
            log: None,
        }
    }

    fn gguf() -> PathBuf {
        path(
            "RESEARCH_BUDDY_GGUF",
            PathBuf::from(std::env::var_os("HOME").expect("HOME to find the app's model"))
                .join("Library/Application Support/com.dejan.research-buddy/models")
                .join("Qwen3-8B-Q4_K_M.gguf"),
        )
    }

    async fn story(server: &Server, batch: &str, subject: &str) -> u64 {
        let body = json!({
            "model": "qwen3-8b",
            "messages": [
                { "role": "system", "content": format!("Batch {batch}. You write short fiction.") },
                { "role": "user", "content": format!("Write a 250 word story about {subject}.") },
            ],
        });
        let (status, bytes) = server
            .chat(Bytes::from(serde_json::to_vec(&body).expect("json")))
            .await
            .expect("an answer");
        assert_eq!(
            status,
            StatusCode::OK,
            "{}",
            String::from_utf8_lossy(&bytes)
        );
        let answer: Value = serde_json::from_slice(&bytes).expect("a json answer");
        answer["usage"]["completion_tokens"]
            .as_u64()
            .unwrap_or_default()
    }

    fn footprint(name: &str) -> String {
        let out = std::process::Command::new("footprint")
            .arg(name)
            .output()
            .expect("footprint to run");
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .find(|line| line.contains("Footprint:"))
            .unwrap_or_default()
            .trim()
            .to_string()
    }

    #[tokio::test]
    #[ignore = "starts llama-server on the real model; run with --ignored --nocapture"]
    async fn requests_of_different_lengths_decode_together() {
        let config = served();
        let server = Server::spawn(&config, &gguf()).expect("llama-server to start");
        server
            .ready()
            .await
            .expect("llama-server to load the model");
        story(&server, "warm", "a crow").await;
        let subjects = [
            "a heron",
            "an otter who builds a dam",
            "a fox who learns to swim across a cold river",
            "two badgers who argue all winter about where to dig a new sett",
        ];

        let started = Instant::now();
        let mut serial_tokens = Vec::new();
        for subject in subjects {
            serial_tokens.push(story(&server, "one by one", subject).await);
        }
        let serial = started.elapsed();

        let started = Instant::now();
        let together_tokens = tokio::join!(
            story(&server, "together", subjects[0]),
            story(&server, "together", subjects[1]),
            story(&server, "together", subjects[2]),
            story(&server, "together", subjects[3]),
        );
        let together = started.elapsed();

        let name = config
            .binary
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        println!(
            "one by one {serial:?} {serial_tokens:?}, together {together:?} {together_tokens:?}, {}",
            footprint(&name)
        );
        assert!(
            together * 4 < serial * 3,
            "together {together:?} is no faster than one by one {serial:?}"
        );
    }
}
