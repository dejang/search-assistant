//! The `search` tool: a web search through `url_extractor`.

use std::future::Future;

use rig_core::tool::{Tool, ToolContext};
use rig_core::wasm_compat::WasmCompatSend;
use serde::Deserialize;
use serde_json::{Value, json};

use super::{Tooling, headed};
use crate::agent::progress::Researcher;
use crate::url_extractor::{ExtractorError, ObscuraPageExtractor, PageExtractor};

/// Runs a web search and returns the results page as Markdown.
pub struct SearchTool<P = ObscuraPageExtractor> {
    tooling: Tooling<P>,
    by: Researcher,
}

/// The search the agent asked for.
#[derive(Deserialize)]
pub struct SearchArgs {
    /// The search terms.
    pub query: String,
}

impl<P> SearchTool<P> {
    #[must_use]
    pub fn new(tooling: Tooling<P>, by: Researcher) -> Self {
        Self { tooling, by }
    }
}

impl<P: PageExtractor + 'static> Tool for SearchTool<P> {
    const NAME: &'static str = "search";
    type Args = SearchArgs;
    type Output = String;
    type Error = ExtractorError;

    fn description(&self) -> String {
        "Search the web and return the results page as Markdown, where every result carries its title and link."
            .to_string()
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The search terms, phrased for a search engine."
                }
            },
            "required": ["query"]
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
            tooling.progress.searched(by, &args.query);
            let page = tooling.extractor.search(&args.query).await?;
            Ok(headed(
                &page.url,
                &page.title,
                &page.markdown,
                tooling.max_chars,
            ))
        }
    }
}
