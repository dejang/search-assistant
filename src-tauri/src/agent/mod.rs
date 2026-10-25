//! Agent — a minimal LLM agent built on [rig](https://github.com/0xPlaygrounds/rig).
pub mod progress;
pub mod tools;

mod main_prompt;
mod sub_agent_prompt;

use std::sync::Arc;
use std::time::Duration;

use rig_agent::{Agent as RigAgent, AgentBuilder, run::PromptError};
use rig_core::client::env::EnvError;
use rig_core::http_client::HttpClientExt;
use rig_core::providers::openai::{OpenAI, OpenAIConfig};
use rig_core::wire::Secret;
use tauri::State;
use thiserror::Error;

use self::main_prompt::MAIN_PROMPT;
use self::progress::Researcher;
use self::tools::{ReadUrlTool, SearchTool, SubagentTool, Tooling};
use crate::agent::progress::ResearchProgress;

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

    /// Another question is already being answered by the one local model.
    #[error("a question is still being answered; it cannot be cancelled")]
    Busy,
}

impl AgentError {
    /// The provider's HTTP status, when the failure came from an answered
    #[must_use]
    pub fn status(&self) -> Option<u16> {
        match self {
            Self::Completion { source } => source.provider_response_status().map(|s| s.as_u16()),
            Self::Config { .. } | Self::Empty | Self::TimedOut { .. } | Self::Busy => None,
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

/// How many model calls one run may make, how many turns in a row may arrive
/// with tool arguments that do not parse, and how many tool calls run at once.
#[derive(Clone, Copy)]
pub struct RunBudget {
    pub turns: usize,
    pub retries: usize,
    pub tools: usize,
}

/// A stateless question-answering agent backed by a [`rig_agent::Agent`].
pub struct BuddyAgent {
    inner: RigAgent,
    model: String,
    budget: RunBudget,
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

    /// The lead researcher: `MAIN_PROMPT` as its preamble, and the three tools
    /// it works through. `budget` bounds its own run and every subagent's.
    #[must_use]
    pub fn with_research<C: HttpClientExt + 'static>(
        client: C,
        model: impl Into<String>,
        tooling: Tooling,
        budget: RunBudget,
    ) -> Self {
        let model = model.into();
        let provider = OpenAIConfig::new("local")
            .with_base_url("http://local-engine/v1")
            .connect(client);
        let inner = AgentBuilder::new(provider.chat(model.clone()))
            .preamble(MAIN_PROMPT)
            .tool(SearchTool::new(tooling.clone(), Researcher::Lead))
            .tool(ReadUrlTool::new(tooling.clone(), Researcher::Lead))
            .tool(SubagentTool::new(provider, model.clone(), tooling, budget))
            .build();
        Self {
            inner,
            model,
            budget,
        }
    }

    fn build(provider: &OpenAI, model: impl Into<String>, preamble: impl Into<String>) -> Self {
        let model = model.into();
        let inner = AgentBuilder::new(provider.chat(model.clone()))
            .preamble(preamble)
            .build();
        Self {
            inner,
            model,
            budget: RunBudget {
                turns: 1,
                retries: 3,
                tools: 1,
            },
        }
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
        let response = self
            .inner
            .prompt(prompt)
            .max_turns(self.budget.turns)
            .max_consecutive_malformed_tool_calls(self.budget.retries)
            .tool_concurrency(self.budget.tools)
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
pub async fn research_ask(
    agent: State<'_, Arc<BuddyAgent>>,
    progress: State<'_, Arc<ResearchProgress>>,
    prompt: String,
) -> Result<String, AgentError> {
    let _run = progress.begin().ok_or(AgentError::Busy)?;
    agent.ask(&prompt).await
}

#[cfg(test)]
mod tests;
