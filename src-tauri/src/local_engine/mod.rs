mod bridge;
mod server;

#[cfg(test)]
mod tests;

use std::path::Path;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, Weak};
use std::time::Duration;

use rig_core::http_client::{Bytes, StatusCode};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::model_store::ModelStore;

pub use bridge::EngineClient;
use server::Server;
pub use server::{Sampling, ServerConfig};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum EnginePhase {
    Absent,
    Loading,
    Ready,
    Unloaded,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineStatus {
    pub phase: EnginePhase,
    pub message: Option<String>,
}

impl EngineStatus {
    fn of(phase: EnginePhase) -> Self {
        Self {
            phase,
            message: None,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("the model could not be loaded: {reason}")]
    Load { reason: String },

    #[error("the model stopped answering and will load again next time")]
    Stopped,

    #[error("the model is not answering right now")]
    Unavailable,

    #[error("the model refused the request: {reason}")]
    Request { reason: String },

    #[error("the model failed mid-answer: {reason}")]
    Generation { reason: String },

    #[error("the model did not answer within {timeout:?}")]
    TimedOut { timeout: Duration },
}

impl EngineError {
    pub(crate) fn status(&self) -> StatusCode {
        match self {
            Self::Request { .. } => StatusCode::BAD_REQUEST,
            Self::Generation { .. } => StatusCode::INTERNAL_SERVER_ERROR,
            Self::TimedOut { .. } => StatusCode::GATEWAY_TIMEOUT,
            Self::Load { .. } | Self::Stopped | Self::Unavailable => {
                StatusCode::SERVICE_UNAVAILABLE
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Budget {
    pub idle: Duration,
    pub load: Duration,
    pub answer: Duration,
}

pub struct LocalEngine {
    store: Option<Arc<ModelStore>>,
    budget: Budget,
    slots: Arc<tokio::sync::Semaphore>,
    server: ServerConfig,
    running: tokio::sync::Mutex<Option<Arc<Server>>>,
    spawned: Mutex<Weak<Server>>,
    in_flight: AtomicU32,
    epoch: AtomicU64,
    status: Arc<Mutex<EngineStatus>>,
    app: AppHandle,
}

impl LocalEngine {
    pub fn start(
        app: &AppHandle,
        store: Option<Arc<ModelStore>>,
        budget: Budget,
        server: ServerConfig,
    ) {
        app.manage(Arc::new(Self::mounted(app, store, budget, server)));
    }

    fn mounted(
        app: &AppHandle,
        store: Option<Arc<ModelStore>>,
        budget: Budget,
        server: ServerConfig,
    ) -> Self {
        let loaded = store
            .as_ref()
            .is_some_and(|store| !store.model_files().is_empty());
        let phase = if loaded {
            EnginePhase::Unloaded
        } else {
            EnginePhase::Absent
        };
        Self {
            store,
            budget,
            slots: Arc::new(tokio::sync::Semaphore::new(server.slots.max(1))),
            server,
            running: tokio::sync::Mutex::new(None),
            spawned: Mutex::new(Weak::new()),
            in_flight: AtomicU32::new(0),
            epoch: AtomicU64::new(0),
            status: Arc::new(Mutex::new(EngineStatus::of(phase))),
            app: app.clone(),
        }
    }

    #[must_use]
    pub fn status(&self) -> EngineStatus {
        self.status
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    pub fn shutdown(&self) {
        if let Some(server) = self.spawned().upgrade() {
            server.stop();
        }
    }

    async fn complete(self: &Arc<Self>, body: Bytes) -> Result<(StatusCode, Bytes), EngineError> {
        let _question = self.begin_turn();
        self.turn(body).await
    }

    fn begin_turn(self: &Arc<Self>) -> TurnGuard {
        self.in_flight.fetch_add(1, Ordering::SeqCst);
        self.epoch.fetch_add(1, Ordering::SeqCst);
        TurnGuard {
            engine: Arc::clone(self),
        }
    }

    async fn turn(self: &Arc<Self>, body: Bytes) -> Result<(StatusCode, Bytes), EngineError> {
        let budget = self.budget.answer;
        let slot = tokio::time::timeout(budget, Arc::clone(&self.slots).acquire_owned())
            .await
            .map_err(|_| EngineError::Unavailable)?
            .map_err(|_| EngineError::Unavailable)?;
        let server = self.running().await?;

        let outcome = tokio::time::timeout(budget, server.chat(body)).await;
        drop(slot);

        match outcome {
            Ok(Ok(reply)) => {
                self.publish(EnginePhase::Ready, None);
                Ok(reply)
            }
            Ok(Err(EngineError::Stopped)) => {
                self.forget(&server).await;
                Err(EngineError::Stopped)
            }
            Ok(Err(error)) => Err(error),
            Err(_) => Err(EngineError::TimedOut { timeout: budget }),
        }
    }

    async fn running(&self) -> Result<Arc<Server>, EngineError> {
        let mut held = self.running.lock().await;
        if let Some(server) = held.as_ref().filter(|server| !server.exited()) {
            return Ok(Arc::clone(server));
        }

        let model = self
            .store
            .as_ref()
            .and_then(|store| store.model_files().into_iter().min())
            .ok_or_else(|| EngineError::Load {
                reason: "the model is not on disk yet".into(),
            })?;

        self.publish(EnginePhase::Loading, None);
        match self.launch(&model).await {
            Ok(server) => {
                *held = Some(Arc::clone(&server));
                self.publish(EnginePhase::Ready, None);
                Ok(server)
            }
            Err(error) => {
                self.publish(EnginePhase::Failed, Some(error.to_string()));
                Err(error)
            }
        }
    }

    async fn launch(&self, model: &Path) -> Result<Arc<Server>, EngineError> {
        let server = Arc::new(Server::spawn(&self.server, model)?);
        *self.spawned() = Arc::downgrade(&server);
        let timeout = self.budget.load;
        match tokio::time::timeout(timeout, server.ready()).await {
            Ok(ready) => ready.map(|()| server),
            Err(_) => Err(EngineError::Load {
                reason: format!("the model did not load within {timeout:?}"),
            }),
        }
    }

    fn quiescent(in_flight: u32, armed: u64, epoch: u64) -> bool {
        in_flight == 0 && armed == epoch
    }

    async fn idle_out(self: Arc<Self>, armed: u64) {
        tokio::time::sleep(self.budget.idle).await;
        self.unload(armed).await;
    }

    async fn unload(&self, armed: u64) {
        let mut held = self.running.lock().await;
        if !Self::quiescent(
            self.in_flight.load(Ordering::SeqCst),
            armed,
            self.epoch.load(Ordering::SeqCst),
        ) {
            return;
        }
        if held.take().is_some() {
            self.publish(EnginePhase::Unloaded, None);
        }
    }

    async fn forget(&self, server: &Arc<Server>) {
        let mut held = self.running.lock().await;
        if held
            .as_ref()
            .is_some_and(|resident| Arc::ptr_eq(resident, server))
        {
            held.take();
            self.publish(EnginePhase::Failed, Some(EngineError::Stopped.to_string()));
        }
    }

    fn spawned(&self) -> MutexGuard<'_, Weak<Server>> {
        self.spawned.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn publish(&self, phase: EnginePhase, message: Option<String>) {
        let next = EngineStatus { phase, message };
        {
            let mut held = self.status.lock().unwrap_or_else(PoisonError::into_inner);
            if *held == next {
                return;
            }
            *held = next.clone();
        }
        let _ = self.app.emit("engine://status", &next);
    }
}

struct TurnGuard {
    engine: Arc<LocalEngine>,
}

impl Drop for TurnGuard {
    fn drop(&mut self) {
        let engine = Arc::clone(&self.engine);
        if engine.in_flight.fetch_sub(1, Ordering::SeqCst) > 1 {
            return;
        }
        let armed = engine.epoch.load(Ordering::SeqCst);
        tauri::async_runtime::spawn(async move { engine.idle_out(armed).await });
    }
}

#[must_use]
#[tauri::command]
pub fn engine_status(engine: State<'_, Arc<LocalEngine>>) -> EngineStatus {
    engine.status()
}
