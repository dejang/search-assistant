//! Agent — a minimal LLM agent built on [rig](https://github.com/0xPlaygrounds/rig).
pub mod progress;
pub mod tools;

mod main_prompt;
mod sub_agent_prompt;

use std::sync::Arc;

use rig_agent::{Agent as RigAgent, AgentBuilder, run::PromptError};
use rig_core::providers::openai::{OpenAI, OpenAIConfig};
use tauri::{Emitter, Manager, State};
use thiserror::Error;

use self::main_prompt::MAIN_PROMPT;
use self::progress::{ResearchProgress, Researcher};
use self::sub_agent_prompt::SUB_PROMPT;
use self::tools::{ReadUrlTool, SearchTool, SubagentTool, Tooling};
use crate::inference_engine::{Engine, EngineStatus, InferenceEngine};
use crate::url_extractor::PageExtractor;

#[derive(Error, Debug)]
pub enum AgentError {
    /// The provider rejected or failed the request.
    #[error("completion failed: {source}")]
    Completion {
        #[source]
        source: Box<PromptError>,
    },

    /// The provider answered successfully with no usable text.
    #[error("the provider returned an empty answer")]
    Empty,

    /// Another question is already being answered by the one local model.
    #[error("a question is still being answered; it cannot be cancelled")]
    Busy,

    /// No inference engine is available to answer questions.
    #[error("no agent can be used at this point")]
    Unavailable,
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
pub struct Agent {
    provider: OpenAI,
    model: String,
    prompt: String,
    inner: RigAgent,
    budget: RunBudget,
}

impl Agent {
    /// Build an agent that sends requests to `endpoint` for `model`.
    #[must_use]
    pub fn new(endpoint: impl Into<String>, model: impl Into<String>) -> Self {
        let provider = OpenAIConfig::new("local").with_base_url(endpoint).client();
        let model = model.into();
        let prompt = MAIN_PROMPT.to_string();
        let inner = AgentBuilder::new(provider.chat(model.clone()))
            .preamble(prompt.clone())
            .build();
        Self {
            provider,
            model,
            prompt,
            inner,
            budget: RunBudget {
                turns: 12,
                retries: 3,
                tools: 4,
            },
        }
    }

    #[must_use]
    pub fn with_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.prompt = prompt.into();
        self.inner = AgentBuilder::new(self.provider.chat(self.model.clone()))
            .preamble(self.prompt.clone())
            .build();
        self
    }

    #[must_use]
    pub fn with_budget(mut self, budget: RunBudget) -> Self {
        self.budget = budget;
        self
    }

    /// Attach `SearchTool`, `ReadUrlTool`, and `SubagentTool` for a research run.
    #[must_use]
    pub fn with_tooling<P: PageExtractor + 'static>(mut self, tooling: Tooling<P>) -> Self {
        let subagent = Self {
            provider: self.provider.clone(),
            model: self.model.clone(),
            prompt: SUB_PROMPT.to_string(),
            inner: AgentBuilder::new(self.provider.chat(self.model.clone()))
                .preamble(SUB_PROMPT)
                .build(),
            budget: self.budget,
        }
        .with_retrieval(tooling.clone(), Researcher::Subagent);

        let progress = Arc::clone(&tooling.progress);
        self.inner = AgentBuilder::new(self.provider.chat(self.model.clone()))
            .preamble(self.prompt.clone())
            .tool(SearchTool::new(tooling.clone(), Researcher::Lead))
            .tool(ReadUrlTool::new(tooling, Researcher::Lead))
            .tool(SubagentTool::new(subagent, progress))
            .build();
        self
    }

    fn with_retrieval<P: PageExtractor + 'static>(
        mut self,
        tooling: Tooling<P>,
        by: Researcher,
    ) -> Self {
        self.inner = AgentBuilder::new(self.provider.chat(self.model.clone()))
            .preamble(self.prompt.clone())
            .tool(SearchTool::new(tooling.clone(), by))
            .tool(ReadUrlTool::new(tooling, by))
            .build();
        self
    }

    /// Send `prompt` to the agent.
    /// # Errors
    /// `AgentError::Completion` if the request fails, `AgentError::Empty` if no text.
    pub async fn ask(&self, prompt: &str) -> Result<String, AgentError> {
        let asked = std::time::Instant::now();
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
        log::debug!(
            "agent: {} completion requests of {} allowed, {} bytes, {:?}",
            response.requests(),
            self.budget.turns,
            answer.len(),
            asked.elapsed()
        );
        if answer.trim().is_empty() {
            return Err(AgentError::Empty);
        }
        Ok(answer)
    }
}

#[tauri::command]
/// # Errors
/// Mirrors `Agent::ask`.
pub async fn research_ask(
    app: tauri::AppHandle,
    progress: State<'_, Arc<ResearchProgress>>,
    prompt: String,
) -> Result<String, AgentError> {
    let asked = std::time::Instant::now();
    log::info!("run: question received: {}", crate::clipped(&prompt, 160));
    let Some(engine) = app.try_state::<Arc<InferenceEngine>>() else {
        log::warn!("run: refused, no inference engine is mounted");
        return Err(AgentError::Unavailable);
    };
    if !engine.is_online() {
        log::warn!("run: refused, the inference engine is offline");
        let _ = app.emit("engine://status", EngineStatus::Offline);
        return Err(AgentError::Unavailable);
    }
    let Some(agent) = app.try_state::<Arc<Agent>>() else {
        log::warn!("run: refused, no agent is mounted");
        return Err(AgentError::Unavailable);
    };
    let Some(_run) = progress.begin() else {
        log::warn!(
            "run: refused, run {} still holds the only slot",
            progress.run_id()
        );
        return Err(AgentError::Busy);
    };
    log::info!("run {}: lead agent started", progress.run_id());
    let result = agent.ask(&prompt).await;
    match &result {
        Ok(answer) => log::info!(
            "run {}: answered {} chars in {:?}",
            progress.run_id(),
            answer.chars().count(),
            asked.elapsed()
        ),
        Err(err) => log::error!(
            "run {}: failed after {:?}: {}",
            progress.run_id(),
            asked.elapsed(),
            crate::clipped(&err.to_string(), 500)
        ),
    }
    if !engine.is_online() {
        log::warn!("run {}: engine went offline", progress.run_id());
        let _ = app.emit("engine://status", EngineStatus::Offline);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::AgentError;

    #[test]
    fn display_strings_and_ipc_serialization_are_pinned() {
        assert_eq!(
            AgentError::Empty.to_string(),
            "the provider returned an empty answer"
        );
        assert_eq!(
            AgentError::Busy.to_string(),
            "a question is still being answered; it cannot be cancelled"
        );
        assert_eq!(
            AgentError::Unavailable.to_string(),
            "no agent can be used at this point"
        );
        let json = serde_json::to_string(&AgentError::Unavailable).expect("json");
        assert_eq!(json, "\"no agent can be used at this point\"");
    }
}
