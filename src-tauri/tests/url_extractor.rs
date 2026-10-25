#[cfg(test)]
mod integration {
    use std::collections::HashMap;
    use std::future::IntoFuture;
    use std::net::SocketAddr;
    use std::time::{Duration, Instant};

    use axum::Router;
    use axum::extract::Query;
    use axum::http::{HeaderMap, HeaderValue, StatusCode};
    use axum::response::{Html, IntoResponse, Redirect};
    use axum::routing::get;
    use research_buddy_lib::url_extractor::{ExtractorError, UrlExtractor};

    fn allow_private_network() {
        unsafe { std::env::set_var("OBSCURA_ALLOW_PRIVATE_NETWORK", "1") };
    }

    async fn simple_page() -> Html<&'static str> {
        Html(include_str!("../src/url_extractor/fixtures/simple.html"))
    }

    async fn links_page() -> Html<&'static str> {
        Html(include_str!("../src/url_extractor/fixtures/links.html"))
    }

    async fn rich_page() -> Html<&'static str> {
        Html(include_str!("../src/url_extractor/fixtures/rich.html"))
    }

    async fn empty_page() -> Html<&'static str> {
        Html(include_str!("../src/url_extractor/fixtures/empty.html"))
    }

    async fn dynamic_page() -> Html<&'static str> {
        Html(include_str!("../src/url_extractor/fixtures/dynamic.html"))
    }

    async fn pdf_document() -> impl IntoResponse {
        let mut headers = HeaderMap::new();
        headers.insert("content-type", HeaderValue::from_static("application/pdf"));
        (
            StatusCode::OK,
            headers,
            include_bytes!("../src/url_extractor/fixtures/sample.pdf").as_slice(),
        )
    }

    async fn not_found_page() -> impl IntoResponse {
        (
            StatusCode::NOT_FOUND,
            Html("<html><head><title>404</title></head><body>Not Found</body></html>"),
        )
    }

    async fn server_error_page() -> impl IntoResponse {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Html("<html><head><title>500</title></head><body>Server Error</body></html>"),
        )
    }

    async fn png_image() -> impl IntoResponse {
        let mut headers = HeaderMap::new();
        headers.insert("content-type", HeaderValue::from_static("image/png"));
        (
            StatusCode::OK,
            headers,
            vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a],
        )
    }

    async fn binary_blob() -> impl IntoResponse {
        let mut headers = HeaderMap::new();
        headers.insert(
            "content-type",
            HeaderValue::from_static("application/octet-stream"),
        );
        (StatusCode::OK, headers, vec![0x00, 0xff, 0x10, 0x80])
    }

    async fn slow_one_second() -> Html<&'static str> {
        tokio::time::sleep(Duration::from_secs(1)).await;
        Html(include_str!("../src/url_extractor/fixtures/simple.html"))
    }

    async fn stalled_fifteen_seconds() -> Html<&'static str> {
        tokio::time::sleep(Duration::from_secs(15)).await;
        Html(include_str!("../src/url_extractor/fixtures/simple.html"))
    }

    async fn redirect_to_simple() -> Redirect {
        Redirect::temporary("/simple")
    }

    async fn search_endpoint(Query(params): Query<HashMap<String, String>>) -> impl IntoResponse {
        if params.get("q").map(String::as_str) != Some("rust book") {
            return (
                StatusCode::BAD_REQUEST,
                Html("<html><body>unexpected query</body></html>"),
            );
        }
        (
            StatusCode::OK,
            Html(include_str!(
                "../src/url_extractor/fixtures/search_results.html"
            )),
        )
    }

    async fn spawn_test_server() -> String {
        allow_private_network();
        let app = Router::new()
            .route("/simple", get(simple_page))
            .route("/links", get(links_page))
            .route("/rich", get(rich_page))
            .route("/empty", get(empty_page))
            .route("/dynamic", get(dynamic_page))
            .route("/sample.pdf", get(pdf_document))
            .route("/missing", get(not_found_page))
            .route("/error", get(server_error_page))
            .route("/image.png", get(png_image))
            .route("/blob.bin", get(binary_blob))
            .route("/slow-1s", get(slow_one_second))
            .route("/hang-15s", get(stalled_fifteen_seconds))
            .route("/redirect", get(redirect_to_simple))
            .route("/html/", get(search_endpoint));

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr: SocketAddr = listener.local_addr().unwrap();
        drop(tokio::task::spawn(axum::serve(listener, app).into_future()));
        format!("http://{addr}")
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn visits_html_url_and_returns_markdown() {
        let base = spawn_test_server().await;
        let extractor = UrlExtractor::default();

        let simple = extractor
            .fetch(&format!("{base}/simple"))
            .await
            .expect("simple HTML page should extract");
        assert_eq!(simple.title, "Simple Page");
        assert!(simple.markdown.contains("# Hello World"));
        assert!(
            simple
                .markdown
                .contains("[link to example](https://example.com)")
        );

        let links = extractor
            .fetch(&format!("{base}/links"))
            .await
            .expect("links HTML page should extract");
        assert_eq!(links.title, "Link Heavy Page");
        assert!(
            links
                .markdown
                .contains("[Rust Language](https://rust-lang.org)")
        );
        assert!(
            links
                .markdown
                .contains("[Obscura Browser](https://github.com/h4ckf0r0day/obscura)")
        );

        let rich = extractor
            .fetch(&format!("{base}/rich"))
            .await
            .expect("rich HTML page should extract");
        assert_eq!(rich.title, "Rich Content");
        assert!(rich.markdown.contains("# Article Title"));
        assert!(rich.markdown.contains("println!"));
        assert!(!rich.markdown.contains("tracking"));

        let empty = extractor
            .fetch(&format!("{base}/empty"))
            .await
            .expect("empty HTML page should extract");
        assert_eq!(empty.title, "Empty Page");
        assert!(!empty.markdown.contains('<'));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn visits_pdf_url_and_returns_extracted_markdown() {
        let base = spawn_test_server().await;
        let extractor = UrlExtractor::default();

        let pdf = extractor
            .fetch(&format!("{base}/sample.pdf"))
            .await
            .expect("PDF URL should extract");
        assert!(
            pdf.markdown.contains("PDF Research Document"),
            "missing PDF heading in: {}",
            pdf.markdown
        );
        assert!(
            pdf.markdown.contains("Extracted from a PDF fixture."),
            "missing PDF body in: {}",
            pdf.markdown
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn rejects_non_2xx_http_status() {
        let base = spawn_test_server().await;
        let extractor = UrlExtractor::default();

        let err_404 = extractor
            .fetch(&format!("{base}/missing"))
            .await
            .expect_err("404 response must return an error");
        assert!(
            matches!(err_404, ExtractorError::HttpStatus { status: 404, .. }),
            "expected HttpStatus 404, got: {err_404:?}"
        );

        let err_500 = extractor
            .fetch(&format!("{base}/error"))
            .await
            .expect_err("500 response must return an error");
        assert!(
            matches!(err_500, ExtractorError::HttpStatus { status: 500, .. }),
            "expected HttpStatus 500, got: {err_500:?}"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn rejects_unsupported_mime_type() {
        let base = spawn_test_server().await;
        let extractor = UrlExtractor::default();

        let png_err = extractor
            .fetch(&format!("{base}/image.png"))
            .await
            .expect_err("image/png response must be rejected");
        assert!(
            matches!(
                png_err,
                ExtractorError::UnsupportedContentType {
                    ref content_type,
                    ..
                } if content_type == "image/png"
            ),
            "expected UnsupportedContentType(image/png), got: {png_err:?}"
        );

        let bin_err = extractor
            .fetch(&format!("{base}/blob.bin"))
            .await
            .expect_err("application/octet-stream response must be rejected");
        assert!(
            matches!(
                bin_err,
                ExtractorError::UnsupportedContentType {
                    ref content_type,
                    ..
                } if content_type == "application/octet-stream"
            ),
            "expected UnsupportedContentType(application/octet-stream), got: {bin_err:?}"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn fetches_four_slow_urls_concurrently_in_roughly_one_second() {
        let base = spawn_test_server().await;
        let extractor = UrlExtractor::default().parallelism(4);
        let urls = vec![
            format!("{base}/slow-1s?page=1"),
            format!("{base}/slow-1s?page=2"),
            format!("{base}/slow-1s?page=3"),
            format!("{base}/slow-1s?page=4"),
        ];

        let started = Instant::now();
        let results = extractor.fetch_all(&urls).await;
        let elapsed = started.elapsed();

        assert_eq!(results.len(), 4);
        for res in results {
            let page = res.expect("concurrent slow page should succeed");
            assert_eq!(page.title, "Simple Page");
        }
        assert!(
            elapsed >= Duration::from_secs(1),
            "4 pages waiting 1s cannot finish in {elapsed:?}"
        );
        assert!(
            elapsed < Duration::from_millis(1800),
            "4 concurrent 1s pages took {elapsed:?}, expected ~1s (parallel, not sequential 4s)"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn aborts_navigation_that_exceeds_configured_timeout() {
        let base = spawn_test_server().await;
        let extractor = UrlExtractor::default().with_timeout(Duration::from_millis(300));

        let started = Instant::now();
        let err = extractor
            .fetch(&format!("{base}/hang-15s"))
            .await
            .expect_err("stalled 15s route must time out");
        let elapsed = started.elapsed();

        assert!(
            matches!(err, ExtractorError::TimedOut { .. }),
            "expected TimedOut, got: {err:?}"
        );
        assert!(
            elapsed < Duration::from_secs(3),
            "300ms timeout took {elapsed:?} to return"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn fetch_all_isolates_failures_and_preserves_order() {
        let base = spawn_test_server().await;
        let extractor = UrlExtractor::default()
            .parallelism(4)
            .with_timeout(Duration::from_millis(300));
        let urls = vec![
            format!("{base}/simple"),
            format!("{base}/missing"),
            format!("{base}/hang-15s"),
            format!("{base}/links"),
        ];

        let results = extractor.fetch_all(&urls).await;
        assert_eq!(results.len(), 4);
        assert_eq!(results[0].as_ref().unwrap().title, "Simple Page");
        assert!(matches!(
            results[1],
            Err(ExtractorError::HttpStatus { status: 404, .. })
        ));
        assert!(matches!(results[2], Err(ExtractorError::TimedOut { .. })));
        assert_eq!(results[3].as_ref().unwrap().title, "Link Heavy Page");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn follows_http_redirects_and_records_final_url() {
        let base = spawn_test_server().await;
        let extractor = UrlExtractor::default();

        let page = extractor
            .fetch(&format!("{base}/redirect"))
            .await
            .expect("302 redirect to /simple should succeed");
        assert_eq!(page.url, format!("{base}/simple"));
        assert_eq!(page.title, "Simple Page");
        assert!(page.markdown.contains("# Hello World"));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn queues_requests_beyond_parallelism_bound_without_dropping() {
        let base = spawn_test_server().await;
        let extractor = UrlExtractor::default().parallelism(2);
        let urls = vec![
            format!("{base}/simple"),
            format!("{base}/empty"),
            format!("{base}/links"),
            format!("{base}/rich"),
        ];

        let pages = extractor.fetch_all(&urls).await;
        let titles: Vec<String> = pages
            .into_iter()
            .map(|p| p.expect("queued page must succeed").title)
            .collect();
        assert_eq!(
            titles,
            [
                "Simple Page",
                "Empty Page",
                "Link Heavy Page",
                "Rich Content"
            ]
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn executes_inline_scripts_and_strips_script_tags_from_markdown() {
        let base = spawn_test_server().await;
        let extractor = UrlExtractor::default();

        let page = extractor
            .fetch(&format!("{base}/dynamic"))
            .await
            .expect("JS-mutated page should extract");
        assert_eq!(page.title, "Rendered By Script");
        assert!(page.markdown.contains("# Dynamic Heading"));
        assert!(page.markdown.contains("Populated by V8."));
        assert!(!page.markdown.contains("getElementById"));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn search_fetches_results_and_unwraps_duckduckgo_redirect_links() {
        let base = spawn_test_server().await;
        let extractor = UrlExtractor::default().with_search_base(format!("{base}/html/"));

        let results = extractor
            .search("  rust book  ")
            .await
            .expect("search should succeed");
        assert_eq!(results.title, "DuckDuckGo Results");
        assert!(
            results
                .markdown
                .contains("[Learn Rust](https://www.rust-lang.org/learn)"),
            "redirect link not unwrapped in: {}",
            results.markdown
        );
        assert!(
            results
                .markdown
                .contains("[The Rust Book](https://doc.rust-lang.org/book/)"),
            "redirect link not unwrapped in: {}",
            results.markdown
        );
    }
}
