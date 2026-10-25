//! The tools the research agents call.

mod read_url;
mod search;
mod subagent;

use std::sync::Arc;

pub use read_url::ReadUrlTool;
pub use search::SearchTool;
pub use subagent::SubagentTool;

use super::progress::ResearchProgress;
use crate::url_extractor::UrlExtractor;

/// What every retrieval tool needs: the browser, the counter, and how much of
/// a page it may hand back.
#[derive(Clone)]
pub struct Tooling {
    pub extractor: Arc<UrlExtractor>,
    pub progress: Arc<ResearchProgress>,
    pub max_chars: usize,
}

impl Tooling {
    #[must_use]
    pub fn new(
        extractor: Arc<UrlExtractor>,
        progress: Arc<ResearchProgress>,
        max_chars: usize,
    ) -> Self {
        Self {
            extractor,
            progress,
            max_chars: max_chars.max(1),
        }
    }
}

/// The page's own url and title above its Markdown, so the agent can cite them.
pub(super) fn headed(url: &str, title: &str, markdown: &str, max_chars: usize) -> String {
    format!("url: {url}\ntitle: {title}\n\n{}", cut(markdown, max_chars))
}

/// `text` within `max` bytes, never between the bytes of one character.
fn cut(text: &str, max: usize) -> &str {
    if text.len() <= max {
        return text;
    }
    let mut end = max;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}
