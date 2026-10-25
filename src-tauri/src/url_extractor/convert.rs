/// Convert an HTML string to Markdown, preserving links.
/// # Errors
/// An I/O error if the HTML cannot be parsed.
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

fn is_tag_boundary(rest: &str) -> bool {
    rest.chars()
        .next()
        .is_none_or(|c| c.is_ascii_whitespace() || c == '>' || c == '/')
}

/// Remove all `<tag ...>...</tag>` regions (case-insensitive).
fn remove_tag_content(html: &str, tag: &str) -> String {
    let lower = html.to_ascii_lowercase();
    let open = format!("<{tag}");
    let close = format!("</{tag}");

    let mut out = String::with_capacity(html.len());
    let mut pos = 0;

    while pos < html.len() {
        if let Some(rel) = lower[pos..].find(&open) {
            let tag_start = pos + rel;
            let after_open = tag_start + open.len();
            if !is_tag_boundary(&lower[after_open..]) {
                out.push_str(&html[pos..after_open]);
                pos = after_open;
                continue;
            }

            if let Some(close_rel) = lower[after_open..].find(&close) {
                let close_start = after_open + close_rel;
                let gt = lower[close_start..]
                    .find('>')
                    .map_or(html.len(), |i| close_start + i + 1);
                out.push_str(&html[pos..tag_start]);
                pos = gt;
            } else {
                out.push_str(&html[pos..]);
                pos = html.len();
            }
        } else {
            out.push_str(&html[pos..]);
            break;
        }
    }

    out
}
