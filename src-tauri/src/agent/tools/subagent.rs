//! The `subagent` tool: one focused web researcher on its own prompt.

use std::future::Future;

use rig_agent::{AgentBuilder, run::PromptError};
use rig_core::providers::openai::OpenAI;
use rig_core::tool::{Tool, ToolContext};
use rig_core::wasm_compat::WasmCompatSend;
use serde::Deserialize;
use serde_json::{Value, json};

use super::{ReadUrlTool, SearchTool, Tooling};
use crate::agent::progress::Researcher;
use crate::agent::sub_agent_prompt::SUB_PROMPT;
use crate::agent::{AgentError, RunBudget};

/// Spawns a Research Subagent for one sub-question and returns its digest.
pub struct SubagentTool {
    provider: OpenAI,
    model: String,
    tooling: Tooling,
    budget: RunBudget,
}

/// The four fields the orchestrator's task brief is made of.
#[derive(Deserialize)]
pub struct SubagentArgs {
    /// The single question this subagent answers.
    pub sub_question: String,
    /// Which angle this covers and what evidence to prioritise.
    pub target_angle: String,
    /// The broader goal, time frame, and boundaries.
    pub context: String,
    /// What other subagents already cover.
    pub exclusions: String,
}

impl SubagentTool {
    #[must_use]
    pub fn new(
        provider: OpenAI,
        model: impl Into<String>,
        tooling: Tooling,
        budget: RunBudget,
    ) -> Self {
        Self {
            provider,
            model: model.into(),
            tooling,
            budget,
        }
    }

    fn brief(args: &SubagentArgs) -> String {
        format!(
            "### Subagent Task Brief\n\
             1. **Assigned Sub-Question**: {}\n\
             2. **Target Angle**: {}\n\
             3. **Overall Context & Scope**: {}\n\
             4. **Exclusions**: {}",
            args.sub_question, args.target_angle, args.context, args.exclusions
        )
    }
}

impl Tool for SubagentTool {
    const NAME: &'static str = "subagent";
    type Args = SubagentArgs;
    type Output = String;
    type Error = AgentError;

    fn description(&self) -> String {
        "Spawn one Research Subagent to answer a single sub-question from the web. Pass the four brief fields; \
         returns the subagent's Evidence Digest."
            .to_string()
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "sub_question": {
                    "type": "string",
                    "description": "The specific question this subagent answers."
                },
                "target_angle": {
                    "type": "string",
                    "description": "Which angle this covers and what evidence to prioritise."
                },
                "context": {
                    "type": "string",
                    "description": "The user's broader goal, relevant time frame, and boundaries."
                },
                "exclusions": {
                    "type": "string",
                    "description": "What other subagents already cover, so this one does not repeat it."
                }
            },
            "required": ["sub_question", "target_angle", "context", "exclusions"]
        })
    }

    fn call(
        &self,
        _context: &mut ToolContext,
        args: Self::Args,
    ) -> impl Future<Output = Result<String, AgentError>> + WasmCompatSend {
        self.tooling
            .progress
            .delegated(Researcher::Lead, &args.sub_question);
        let brief = Self::brief(&args);
        let agent = AgentBuilder::new(self.provider.chat(self.model.clone()))
            .preamble(SUB_PROMPT)
            .tool(SearchTool::new(self.tooling.clone(), Researcher::Subagent))
            .tool(ReadUrlTool::new(self.tooling.clone(), Researcher::Subagent))
            .build();
        let budget = self.budget;
        async move {
            let answer = agent
                .prompt(&brief)
                .max_turns(budget.turns)
                .max_consecutive_malformed_tool_calls(budget.retries)
                .tool_concurrency(budget.tools)
                .await
                .map_err(|source: PromptError| AgentError::Completion {
                    source: Box::new(source),
                })?;
            let digest = answer.output();
            if digest.trim().is_empty() {
                return Err(AgentError::Empty);
            }
            Ok(digest)
        }
    }
}
