//! Tests for the url_extractor module.
//!
//! Unit tests verify HTML→Markdown conversion with fixture strings.
//! Integration tests spin up a local HTTP server and use obscura to fetch pages.

use super::convert::html_to_markdown;

// ─── Unit tests: HTML → Markdown conversion ───────────────────────────────────

#[cfg(test)]
mod conversion {
    use super::*;

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
        assert!(md.contains("> ") || md.contains("\n>")); // blockquote
        assert!(md.contains("```")); // code block
        assert!(md.contains("println!"));
        assert!(md.contains("`let x = 42;`")); // inline code
    }

    #[test]
    fn rich_content_ordered_list() {
        let html = include_str!("fixtures/rich.html");
        let md = html_to_markdown(html).unwrap();
        // htmd uses "1.  " (two spaces) for ordered lists
        assert!(md.contains("First item"), "missing 'First item' in: {md}");
        assert!(md.contains("Second item"));
        assert!(md.contains("Third item"));
        // Verify it's an ordered list (starts with digit + period)
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
    fn empty_page_has_no_body_content() {
        let html = include_str!("fixtures/empty.html");
        let md = html_to_markdown(html).unwrap();
        // htmd includes the <title> text; body content should be absent
        assert!(
            !md.contains("<") && !md.contains("</"),
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

// ─── Integration tests: full pipeline with obscura + local server ─────────────

#[cfg(test)]
mod integration {
    use super::super::{ExtractedContent, UrlExtractor};
    use std::net::SocketAddr;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    /// Allow obscura to connect to 127.0.0.1 for local test servers.
    fn allow_private_network() {
        // Safe: tests run single-threaded per test binary; this only affects obscura's SSRF guard.
        unsafe { std::env::set_var("OBSCURA_ALLOW_PRIVATE_NETWORK", "1") };
    }

    /// Serve a fixture over HTTP; returns (url, server_future) to drive with tokio::join!.
    async fn serve_fixture(
        fixture: &str,
    ) -> (String, impl std::future::Future<Output = ()>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr: SocketAddr = listener.local_addr().unwrap();
        let html: &str = match fixture {
            "simple" => include_str!("fixtures/simple.html"),
            "links" => include_str!("fixtures/links.html"),
            "rich" => include_str!("fixtures/rich.html"),
            "empty" => include_str!("fixtures/empty.html"),
            _ => panic!("unknown fixture: {fixture}"),
        };
        let html = html.to_string();

        let server = async move {
            if let Ok((mut stream, _)) = listener.accept().await {
                let mut buf = [0u8; 4096];
                let _ = stream.read(&mut buf).await;
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    html.len(),
                    html,
                );
                let _ = stream.write_all(response.as_bytes()).await;
                let _ = stream.flush().await;
            }
        };

        (format!("http://{addr}/"), server)
    }

    #[tokio::test]
    async fn fetch_simple_page() {
        allow_private_network();
        let (url, server) = serve_fixture("simple").await;
        let extractor = UrlExtractor::new().expect("failed to create browser");
        let (result, _) = tokio::join!(extractor.fetch(&url), server);
        let result: ExtractedContent = result.expect("fetch failed");

        assert!(result.markdown.contains("[link to example](https://example.com)"));
        assert!(result.markdown.contains("# Hello World"));
        assert_eq!(result.title, "Simple Page");
    }

    #[tokio::test]
    async fn fetch_link_heavy_page() {
        allow_private_network();
        let (url, server) = serve_fixture("links").await;
        let extractor = UrlExtractor::new().expect("failed to create browser");
        let (result, _) = tokio::join!(extractor.fetch(&url), server);
        let result = result.expect("fetch failed");

        assert!(result.markdown.contains("[Rust Language](https://rust-lang.org)"));
        assert!(result.markdown.contains("[Tauri Framework](https://tauri.app)"));
        assert!(result.markdown.contains("[Obscura Browser](https://github.com/h4ckf0r0day/obscura)"));
        assert_eq!(result.title, "Link Heavy Page");
    }

    #[tokio::test]
    async fn fetch_rich_page() {
        allow_private_network();
        let (url, server) = serve_fixture("rich").await;
        let extractor = UrlExtractor::new().expect("failed to create browser");
        let (result, _) = tokio::join!(extractor.fetch(&url), server);
        let result = result.expect("fetch failed");

        assert!(result.markdown.contains("# Article Title"));
        assert!(result.markdown.contains("## Section One"));
        assert!(result.markdown.contains("println!"));
        assert!(!result.markdown.contains("tracking"));
        assert_eq!(result.title, "Rich Content");
    }

    #[tokio::test]
    async fn fetch_empty_page() {
        allow_private_network();
        let (url, server) = serve_fixture("empty").await;
        let extractor = UrlExtractor::new().expect("failed to create browser");
        let (result, _) = tokio::join!(extractor.fetch(&url), server);
        let result = result.expect("fetch failed");

        assert!(!result.markdown.contains("<"));
        assert_eq!(result.title, "Empty Page");
    }

    #[tokio::test]
    async fn navigation_failure_returns_error() {
        allow_private_network();
        let extractor = UrlExtractor::new().expect("failed to create browser");
        let result = extractor.fetch("http://127.0.0.1:1/nonexistent").await;
        assert!(result.is_err());
    }
}
