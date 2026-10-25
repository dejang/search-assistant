use std::fs::File;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use rig_core::http_client::{Bytes, StatusCode};

use super::EngineError;

#[derive(Debug, Clone, Copy)]
pub struct Sampling {
    pub temperature: f32,
    pub top_p: f32,
    pub top_k: u32,
    pub min_p: f32,
}

#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub binary: PathBuf,
    pub slots: usize,
    pub slot_context: usize,
    pub cache_mib: usize,
    pub sampling: Sampling,
    pub log: Option<PathBuf>,
}

pub(super) struct Server {
    child: Mutex<Child>,
    base: String,
    http: reqwest::Client,
}

impl Server {
    pub(super) fn spawn(config: &ServerConfig, model: &Path) -> Result<Self, EngineError> {
        let failed = |error: std::io::Error| EngineError::Load {
            reason: format!("llama-server did not start: {error}"),
        };
        if !config.binary.is_file() {
            return Err(EngineError::Load {
                reason: format!("llama-server is missing at {}", config.binary.display()),
            });
        }
        let port = TcpListener::bind(("127.0.0.1", 0))
            .and_then(|listener| listener.local_addr())
            .map_err(failed)?
            .port();
        let (stdout, stderr) = match &config.log {
            Some(path) => {
                let file = File::create(path).map_err(failed)?;
                (
                    Stdio::from(file.try_clone().map_err(failed)?),
                    Stdio::from(file),
                )
            }
            None => (Stdio::null(), Stdio::null()),
        };
        let http = reqwest::Client::builder()
            .no_proxy()
            .build()
            .map_err(|error| EngineError::Load {
                reason: format!("no client for llama-server: {error}"),
            })?;
        let mut command = Self::watched(&config.binary);
        for (name, _) in std::env::vars_os() {
            if name.to_string_lossy().starts_with("LLAMA_") {
                command.env_remove(name);
            }
        }
        let sampling = config.sampling;
        let child = command
            .arg("--model")
            .arg(model)
            .args(["--host", "127.0.0.1", "--port", &port.to_string()])
            .args(["--parallel", &config.slots.to_string()])
            .args(["--kv-unified-per-slot", &config.slot_context.to_string()])
            .args(["--cache-ram", &config.cache_mib.to_string()])
            .args(["--gpu-layers", "all", "--reasoning", "off", "--no-webui"])
            .args(["--temp", &sampling.temperature.to_string()])
            .args(["--top-p", &sampling.top_p.to_string()])
            .args(["--top-k", &sampling.top_k.to_string()])
            .args(["--min-p", &sampling.min_p.to_string()])
            .stdout(stdout)
            .stderr(stderr)
            .spawn()
            .map_err(failed)?;
        Ok(Self {
            child: Mutex::new(child),
            base: format!("http://127.0.0.1:{port}"),
            http,
        })
    }

    pub(super) fn watched(binary: &Path) -> Command {
        let mut command = Command::new("/bin/sh");
        command
            .arg("-c")
            .arg(
                r#"exec 3<&0
                "$@" </dev/null &
                server=$!
                (read -r line <&3; kill -9 "$server" 2>/dev/null) &
                watcher=$!
                exec 3<&-
                wait "$server"
                status=$?
                kill "$watcher" 2>/dev/null
                exit "$status""#,
            )
            .arg("llama-server")
            .arg(binary)
            .stdin(Stdio::piped());
        command
    }

    pub(super) async fn ready(&self) -> Result<(), EngineError> {
        loop {
            if self.exited() {
                return Err(EngineError::Load {
                    reason: "llama-server exited before it was ready".into(),
                });
            }
            let health = self.http.get(format!("{}/health", self.base)).send().await;
            if health.is_ok_and(|response| response.status().is_success()) {
                return Ok(());
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
    }

    pub(super) async fn chat(&self, body: Bytes) -> Result<(StatusCode, Bytes), EngineError> {
        let response = self
            .http
            .post(format!("{}/v1/chat/completions", self.base))
            .header("content-type", "application/json")
            .body(body)
            .send()
            .await
            .map_err(|error| self.lost(&error))?;
        let status = response.status();
        let bytes = response.bytes().await.map_err(|error| self.lost(&error))?;
        Ok((status, bytes))
    }

    pub(super) fn stop(&self) {
        let mut child = self.child();
        drop(child.stdin.take());
        let _ = child.wait();
    }

    fn lost(&self, error: &reqwest::Error) -> EngineError {
        if self.exited() {
            EngineError::Stopped
        } else {
            EngineError::Generation {
                reason: error.to_string(),
            }
        }
    }

    pub(super) fn exited(&self) -> bool {
        !matches!(self.child().try_wait(), Ok(None))
    }

    fn child(&self) -> MutexGuard<'_, Child> {
        self.child.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stop();
    }
}
