#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};
    use std::time::Duration;

    use research_buddy_lib::agent::{Agent, AgentError};
    use research_buddy_lib::inference_engine::{
        Engine, EngineError, EngineStatus, InferenceEngine, LocalEngine, Sampling, ServerConfig,
    };
    use tempfile::TempDir;

    fn write_fake_server(dir: &Path) -> PathBuf {
        let script = r#"#!/usr/bin/env python3
import http.server
import json
import pathlib
import sys
import threading
import time

args = sys.argv[1:]
port = int(args[args.index("--port") + 1])
idle_seconds = (
    int(args[args.index("--sleep-idle-seconds") + 1])
    if "--sleep-idle-seconds" in args
    else -1
)
model_path = pathlib.Path(args[args.index("--model") + 1])
mode = model_path.read_text().strip()

if mode == "exit_on_startup":
    sys.exit(1)

state = {"last_active": time.monotonic(), "sleeping": False}
lock = threading.Lock()

def monitor_idle():
    while True:
        time.sleep(0.1)
        with lock:
            if (
                idle_seconds > 0
                and not state["sleeping"]
                and (time.monotonic() - state["last_active"]) >= idle_seconds
            ):
                state["sleeping"] = True
                print("entering sleeping state", flush=True)

threading.Thread(target=monitor_idle, daemon=True).start()

class Handler(http.server.BaseHTTPRequestHandler):
    def log_message(self, format, *args):
        pass

    def do_GET(self):
        if self.path == "/health":
            self.send_response(200)
            self.end_headers()
            self.wfile.write(b'{"status":"ok"}')
        else:
            self.send_response(404)
            self.end_headers()

    def do_POST(self):
        with lock:
            if state["sleeping"]:
                state["sleeping"] = False
                print("waking from sleeping state", flush=True)
            state["last_active"] = time.monotonic()
        length = int(self.headers.get("Content-Length", "0"))
        if length:
            self.rfile.read(length)
        if mode == "crash_inference":
            body = b'{"error":{"message":"inference crashed","type":"server_error"}}'
            self.send_response(500)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
            return
        reply = {
            "id": "chatcmpl-test",
            "object": "chat.completion",
            "created": 1,
            "model": "qwen3-8b",
            "choices": [{
                "index": 0,
                "message": {"role": "assistant", "content": "Paris"},
                "finish_reason": "stop",
            }],
            "usage": {"prompt_tokens": 4, "completion_tokens": 1, "total_tokens": 5},
        }
        body = json.dumps(reply).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

http.server.HTTPServer(("127.0.0.1", port), Handler).serve_forever()
"#;
        let binary = dir.join("fake-llama-server");
        std::fs::write(&binary, script).expect("write fake server script");
        std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o755))
            .expect("chmod fake server script");
        binary
    }

    fn write_model(dir: &Path, mode: &str) -> PathBuf {
        let model = dir.join(format!("{mode}.gguf"));
        std::fs::write(&model, mode).expect("write model fixture");
        model
    }

    fn config_with(binary: PathBuf) -> ServerConfig {
        ServerConfig {
            binary,
            slots: 1,
            slot_context: 512,
            cache_mib: 0,
            idle_seconds: 120,
            sampling: Sampling {
                temperature: 0.7,
                top_p: 0.8,
                top_k: 20,
                min_p: 0.0,
            },
            log: None,
        }
    }

    async fn spawn_local(
        config: &ServerConfig,
        model: &Path,
    ) -> Result<InferenceEngine, EngineError> {
        let local = LocalEngine::spawn(config, model, Duration::from_secs(5)).await?;
        Ok(InferenceEngine::Local(local))
    }

    #[tokio::test]
    async fn engine_starts_and_runs_inference() {
        let temp = TempDir::new().expect("temp dir");
        let config = config_with(write_fake_server(temp.path()));
        let model = write_model(temp.path(), "ok");
        let engine = spawn_local(&config, &model).await.expect("engine to start");

        assert_eq!(engine.status(), EngineStatus::Online);
        assert!(engine.is_online());

        let agent = Agent::new(engine.endpoint(), "qwen3-8b");
        let answer = agent
            .ask("Capital of France?")
            .await
            .expect("inference turn to succeed");

        assert_eq!(answer, "Paris");
    }

    #[tokio::test]
    async fn engine_starts_but_inference_crashes_returns_error() {
        let temp = TempDir::new().expect("temp dir");
        let config = config_with(write_fake_server(temp.path()));
        let model = write_model(temp.path(), "crash_inference");
        let engine = spawn_local(&config, &model).await.expect("engine to start");

        assert!(engine.is_online());

        let agent = Agent::new(engine.endpoint(), "qwen3-8b");
        let result = agent.ask("Capital of France?").await;

        assert!(
            matches!(result, Err(AgentError::Completion { .. })),
            "expected inference failure to return Completion error, got {result:?}"
        );
    }

    #[tokio::test]
    async fn engine_cannot_start_returns_init_error() {
        let temp = TempDir::new().expect("temp dir");
        let binary = write_fake_server(temp.path());
        let model = write_model(temp.path(), "exit_on_startup");

        let missing_binary = config_with(temp.path().join("missing-llama-server"));
        let missing_result = spawn_local(&missing_binary, &model).await;
        assert!(matches!(missing_result, Err(EngineError::Init { .. })));

        let crashing_config = config_with(binary);
        let crashing_result = spawn_local(&crashing_config, &model).await;
        assert!(matches!(crashing_result, Err(EngineError::Init { .. })));
    }

    #[tokio::test]
    async fn engine_crashes_after_inference_returns_error_on_next_call() {
        let temp = TempDir::new().expect("temp dir");
        let config = config_with(write_fake_server(temp.path()));
        let model = write_model(temp.path(), "ok");
        let engine = spawn_local(&config, &model).await.expect("engine to start");

        let agent = Agent::new(engine.endpoint(), "qwen3-8b");
        let first = agent
            .ask("Capital of France?")
            .await
            .expect("first inference turn to succeed");
        assert_eq!(first, "Paris");

        engine.shutdown();

        assert_eq!(engine.status(), EngineStatus::Offline);
        assert!(!engine.is_online());

        let second = agent.ask("Capital of France?").await;
        assert!(
            matches!(second, Err(AgentError::Completion { .. })),
            "expected call against dead engine to fail, got {second:?}"
        );
    }

    #[tokio::test]
    async fn engine_sleeps_when_idle_and_wakes_on_next_inference() {
        let temp = TempDir::new().expect("temp dir");
        let log_path = temp.path().join("llama-server.log");
        let mut config = config_with(write_fake_server(temp.path()));
        config.idle_seconds = 1;
        config.log = Some(log_path.clone());
        let model = write_model(temp.path(), "ok");
        let engine = spawn_local(&config, &model).await.expect("engine to start");

        let agent = Agent::new(engine.endpoint(), "qwen3-8b");
        let first = agent.ask("Capital of France?").await.expect("first turn");
        assert_eq!(first, "Paris");

        tokio::time::sleep(Duration::from_millis(1300)).await;
        assert!(engine.is_online(), "sleeping engine stays online");
        let log_after_idle = std::fs::read_to_string(&log_path).expect("read server log");
        assert!(
            log_after_idle.contains("entering sleeping state"),
            "expected server to enter sleeping state after idle_seconds, got {log_after_idle:?}"
        );

        let second = agent
            .ask("Capital of France?")
            .await
            .expect("wake up on next turn");
        assert_eq!(second, "Paris");
        let log_after_wake = std::fs::read_to_string(&log_path).expect("read server log");
        assert!(
            log_after_wake.contains("waking from sleeping state"),
            "expected server to wake on next turn, got {log_after_wake:?}"
        );
    }
}
