//! HTML → Markdown conversion with link preservation.

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
    let mut result = html.to_string();
    for tag in ["script", "style", "noscript"] {
        result = remove_tag_content(&result, tag);
    }
    result
}

/// Remove all `<tag ...>...</tag>` regions (case-insensitive).
fn remove_tag_content(html: &str, tag: &str) -> String {
    let lower = html.to_ascii_lowercase();
    let open = format!("<{tag}");
    let close = format!("</{tag}");

    let mut out = String::with_capacity(html.len());
    let mut pos = 0;

    while pos < html.len() {
        match lower[pos..].find(&open) {
            Some(rel) => {
                let tag_start = pos + rel;
                // Ensure it's actually the tag (next char is space, >, or /)
                let after_open = tag_start + open.len();
                let is_real_tag = lower[after_open..]
                    .chars()
                    .next()
                    .is_none_or(|c| c == '>' || c == ' ' || c == '\t' || c == '\n' || c == '/');

                if !is_real_tag {
                    out.push_str(&html[pos..after_open]);
                    pos = after_open;
                    continue;
                }

                // Find the closing tag
                match lower[after_open..].find(&close) {
                    Some(close_rel) => {
                        let close_start = after_open + close_rel;
                        // Skip past the '>' of the closing tag
                        let gt = lower[close_start..]
                            .find('>')
                            .map(|i| close_start + i + 1)
                            .unwrap_or(html.len());
                        out.push_str(&html[pos..tag_start]);
                        pos = gt;
                    }
                    None => {
                        // No closing tag found; keep the rest
                        out.push_str(&html[pos..]);
                        pos = html.len();
                    }
                }
            }
            None => {
                out.push_str(&html[pos..]);
                break;
            }
        }
    }

    out
}
