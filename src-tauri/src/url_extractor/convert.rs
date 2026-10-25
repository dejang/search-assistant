//! MIME classification and HTML/PDF to Markdown conversion.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DocumentKind {
    Html,
    PlainText,
    Pdf,
}

/// Classify an HTTP `Content-Type` header value into a supported document kind.
pub(super) fn classify_mime(content_type: Option<&str>) -> Option<DocumentKind> {
    let raw = content_type?;
    let mime = raw
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    match mime.as_str() {
        "text/html" | "application/xhtml+xml" => Some(DocumentKind::Html),
        "text/plain" | "text/markdown" => Some(DocumentKind::PlainText),
        "application/pdf" => Some(DocumentKind::Pdf),
        _ => None,
    }
}

/// Convert an HTML string to Markdown, preserving links.
pub(super) fn html_to_markdown(html: &str) -> Result<String, std::io::Error> {
    let cleaned = strip_non_content(html);
    htmd::convert(&cleaned)
}

/// Extract text from a PDF byte buffer as Markdown.
pub(super) fn pdf_to_markdown(bytes: &[u8]) -> Result<String, String> {
    pdf_extract::extract_text_from_mem(bytes)
        .map(|text| text.trim().to_string())
        .map_err(|e| e.to_string())
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

#[cfg(test)]
mod tests {
    use super::{DocumentKind, classify_mime, html_to_markdown, pdf_to_markdown};

    #[test]
    fn classifies_supported_and_unsupported_mime_types() {
        assert_eq!(
            classify_mime(Some("text/html; charset=utf-8")),
            Some(DocumentKind::Html)
        );
        assert_eq!(
            classify_mime(Some("application/xhtml+xml")),
            Some(DocumentKind::Html)
        );
        assert_eq!(
            classify_mime(Some("text/plain; charset=utf-8")),
            Some(DocumentKind::PlainText)
        );
        assert_eq!(
            classify_mime(Some("application/pdf")),
            Some(DocumentKind::Pdf)
        );
        assert_eq!(classify_mime(Some("image/png")), None);
        assert_eq!(classify_mime(Some("application/octet-stream")), None);
        assert_eq!(classify_mime(None), None);
    }

    #[test]
    fn extracts_text_from_pdf_fixture() {
        let pdf = include_bytes!("fixtures/sample.pdf");
        let md = pdf_to_markdown(pdf).unwrap();
        assert!(
            md.contains("PDF Research Document"),
            "missing heading in: {md}"
        );
        assert!(
            md.contains("Extracted from a PDF fixture."),
            "missing body in: {md}"
        );
    }

    #[test]
    fn simple_page_preserves_link() {
        let html = include_str!("fixtures/simple.html");
        let md = html_to_markdown(html).unwrap();
        assert!(md.contains("[link to example](https://example.com)"));
        assert!(md.contains("# Hello World"));
        assert!(md.contains("**bold**"));
        assert!(md.contains("_italic_") || md.contains("*italic*"));
    }

    #[test]
    fn link_heavy_page_preserves_all_links() {
        let html = include_str!("fixtures/links.html");
        let md = html_to_markdown(html).unwrap();
        assert!(md.contains("[Rust Language](https://rust-lang.org)"));
        assert!(md.contains("[Tauri Framework](https://tauri.app)"));
        assert!(md.contains("[Obscura Browser](https://github.com/h4ckf0r0day/obscura)"));
        assert!(md.contains("[docs](https://docs.rs)"));
        assert!(md.contains("[crate registry](https://crates.io)"));
    }

    #[test]
    fn link_with_query_params_preserved() {
        let html = include_str!("fixtures/links.html");
        let md = html_to_markdown(html).unwrap();
        assert!(
            md.contains("https://example.org/path?q=1&ref=2"),
            "query-param link missing from output: {md}"
        );
    }

    #[test]
    fn bold_link_preserved() {
        let html = include_str!("fixtures/links.html");
        let md = html_to_markdown(html).unwrap();
        assert!(
            md.contains("bold linked") && md.contains("https://example.com"),
            "bold link not preserved: {md}"
        );
    }

    #[test]
    fn rich_content_structure() {
        let html = include_str!("fixtures/rich.html");
        let md = html_to_markdown(html).unwrap();
        assert!(md.contains("# Article Title"));
        assert!(md.contains("## Section One"));
        assert!(md.contains("> ") || md.contains("\n>"));
        assert!(md.contains("```"));
        assert!(md.contains("println!"));
        assert!(md.contains("`let x = 42;`"));
    }

    #[test]
    fn rich_content_ordered_list() {
        let html = include_str!("fixtures/rich.html");
        let md = html_to_markdown(html).unwrap();
        assert!(md.contains("First item"), "missing 'First item' in: {md}");
        assert!(md.contains("Second item"));
        assert!(md.contains("Third item"));
        assert!(
            md.contains("1.") && md.contains("2.") && md.contains("3."),
            "not an ordered list: {md}"
        );
    }

    #[test]
    fn script_and_style_stripped() {
        let html = include_str!("fixtures/rich.html");
        let md = html_to_markdown(html).unwrap();
        assert!(!md.contains("tracking"), "script content leaked: {md}");
        assert!(!md.contains("display: none"), "style content leaked: {md}");
    }

    #[test]
    fn crlf_script_tag_is_stripped() {
        let html = "<script\r\ntype=\"text/javascript\">\r\n tracking();\r\n</script><p>ok</p>";
        let md = html_to_markdown(html).unwrap();
        assert!(!md.contains("tracking"), "CRLF script content leaked: {md}");
        assert!(md.contains("ok"));
    }

    #[test]
    fn empty_page_has_no_body_content() {
        let html = include_str!("fixtures/empty.html");
        let md = html_to_markdown(html).unwrap();
        assert!(
            !md.contains('<') && !md.contains("</"),
            "unexpected HTML in output: {md}"
        );
    }

    #[test]
    fn standalone_link_conversion() {
        let html = r#"<a href="https://rust-lang.org">Rust</a>"#;
        let md = html_to_markdown(html).unwrap();
        assert_eq!(md.trim(), "[Rust](https://rust-lang.org)");
    }

    #[test]
    fn nested_list_with_links() {
        let html = r#"<ul><li><a href="https://a.com">A</a></li><li><a href="https://b.com">B</a></li></ul>"#;
        let md = html_to_markdown(html).unwrap();
        assert!(md.contains("[A](https://a.com)"));
        assert!(md.contains("[B](https://b.com)"));
    }
}
