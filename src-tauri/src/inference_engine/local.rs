use std::fs::OpenOptions;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::Duration;

use super::{Engine, EngineError, EngineStatus};

#[derive(Clone, Copy)]
pub struct Sampling {
    pub temperature: f32,
    pub top_p: f32,
    pub top_k: u32,
    pub min_p: f32,
}

#[derive(Clone)]
pub struct ServerConfig {
    pub binary: PathBuf,
    pub slots: usize,
    pub slot_context: usize,
    pub cache_mib: usize,
    pub idle_seconds: u64,
    pub sampling: Sampling,
    pub log: Option<PathBuf>,
}

pub struct LocalEngine {
    child: Mutex<Child>,
    endpoint: String,
}

impl LocalEngine {
    /// Spawn `llama-server` for `model` and wait until `/health` answers `200 OK`.
    /// # Errors
    /// `EngineError::Init` if the binary or model is missing, or the server fails to start.
    pub async fn spawn(
        config: &ServerConfig,
        model: &Path,
        load_timeout: Duration,
    ) -> Result<Self, EngineError> {
        if !config.binary.is_file() {
            log::error!("engine: no llama-server at {}", config.binary.display());
            return Err(EngineError::Init {
                reason: format!("no llama-server at {}", config.binary.display()),
            });
        }
        if !model.is_file() {
            log::error!("engine: no model at {}", model.display());
            return Err(EngineError::Init {
                reason: format!("no model at {}", model.display()),
            });
        }

        let listener = TcpListener::bind(("127.0.0.1", 0)).map_err(|err| EngineError::Init {
            reason: err.to_string(),
        })?;
        let port = listener
            .local_addr()
            .map_err(|err| EngineError::Init {
                reason: err.to_string(),
            })?
            .port();
        drop(listener);

        let (stdout, stderr) = log_streams(config.log.as_deref());
        let mut command = Command::new(&config.binary);
        for (name, _) in std::env::vars_os() {
            if name.to_string_lossy().starts_with("LLAMA_") {
                command.env_remove(name);
            }
        }
        let child = command
            .arg("--model")
            .arg(model)
            .args(["--host", "127.0.0.1", "--port", &port.to_string()])
            .args([
                "--parallel",
                &config.slots.to_string(),
                "--kv-unified-per-slot",
                &config.slot_context.to_string(),
                "--cache-ram",
                &config.cache_mib.to_string(),
                "--sleep-idle-seconds",
                &config.idle_seconds.to_string(),
            ])
            .args(["--gpu-layers", "all", "--reasoning", "off", "--no-webui"])
            .args([
                "--temp",
                &config.sampling.temperature.to_string(),
                "--top-p",
                &config.sampling.top_p.to_string(),
                "--top-k",
                &config.sampling.top_k.to_string(),
                "--min-p",
                &config.sampling.min_p.to_string(),
            ])
            .stdin(Stdio::null())
            .stdout(stdout)
            .stderr(stderr)
            .spawn()
            .map_err(|err| {
                log::error!("engine: llama-server would not start: {err}");
                EngineError::Init {
                    reason: err.to_string(),
                }
            })?;

        log::info!(
            "engine: llama-server pid {} asked to listen on port {port}, ready within {load_timeout:?}",
            child.id()
        );
        let engine = Self {
            child: Mutex::new(child),
            endpoint: format!("http://127.0.0.1:{port}/v1"),
        };
        engine.wait_ready(port, load_timeout).await?;
        Ok(engine)
    }

    async fn wait_ready(&self, port: u16, load_timeout: Duration) -> Result<(), EngineError> {
        let health_url = format!("http://127.0.0.1:{port}/health");
        let client = reqwest::Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(2))
            .build()
            .map_err(|err| EngineError::Init {
                reason: err.to_string(),
            })?;
        let started = std::time::Instant::now();
        let poll = async {
            let mut probes = 0_u32;
            loop {
                if !self.is_online() {
                    log::error!("engine: llama-server exited before it was ready");
                    return Err(EngineError::Init {
                        reason: "llama-server exited before it was ready".into(),
                    });
                }
                if let Ok(response) = client.get(&health_url).send().await
                    && response.status() == reqwest::StatusCode::OK
                {
                    log::debug!(
                        "engine: llama-server answered /health after {probes} probes ({:?})",
                        started.elapsed()
                    );
                    return Ok(());
                }
                probes += 1;
                if probes.is_multiple_of(100) {
                    log::info!(
                        "engine: llama-server not ready after {probes} probes ({:?})",
                        started.elapsed()
                    );
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        };
        tokio::time::timeout(load_timeout, poll)
            .await
            .map_err(|_| {
                log::error!("engine: llama-server was not ready within {load_timeout:?}");
                EngineError::Init {
                    reason: format!("llama-server did not become ready within {load_timeout:?}"),
                }
            })?
    }
}

impl Engine for LocalEngine {
    fn endpoint(&self) -> &str {
        &self.endpoint
    }

    fn status(&self) -> EngineStatus {
        let Ok(mut child) = self.child.lock() else {
            return EngineStatus::Offline;
        };
        match child.try_wait() {
            Ok(None) => EngineStatus::Online,
            Ok(Some(_)) | Err(_) => EngineStatus::Offline,
        }
    }

    fn shutdown(&self) {
        if let Ok(mut child) = self.child.lock() {
            log::info!("engine: stopping llama-server pid {}", child.id());
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

impl Drop for LocalEngine {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn log_streams(path: Option<&Path>) -> (Stdio, Stdio) {
    let captured = path.and_then(|file| {
        let parent = file.parent()?;
        std::fs::create_dir_all(parent).ok()?;
        let out = OpenOptions::new()
            .create(true)
            .append(true)
            .open(file)
            .ok()?;
        let err = out.try_clone().ok()?;
        Some((Stdio::from(out), Stdio::from(err)))
    });
    if captured.is_none() && path.is_some() {
        log::warn!("engine: llama-server output will not be captured");
    }
    captured.unwrap_or_else(|| (Stdio::null(), Stdio::null()))
}
