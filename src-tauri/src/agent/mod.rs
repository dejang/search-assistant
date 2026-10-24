//! Agent — a minimal LLM agent built on [rig](https://github.com/0xPlaygrounds/rig).
//!
//! The agent talks to an OpenAI-compatible provider. [`BuddyAgent::new`] takes
//! credentials from the environment: `OPENAI_API_KEY`, with an optional
//! `OPENAI_BASE_URL` override for self-hosted or proxy endpoints. The model
//! name and preamble are always supplied by the caller, so the agent carries no
//! built-in behaviour defaults.
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
    /// Kept structured so callers can branch on [`AgentError::status`] — a 401
    /// (open settings) is not a 429 (back off) is not a transport failure
    /// (offline). Boxed because `PromptError` carries a whole transcript and
    /// would otherwise dominate every `Result` in the crate.
    ///
    /// `Display` forwards the provider's own message, which is what a user needs
    /// to see, but it can echo provider-supplied text — prefer
    /// [`AgentError::status`] over matching on the string, and avoid persisting
    /// this text in logs.
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
    /// Build an agent for `model` and `preamble`, reading credentials from the
    /// environment (`OPENAI_API_KEY`, optional `OPENAI_BASE_URL`).
    ///
    /// Note that a packaged macOS app launched from the Dock does not inherit a
    /// shell environment; prefer [`with_api_key`](Self::with_api_key) or
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
    /// bypassing the environment.
    ///
    /// Always targets the default provider host: `OPENAI_BASE_URL` is *not*
    /// consulted on this path. Use [`with_endpoint`](Self::with_endpoint) for a
    /// proxy or a self-hosted endpoint.
    ///
    /// Builds a provider client (connection pool and trust store), so construct
    /// one agent and share it — for example via `app.manage` — rather than
    /// building one per request.
    ///
    /// This does not contact the provider, so construction cannot fail.
    #[must_use]
    pub fn with_api_key(
        api_key: impl Into<Secret>,
        model: impl Into<String>,
        preamble: impl Into<String>,
    ) -> Self {
        Self::build(&OpenAI::new(api_key), model, preamble)
    }

    /// Build an agent that sends requests to `base_url` with `api_key`, for a
    /// proxy, a gateway, or a local inference server that speaks the same wire
    /// format.
    ///
    /// Builds a provider client (connection pool and trust store), so construct
    /// one agent and share it rather than building one per request.
    ///
    /// This does not contact the provider, so construction cannot fail.
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
    /// There is no deadline: a gateway that accepts the connection and never
    /// answers leaves this future pending forever. Prefer
    /// [`ask_within`](Self::ask_within) anywhere a caller cannot tolerate a
    /// hang, such as a `#[tauri::command]` feeding a frontend promise.
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
    /// Requires a tokio runtime with its time driver active, which Tauri's
    /// async runtime provides. Polled off any tokio runtime this panics, and
    /// the release profile sets `panic = "abort"`, so never drive it from a
    /// plain `block_on` on a runtime built without `time`.
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
