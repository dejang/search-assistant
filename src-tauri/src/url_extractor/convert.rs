//! HTML → Markdown conversion with link preservation.

use regex::Regex;
use std::sync::LazyLock;

static SCRIPT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?is)<script[^>]*>.*?</script>").unwrap());
static STYLE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?is)<style[^>]*>.*?</style>").unwrap());
static NOSCRIPT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?is)<noscript[^>]*>.*?</noscript>").unwrap());

/// Convert an HTML string to Markdown, preserving links.
///
/// Strips `<script>`, `<style>`, and `<noscript>` elements before conversion,
/// then uses `htmd` (a turndown.js port) which handles:
/// - `<a href="...">text</a>` → `[text](url)`
/// - Headings, lists, code blocks, blockquotes, emphasis, tables
pub fn html_to_markdown(html: &str) -> Result<String, std::io::Error> {
    let cleaned = strip_non_content(html);
    htmd::convert(&cleaned)
}

/// Remove script, style, and noscript elements (including their content).
fn strip_non_content(html: &str) -> String {
    let html = SCRIPT_RE.replace_all(html, "");
    let html = STYLE_RE.replace_all(&html, "");
    let html = NOSCRIPT_RE.replace_all(&html, "");
    html.into_owned()
}
