mod local;
mod remote;

use std::sync::Arc;
use std::time::Duration;

use tauri::{AppHandle, Manager};

pub use self::local::{LocalEngine, Sampling, ServerConfig};
pub use self::remote::RemoteEngine;
use crate::model_store::ModelStore;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum EngineStatus {
    Online,
    Offline,
}

#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("could not start the inference engine: {reason}")]
    Init { reason: String },
    #[error("the inference engine is offline")]
    Offline,
}

pub trait Engine: Send + Sync {
    #[must_use]
    fn endpoint(&self) -> &str;

    #[must_use]
    fn status(&self) -> EngineStatus;

    #[must_use]
    fn is_online(&self) -> bool {
        self.status() == EngineStatus::Online
    }

    fn shutdown(&self);
}

pub enum InferenceEngine {
    Local(LocalEngine),
    Remote(RemoteEngine),
}

impl Engine for InferenceEngine {
    fn endpoint(&self) -> &str {
        match self {
            Self::Local(engine) => engine.endpoint(),
            Self::Remote(engine) => engine.endpoint(),
        }
    }

    fn status(&self) -> EngineStatus {
        match self {
            Self::Local(engine) => engine.status(),
            Self::Remote(engine) => engine.status(),
        }
    }

    fn shutdown(&self) {
        match self {
            Self::Local(engine) => engine.shutdown(),
            Self::Remote(engine) => engine.shutdown(),
        }
    }
}

/// Start the configured inference engine for `app_handle`.
/// # Errors
/// `EngineError::Init` if the model or server binary is missing or fails to start.
pub async fn get_inference_engine(app_handle: &AppHandle) -> Result<InferenceEngine, EngineError> {
    let store = app_handle.try_state::<Arc<ModelStore>>().ok_or_else(|| {
        log::error!("engine: cannot boot, the model store is not mounted");
        EngineError::Init {
            reason: "the model store is not available".into(),
        }
    })?;
    let model = store
        .model_files()
        .into_iter()
        .next()
        .ok_or_else(|| {
            log::error!("engine: cannot boot, no model file is on disk yet");
            EngineError::Init {
                reason: "the model is not on disk".into(),
            }
        })?;
    let exe = std::env::current_exe().map_err(|err| {
        log::error!("engine: cannot locate the app binary: {err}");
        EngineError::Init {
            reason: err.to_string(),
        }
    })?;
    let config = ServerConfig {
        binary: exe.with_file_name("llama-server"),
        slots: 4,
        slot_context: 16_384,
        cache_mib: 2_048,
        idle_seconds: 600,
        sampling: Sampling {
            temperature: 0.7,
            top_p: 0.8,
            top_k: 20,
            min_p: 0.0,
        },
        log: app_handle
            .path()
            .app_log_dir()
            .ok()
            .map(|dir| dir.join("llama-server.log")),
    };
    log::info!(
        "engine: booting llama-server for {} with {} slots and {} MiB cache, log {:?}",
        model.display(),
        config.slots,
        config.cache_mib,
        config.log
    );
    let local = LocalEngine::spawn(&config, &model, Duration::from_secs(300)).await?;
    Ok(InferenceEngine::Local(local))
}

#[must_use]
#[tauri::command]
pub fn engine_status(app: AppHandle) -> Option<EngineStatus> {
    let status = if let Some(engine) = app.try_state::<Arc<InferenceEngine>>() {
        Some(engine.status())
    } else {
        app.try_state::<EngineStatus>().map(|state| *state)
    };
    log::debug!("engine: status asked, answered {status:?}");
    drop(app);
    status
}

#[cfg(test)]
mod tests {
    use super::{Engine, EngineStatus};

    struct StubEngine {
        endpoint: String,
        status: EngineStatus,
    }

    impl Engine for StubEngine {
        fn endpoint(&self) -> &str {
            &self.endpoint
        }

        fn status(&self) -> EngineStatus {
            self.status
        }

        fn shutdown(&self) {}
    }

    #[test]
    fn status_serializes_lowercase_for_ipc() {
        assert_eq!(
            serde_json::to_string(&EngineStatus::Online).expect("json"),
            "\"online\""
        );
        assert_eq!(
            serde_json::to_string(&EngineStatus::Offline).expect("json"),
            "\"offline\""
        );
    }

    #[test]
    fn is_online_reflects_current_engine_status() {
        let online = StubEngine {
            endpoint: "http://127.0.0.1:8080/v1".into(),
            status: EngineStatus::Online,
        };
        let offline = StubEngine {
            endpoint: "http://127.0.0.1:8080/v1".into(),
            status: EngineStatus::Offline,
        };

        assert!(online.is_online());
        assert!(!offline.is_online());
    }
}
