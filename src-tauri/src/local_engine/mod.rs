mod bridge;

#[cfg(test)]
mod tests;

use std::panic::AssertUnwindSafe;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::{Mutex, PoisonError};
use std::time::Duration;

use futures::FutureExt;
use mistralrs::{GgufModelBuilder, Model, RequestBuilder};
use rig_core::http_client::StatusCode;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::model_store::ModelStore;

pub use bridge::EngineClient;

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
    Panicked,

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
            Self::Load { .. } | Self::Panicked | Self::Unavailable => {
                StatusCode::SERVICE_UNAVAILABLE
            }
        }
    }

    fn is_fatal(&self) -> bool {
        matches!(
            self,
            Self::Panicked | Self::Unavailable | Self::Load { .. } | Self::TimedOut { .. }
        )
    }

    fn why(&self) -> &'static str {
        match self {
            Self::TimedOut { .. } => "the model did not answer in time",
            _ => "the model stopped answering",
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
    model: tokio::sync::Mutex<Option<Arc<Model>>>,
    building: Arc<AtomicU32>,
    in_flight: AtomicU32,
    epoch: AtomicU64,
    status: Arc<Mutex<EngineStatus>>,
    app: AppHandle,
}

impl LocalEngine {
    pub fn start(app: &AppHandle, store: Option<Arc<ModelStore>>, budget: Budget, slots: usize) {
        app.manage(Arc::new(Self::mounted(app, store, budget, slots)));
    }

    fn mounted(
        app: &AppHandle,
        store: Option<Arc<ModelStore>>,
        budget: Budget,
        slots: usize,
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
            slots: Arc::new(tokio::sync::Semaphore::new(slots.max(1))),
            model: tokio::sync::Mutex::new(None),
            building: Arc::new(AtomicU32::new(0)),
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

    pub(crate) async fn complete(
        self: &Arc<Self>,
        request: RequestBuilder,
    ) -> Result<serde_json::Value, EngineError> {
        let _question = self.begin_turn();
        self.turn(request).await
    }

    fn begin_turn(self: &Arc<Self>) -> TurnGuard {
        self.in_flight.fetch_add(1, Ordering::SeqCst);
        self.epoch.fetch_add(1, Ordering::SeqCst);
        TurnGuard {
            engine: Arc::clone(self),
        }
    }

    async fn turn(
        self: &Arc<Self>,
        request: RequestBuilder,
    ) -> Result<serde_json::Value, EngineError> {
        let budget = self.budget.answer;
        let slot = tokio::time::timeout(budget, Arc::clone(&self.slots).acquire_owned())
            .await
            .map_err(|_| EngineError::Unavailable)?
            .map_err(|_| EngineError::Unavailable)?;
        let model = self.loaded().await?;

        let generating = AssertUnwindSafe(model.send_chat_request(request)).catch_unwind();
        let outcome = tokio::time::timeout(budget, generating).await;
        drop(slot);

        let mapped = match outcome {
            Ok(Ok(Ok(completion))) => {
                self.publish(EnginePhase::Ready, None);
                return serde_json::to_value(completion).map_err(|error| EngineError::Generation {
                    reason: error.to_string(),
                });
            }
            Ok(Ok(Err(error))) => Self::refused(&error),
            Ok(Err(_)) => EngineError::Panicked,
            Err(_) => EngineError::TimedOut { timeout: budget },
        };
        if mapped.is_fatal() {
            self.forget_model(&model, mapped.why()).await;
        }
        Err(mapped)
    }

    async fn loaded(&self) -> Result<Arc<Model>, EngineError> {
        let mut held = self.model.lock().await;
        if let Some(model) = held.as_ref() {
            return Ok(Arc::clone(model));
        }
        if self.building.load(Ordering::SeqCst) > 0 {
            return Err(EngineError::Unavailable);
        }

        let files = self
            .store
            .as_ref()
            .map_or_else(Vec::new, |store| store.model_files());
        if files.is_empty() {
            return Err(EngineError::Load {
                reason: "the model is not on disk yet".into(),
            });
        }

        self.publish(EnginePhase::Loading, None);
        let loading = tokio::time::timeout(self.budget.load, self.load_model(&files)).await;
        match loading {
            Ok(Ok(model)) => {
                let model = Arc::new(model);
                *held = Some(Arc::clone(&model));
                self.publish(EnginePhase::Ready, None);
                Ok(model)
            }
            Ok(Err(error)) => {
                self.publish(EnginePhase::Failed, Some(error.to_string()));
                Err(error)
            }
            Err(_) => {
                let timeout = self.budget.load;
                let reason = format!("the model did not load within {timeout:?}");
                self.publish(EnginePhase::Failed, Some(reason.clone()));
                Err(EngineError::Load { reason })
            }
        }
    }

    async fn load_model(&self, files: &[PathBuf]) -> Result<Model, EngineError> {
        let names: Vec<String> = files
            .iter()
            .map(|path| path.display().to_string())
            .collect();
        self.building.fetch_add(1, Ordering::SeqCst);
        let count = Arc::clone(&self.building);
        let built = tauri::async_runtime::spawn_blocking(move || {
            let _count = Count(count);
            tauri::async_runtime::handle().block_on(async move {
                AssertUnwindSafe(GgufModelBuilder::new("local", names).build())
                    .catch_unwind()
                    .await
            })
        })
        .await
        .map_err(|error| {
            self.building.fetch_sub(1, Ordering::SeqCst);
            EngineError::Load {
                reason: error.to_string(),
            }
        })?;

        match built {
            Ok(Ok(model)) => Ok(model),
            Ok(Err(error)) => Err(EngineError::Load {
                reason: format!("{error:#}"),
            }),
            Err(_) => Err(EngineError::Panicked),
        }
    }

    fn quiescent(in_flight: u32, armed: u64, epoch: u64) -> bool {
        in_flight == 0 && armed == epoch
    }

    fn refused(error: &mistralrs::error::Error) -> EngineError {
        let reason = error.to_string();
        match error {
            mistralrs::error::Error::RequestValidation(..) => EngineError::Request { reason },
            mistralrs::error::Error::Inference(..)
            | mistralrs::error::Error::Channel(..)
            | mistralrs::error::Error::Management(..) => EngineError::Unavailable,
            mistralrs::error::Error::ModelLoad(..) => EngineError::Load { reason },
            _ => EngineError::Generation { reason },
        }
    }

    async fn idle_out(self: Arc<Self>, armed: u64) {
        tokio::time::sleep(self.budget.idle).await;
        self.unload(armed).await;
    }

    async fn unload(&self, armed: u64) {
        let mut held = self.model.lock().await;
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

    async fn forget_model(&self, model: &Arc<Model>, why: &str) {
        self.publish(
            EnginePhase::Failed,
            Some(format!("{why} and will load again next time")),
        );
        let mut held = self.model.lock().await;
        if held
            .as_ref()
            .is_some_and(|resident| Arc::ptr_eq(resident, model))
        {
            held.take();
        }
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

struct Count(Arc<AtomicU32>);

impl Drop for Count {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}
