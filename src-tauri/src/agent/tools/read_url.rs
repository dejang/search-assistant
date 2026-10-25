//! The `read_url` tool: one page through `url_extractor`, as Markdown.

use std::future::Future;

use rig_core::tool::{Tool, ToolContext};
use rig_core::wasm_compat::WasmCompatSend;
use serde::Deserialize;
use serde_json::{Value, json};

use super::{Tooling, headed};
use crate::agent::progress::Researcher;
use crate::url_extractor::ExtractorError;

/// Reads one page and returns it as Markdown.
pub struct ReadUrlTool {
    tooling: Tooling,
    by: Researcher,
}

/// The page the agent asked for.
#[derive(Deserialize)]
pub struct ReadUrlArgs {
    /// The URL to read.
    pub url: String,
}

impl ReadUrlTool {
    #[must_use]
    pub fn new(tooling: Tooling, by: Researcher) -> Self {
        Self { tooling, by }
    }
}

impl Tool for ReadUrlTool {
    const NAME: &'static str = "read_url";
    type Args = ReadUrlArgs;
    type Output = String;
    type Error = ExtractorError;

    fn description(&self) -> String {
        "Read one web page and return its content as Markdown, with the final URL and title it resolved to."
            .to_string()
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "The absolute URL to read."
                }
            },
            "required": ["url"]
        })
    }

    fn call(
        &self,
        _context: &mut ToolContext,
        args: Self::Args,
    ) -> impl Future<Output = Result<String, ExtractorError>> + WasmCompatSend {
        let tooling = self.tooling.clone();
        let by = self.by;
        async move {
            tooling.progress.read(by, &args.url);
            let page = tooling.extractor.fetch(&args.url).await?;
            Ok(headed(
                &page.url,
                &page.title,
                &page.markdown,
                tooling.max_chars,
            ))
        }
    }
}
