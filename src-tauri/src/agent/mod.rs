//! Agent — a minimal LLM agent built on [rig](https://github.com/0xPlaygrounds/rig).
//!
//! Talks to an OpenAI-compatible provider; [`BuddyAgent::new`] reads `OPENAI_API_KEY`
//! and an optional `OPENAI_BASE_URL`. The caller always supplies model and preamble.
//!
//! ```no_run
//! use research_buddy_lib::agent::BuddyAgent;
//!
//! # async fn run() -> Result<(), Box<dyn std::error::Error>> {
//! let agent = BuddyAgent::new("gpt-4o-mini", "You are a concise research assistant.")?;
//! let answer = agent.ask("What is a vector database?").await?;
//! # Ok(())
//! # }
//! ```

use std::time::Duration;

use rig_agent::{Agent as RigAgent, AgentBuilder, run::PromptError};
use rig_core::client::env::EnvError;
use rig_core::providers::openai::{OpenAI, OpenAIConfig};
use rig_core::wire::Secret;
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
    ///
    /// Branch on [`AgentError::status`]: a 401 (open settings) is not a 429 (back
    /// off) is not a transport failure (offline). Boxed because `PromptError`
    /// carries a whole transcript; `Display` can echo provider text, so do not log it.
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
    /// request. Lets a command layer branch on 401 / 429 / 5xx without parsing
    /// `Display`, and returns [`None`] for transport failures, timeouts, and
    /// empty answers.
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
///
/// The agent is cheap to reuse: each [`ask`](Self::ask) call is an independent
/// request, and no conversation history carries over between calls.
pub struct BuddyAgent {
    inner: RigAgent,
    model: String,
}

impl BuddyAgent {
    /// Build an agent for `model` and `preamble` from `OPENAI_API_KEY` and the
    /// optional `OPENAI_BASE_URL`. An app launched from the Dock inherits no shell
    /// environment, so prefer [`with_api_key`](Self::with_api_key) or
    /// [`with_endpoint`](Self::with_endpoint) when the host supplies the key.
    ///
    /// # Errors
    ///
    /// Returns [`AgentError::Config`] if `OPENAI_API_KEY` is unset or invalid,
    /// or if `OPENAI_BASE_URL` is set but is not a usable URL.
    pub fn new(model: impl Into<String>, preamble: impl Into<String>) -> Result<Self, AgentError> {
        let provider = OpenAI::from_env().map_err(|source| AgentError::Config { source })?;
        Ok(Self::build(&provider, model, preamble))
    }

    /// Build an agent for `model` and `preamble` with an explicit `api_key`,
    /// bypassing the environment. `OPENAI_BASE_URL` is not consulted here; use
    /// [`with_endpoint`](Self::with_endpoint) for a proxy or self-hosted host.
    /// Builds a provider client, so construct one agent and share it; building
    /// does not contact the provider, so it cannot fail.
    #[must_use]
    pub fn with_api_key(
        api_key: impl Into<Secret>,
        model: impl Into<String>,
        preamble: impl Into<String>,
    ) -> Self {
        Self::build(&OpenAI::new(api_key), model, preamble)
    }

    /// Build an agent that sends requests to `base_url` with `api_key`, for a
    /// proxy, a gateway, or a local inference server on the same wire format.
    ///
    /// Builds a provider client, so construct one agent and share it; building
    /// does not contact the provider, so it cannot fail.
    #[must_use]
    pub fn with_endpoint(
        api_key: impl Into<Secret>,
        base_url: impl Into<String>,
        model: impl Into<String>,
        preamble: impl Into<String>,
    ) -> Self {
        let provider = OpenAIConfig::new(api_key)
            .with_base_url(base_url)
            .client();
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

    /// Send `prompt` to the model and return its text reply.
    ///
    /// No deadline: a gateway that accepts but never answers leaves this future
    /// pending. Prefer [`ask_within`](Self::ask_within) where a hang is intolerable.
    ///
    /// # Errors
    ///
    /// - [`AgentError::Completion`] if the provider request fails.
    /// - [`AgentError::Empty`] if the provider answered with no usable text, so
    ///   a content-filtered or reasoning-only turn is not rendered as a blank
    ///   success.
    pub async fn ask(&self, prompt: &str) -> Result<String, AgentError> {
        self.run(prompt).await
    }

    /// [`ask`](Self::ask), giving up after `timeout`.
    ///
    /// Needs a tokio runtime with its time driver, which Tauri's provides; the
    /// release profile aborts on panic, so never drive this without `time`.
    ///
    /// # Errors
    ///
    /// - [`AgentError::TimedOut`] if `timeout` elapses first.
    /// - Anything [`ask`](Self::ask) returns.
    pub async fn ask_within(
        &self,
        prompt: &str,
        timeout: Duration,
    ) -> Result<String, AgentError> {
        tokio::time::timeout(timeout, self.run(prompt))
            .await
            .map_err(|_| AgentError::TimedOut { timeout })?
    }

    async fn run(&self, prompt: &str) -> Result<String, AgentError> {
        let response = self
            .inner
            .prompt(prompt)
            .await
            .map_err(|source| AgentError::Completion { source: Box::new(source) })?;

        let answer = response.output();
        if answer.trim().is_empty() {
            return Err(AgentError::Empty);
        }
        Ok(answer)
    }
}

#[cfg(test)]
mod tests;
