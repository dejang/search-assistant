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
            let run = tooling.progress.run_id();
            let query = crate::clipped(&args.query, 120);
            tooling.progress.searched(by, &args.query);
            log::info!("run {run}: {by:?} search({query})");
            let asked = std::time::Instant::now();
            match tooling.extractor.search(&args.query).await {
                Ok(page) => {
                    log::info!(
                        "run {run}: {by:?} search got {} bytes of markdown from {} in {:?}",
                        page.markdown.len(),
                        crate::clipped(&page.url, 160),
                        asked.elapsed()
                    );
                    Ok(headed(
                        &page.url,
                        &page.title,
                        &page.markdown,
                        tooling.max_chars,
                    ))
                }
                Err(err) => {
                    log::warn!(
                        "run {run}: {by:?} search failed after {:?}: {}",
                        asked.elapsed(),
                        crate::clipped(&err.to_string(), 300)
                    );
                    Err(err)
                }
            }
        }
    }
}
