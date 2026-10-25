//! Agent — a minimal LLM agent built on [rig](https://github.com/0xPlaygrounds/rig).

use std::sync::Arc;
use std::time::Duration;

use rig_agent::{Agent as RigAgent, AgentBuilder, run::PromptError};
use rig_core::client::env::EnvError;
use rig_core::http_client::HttpClientExt;
use rig_core::providers::openai::{OpenAI, OpenAIConfig};
use rig_core::wire::Secret;
use tauri::State;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AgentError {
    /// Provider configuration could not be read from the environment.
    #[error("provider is not configured: {source}")]
    Config {
        #[source]
        source: EnvError,
    },

    /// The provider rejected or failed the request.
    #[error("completion failed: {source}")]
    Completion {
        #[source]
        source: Box<PromptError>,
    },

    /// The provider answered successfully with no usable text.
    #[error("the provider returned an empty answer")]
    Empty,

    /// The provider did not answer within the caller's deadline.
    #[error("the provider did not answer within {timeout:?}")]
    TimedOut { timeout: Duration },
}

impl AgentError {
    /// The provider's HTTP status, when the failure came from an answered
    #[must_use]
    pub fn status(&self) -> Option<u16> {
        match self {
            Self::Completion { source } => source.provider_response_status().map(|s| s.as_u16()),
            Self::Config { .. } | Self::Empty | Self::TimedOut { .. } => None,
        }
    }
}

/// Rendered the same way over IPC, so a `#[tauri::command]` can return
/// `Result<T, AgentError>` directly instead of hand-mapping to `String`.
impl serde::Serialize for AgentError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(&self)
    }
}

/// A stateless question-answering agent backed by a [`rig_agent::Agent`].
pub struct BuddyAgent {
    inner: RigAgent,
    model: String,
}

impl BuddyAgent {
    /// optional `OPENAI_BASE_URL`. An app launched from the Dock inherits no shell
    /// # Errors
    /// `AgentError::Config` if `OPENAI_API_KEY` or `OPENAI_BASE_URL` is unusable.
    pub fn new(model: impl Into<String>, preamble: impl Into<String>) -> Result<Self, AgentError> {
        let provider = OpenAI::from_env().map_err(|source| AgentError::Config { source })?;
        Ok(Self::build(&provider, model, preamble))
    }

    /// Build an agent for `model` and `preamble` with an explicit `api_key`,
    #[must_use]
    pub fn with_api_key(
        api_key: impl Into<Secret>,
        model: impl Into<String>,
        preamble: impl Into<String>,
    ) -> Self {
        Self::build(&OpenAI::new(api_key), model, preamble)
    }

    /// Build an agent that sends requests to `base_url` with `api_key`, for a
    #[must_use]
    pub fn with_endpoint(
        api_key: impl Into<Secret>,
        base_url: impl Into<String>,
        model: impl Into<String>,
        preamble: impl Into<String>,
    ) -> Self {
        let provider = OpenAIConfig::new(api_key).with_base_url(base_url).client();
        Self::build(&provider, model, preamble)
    }

    #[must_use]
    pub fn with_client<C: HttpClientExt + 'static>(
        client: C,
        model: impl Into<String>,
        preamble: impl Into<String>,
    ) -> Self {
        let provider = OpenAIConfig::new("local")
            .with_base_url("http://local-engine/v1")
            .connect(client);
        Self::build(&provider, model, preamble)
    }

    fn build(provider: &OpenAI, model: impl Into<String>, preamble: impl Into<String>) -> Self {
        let model = model.into();
        let inner = AgentBuilder::new(provider.chat(model.clone()))
            .preamble(preamble)
            .build();
        Self { inner, model }
    }

    /// The model this agent sends requests to.
    #[must_use]
    pub fn model(&self) -> &str {
        &self.model
    }

    /// No deadline: a gateway that accepts but never answers leaves this future
    /// # Errors
    /// `AgentError::Completion` if the request fails, `AgentError::Empty` if no text.
    pub async fn ask(&self, prompt: &str) -> Result<String, AgentError> {
        self.run(prompt).await
    }

    /// Needs a tokio runtime with its time driver, which Tauri's provides; the
    /// # Errors
    /// `AgentError::TimedOut` if `timeout` elapses; otherwise what `ask` gives.
    pub async fn ask_within(&self, prompt: &str, timeout: Duration) -> Result<String, AgentError> {
        tokio::time::timeout(timeout, self.run(prompt))
            .await
            .map_err(|_| AgentError::TimedOut { timeout })?
    }

    async fn run(&self, prompt: &str) -> Result<String, AgentError> {
        let response =
            self.inner
                .prompt(prompt)
                .await
                .map_err(|source| AgentError::Completion {
                    source: Box::new(source),
                })?;

        let answer = response.output();
        if answer.trim().is_empty() {
            return Err(AgentError::Empty);
        }
        Ok(answer)
    }
}

#[tauri::command]
/// # Errors
/// Mirrors `BuddyAgent::ask`.
pub async fn chat_ask(
    agent: State<'_, Arc<BuddyAgent>>,
    prompt: String,
) -> Result<String, AgentError> {
    agent.ask(&prompt).await
}

#[cfg(test)]
mod tests;
