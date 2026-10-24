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
    async fn serve_fixture(fixture: &str) -> (String, impl std::future::Future<Output = ()>) {
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
        let extractor = UrlExtractor::new(4);
        let (result, _) = tokio::join!(extractor.fetch(&url), server);
        let result: ExtractedContent = result.expect("fetch failed");

        assert!(
            result
                .markdown
                .contains("[link to example](https://example.com)")
        );
        assert!(result.markdown.contains("# Hello World"));
        assert_eq!(result.title, "Simple Page");
    }

    #[tokio::test]
    async fn fetch_link_heavy_page() {
        allow_private_network();
        let (url, server) = serve_fixture("links").await;
        let extractor = UrlExtractor::new(4);
        let (result, _) = tokio::join!(extractor.fetch(&url), server);
        let result = result.expect("fetch failed");

        assert!(
            result
                .markdown
                .contains("[Rust Language](https://rust-lang.org)")
        );
        assert!(
            result
                .markdown
                .contains("[Tauri Framework](https://tauri.app)")
        );
        assert!(
            result
                .markdown
                .contains("[Obscura Browser](https://github.com/h4ckf0r0day/obscura)")
        );
        assert_eq!(result.title, "Link Heavy Page");
    }

    #[tokio::test]
    async fn fetch_rich_page() {
        allow_private_network();
        let (url, server) = serve_fixture("rich").await;
        let extractor = UrlExtractor::new(4);
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
        let extractor = UrlExtractor::new(4);
        let (result, _) = tokio::join!(extractor.fetch(&url), server);
        let result = result.expect("fetch failed");

        assert!(!result.markdown.contains("<"));
        assert_eq!(result.title, "Empty Page");
    }

    #[tokio::test]
    async fn navigation_failure_returns_error() {
        allow_private_network();
        let extractor = UrlExtractor::new(4);
        let result = extractor.fetch("http://127.0.0.1:1/nonexistent").await;
        assert!(result.is_err());
    }

    /// The app's runtime is multi-threaded, so the command path is exercised on
    /// one: obscura still has to be driven by the single-threaded runtime the
    /// extractor builds for itself, not by the one the command was awaited on.
    #[tokio::test(flavor = "multi_thread")]
    async fn a_fetch_gets_the_single_threaded_runtime_obscura_needs() {
        allow_private_network();
        let extractor = UrlExtractor::new(4);
        let (url, server) = serve_fixture("simple").await;

        let (result, ()) = tokio::join!(extractor.fetch(&url), server);
        let result = result.expect("the off-thread fetch failed");

        assert!(result.markdown.contains("# Hello World"));
        assert_eq!(result.title, "Simple Page");
    }

    /// Two pages read at once, each getting the browser and isolate of its own.
    #[tokio::test(flavor = "multi_thread")]
    async fn two_pages_read_at_once_never_share_an_isolate() {
        allow_private_network();
        let extractor = UrlExtractor::new(4);
        let (first, first_server) = serve_fixture("simple").await;
        let (second, second_server) = serve_fixture("empty").await;

        let (one, two, (), ()) = tokio::join!(
            extractor.fetch(&first),
            extractor.fetch(&second),
            first_server,
            second_server
        );

        assert_eq!(one.expect("first fetch failed").title, "Simple Page");
        assert_eq!(two.expect("second fetch failed").title, "Empty Page");
    }

    /// More pages than permits: the bound queues the rest rather than dropping
    /// or deadlocking them, and every answer comes back in the order asked for.
    #[tokio::test(flavor = "multi_thread")]
    async fn fetch_all_answers_every_page_in_order() {
        allow_private_network();
        let extractor = UrlExtractor::new(2);
        let (first, first_server) = serve_fixture("simple").await;
        let (second, second_server) = serve_fixture("empty").await;
        let (third, third_server) = serve_fixture("links").await;

        let urls = [first, second, third];
        let (pages, (), (), ()) = tokio::join!(
            extractor.fetch_all(&urls),
            first_server,
            second_server,
            third_server,
        );

        let titles: Vec<String> = pages
            .into_iter()
            .map(|page| page.expect("fetch_all page failed").title)
            .collect();
        assert_eq!(titles, ["Simple Page", "Empty Page", "Link Heavy Page"]);
    }
}

// ─── Unit tests: search URL ───────────────────────────────────────────────────

#[cfg(test)]
mod search {
    use super::super::UrlExtractor;

    #[test]
    fn the_query_is_encoded_into_the_results_page_url() {
        assert_eq!(
            UrlExtractor::search_url("rust borrow checker"),
            "https://html.duckduckgo.com/html/?q=rust+borrow+checker"
        );
    }

    #[test]
    fn characters_that_would_break_the_query_string_are_escaped() {
        assert_eq!(
            UrlExtractor::search_url("C++ & rust"),
            "https://html.duckduckgo.com/html/?q=C%2B%2B+%26+rust"
        );
    }

    #[test]
    fn padding_around_the_query_is_not_part_of_it() {
        assert_eq!(
            UrlExtractor::search_url("  mcp  "),
            "https://html.duckduckgo.com/html/?q=mcp"
        );
    }

    #[test]
    fn a_result_link_points_at_the_page_it_wraps() {
        let wrapped = "[LogRocket](//duckduckgo.com/l/?uddg=https%3A%2F%2Fblog.logrocket.com%2Fintroducing%2Drust%2Dborrow%2Dchecker%2F&rut=a5a24a)";

        assert_eq!(
            UrlExtractor::unwrap_redirects(wrapped),
            "[LogRocket](https://blog.logrocket.com/introducing-rust-borrow-checker/)"
        );
    }

    #[test]
    fn every_redirect_on_the_page_is_unwrapped() {
        let wrapped = "[a](//duckduckgo.com/l/?uddg=https%3A%2F%2Fa.dev&rut=1) text ![i](//duckduckgo.com/l/?uddg=https%3A%2F%2Fb.dev%2Ffavicon.ico&rut=2)";

        assert_eq!(
            UrlExtractor::unwrap_redirects(wrapped),
            "[a](https://a.dev) text ![i](https://b.dev/favicon.ico)"
        );
    }

    #[test]
    fn a_link_that_is_not_a_redirect_is_left_alone() {
        let markdown = "[docs](https://doc.rust-lang.org/book/) and [nav](/html/)";

        assert_eq!(UrlExtractor::unwrap_redirects(markdown), markdown);
    }
}
