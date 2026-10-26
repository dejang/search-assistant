//! The `subagent` tool: one focused web researcher on its own prompt.

use std::future::Future;
use std::sync::Arc;

use rig_core::tool::{Tool, ToolContext};
use rig_core::wasm_compat::WasmCompatSend;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::agent::progress::{ResearchProgress, Researcher};
use crate::agent::{Agent, AgentError};

/// Spawns a Research Subagent for one sub-question and returns its digest.
pub struct SubagentTool {
    subagent: Arc<Agent>,
    progress: Arc<ResearchProgress>,
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
    pub fn new(subagent: Agent, progress: Arc<ResearchProgress>) -> Self {
        Self {
            subagent: Arc::new(subagent),
            progress,
        }
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
        let run = self.progress.run_id();
        let sub_question = crate::clipped(&args.sub_question, 120);
        self.progress.delegated(Researcher::Lead, &args.sub_question);
        log::info!("run {run}: Lead delegated a subagent: {sub_question}");
        let task_brief = brief(&args);
        let subagent = Arc::clone(&self.subagent);
        async move {
            let asked = std::time::Instant::now();
            let outcome = subagent.ask(&task_brief).await;
            match &outcome {
                Ok(digest) => log::info!(
                    "run {run}: subagent {sub_question} answered {} bytes in {:?}",
                    digest.len(),
                    asked.elapsed()
                ),
                Err(err) => log::warn!(
                    "run {run}: subagent {sub_question} failed after {:?}: {}",
                    asked.elapsed(),
                    crate::clipped(&err.to_string(), 500)
                ),
            }
            outcome
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{SubagentArgs, brief};

    #[test]
    fn brief_formats_all_four_fields_in_order() {
        let formatted = brief(&SubagentArgs {
            sub_question: "What is Qwen3?".into(),
            target_angle: "Angle A".into(),
            context: "2026 release".into(),
            exclusions: "Pricing".into(),
        });
        assert!(formatted.contains("1. **Assigned Sub-Question**: What is Qwen3?"));
        assert!(formatted.contains("2. **Target Angle**: Angle A"));
        assert!(formatted.contains("3. **Overall Context & Scope**: 2026 release"));
        assert!(formatted.contains("4. **Exclusions**: Pricing"));
    }
}
