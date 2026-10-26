//! The tools the research agents call.

mod read_url;
mod search;
mod subagent;

use std::sync::Arc;

pub use read_url::ReadUrlTool;
pub use search::SearchTool;
pub use subagent::SubagentTool;

use super::progress::ResearchProgress;
use crate::url_extractor::{ObscuraPageExtractor, UrlExtractor};

/// What every retrieval tool needs: the browser, the counter, and how much of
/// a page it may hand back.
pub struct Tooling<P = ObscuraPageExtractor> {
    pub extractor: Arc<UrlExtractor<P>>,
    pub progress: Arc<ResearchProgress>,
    pub max_chars: usize,
}

impl<P> Clone for Tooling<P> {
    fn clone(&self) -> Self {
        Self {
            extractor: Arc::clone(&self.extractor),
            progress: Arc::clone(&self.progress),
            max_chars: self.max_chars,
        }
    }
}

impl<P> Tooling<P> {
    #[must_use]
    pub fn new(
        extractor: Arc<UrlExtractor<P>>,
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

#[cfg(test)]
mod tests {
    use super::{cut, headed};

    #[test]
    fn headed_prepends_url_and_title_and_truncates_body() {
        let out = headed("https://example.com", "Example", "Hello world", 5);
        assert_eq!(out, "url: https://example.com\ntitle: Example\n\nHello");
    }

    #[test]
    fn cut_never_splits_a_multibyte_codepoint() {
        let text = "aé🦀b";
        assert_eq!(cut(text, 1), "a");
        assert_eq!(cut(text, 2), "a");
        assert_eq!(cut(text, 3), "aé");
        assert_eq!(cut(text, 5), "aé");
        assert_eq!(cut(text, 7), "aé🦀");
    }
}
