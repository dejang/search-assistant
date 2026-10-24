//! URL Extractor — visit a URL with a headless browser and return its content as Markdown.
//!
//! Uses [obscura](https://github.com/h4ckf0r0day/obscura) for browsing and
//! [htmd](https://crates.io/crates/htmd) for HTML→Markdown, links preserved.
//! [`search_web`] is the command the webview calls with a research query.

mod convert;

#[cfg(test)]
mod tests;

pub use convert::html_to_markdown;

use std::sync::Arc;

use obscura::Browser;
use tauri::State;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ExtractorError {
    #[error("browser error: {0}")]
    Browser(String),

    #[error("navigation failed for {url}: {reason}")]
    Navigation { url: String, reason: String },

    #[error("conversion failed: {0}")]
    Conversion(String),
}

/// The causes are not serializable, and the webview only reads the sentence,
/// so that is all that crosses the boundary.
impl serde::Serialize for ExtractorError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

/// Extracted content from a URL.
#[derive(Debug, Clone)]
pub struct ExtractedContent {
    /// The final URL after any redirects.
    pub url: String,
    /// The page title.
    pub title: String,
    /// The page content converted to Markdown (links preserved).
    pub markdown: String,
}

/// Reads pages through `obscura`, at most `parallelism` pages at once.
///
/// Each read builds its own browser on a blocking thread driving a single-threaded
/// runtime: `deno_core`'s re-entrancy guard is thread-local, so two isolates may not
/// interleave on one thread. Permits bound how many run at once, not correctness.
pub struct UrlExtractor {
    pages: Arc<tokio::sync::Semaphore>,
}

impl UrlExtractor {
    /// An extractor running at most `parallelism` pages at once.
    #[must_use]
    pub fn new(parallelism: usize) -> Self {
        Self {
            pages: Arc::new(tokio::sync::Semaphore::new(parallelism.max(1))),
        }
    }

    /// The `DuckDuckGo` results page for `query`. Google answers a headless
    /// browser with a captcha, on any network that has not already been
    /// trusted, so the query goes to the engine that answers. Its HTML endpoint
    /// is static and caps a page near 30 results, short of the 50 wanted.
    fn search_url(query: &str) -> String {
        let encoded: String = form_urlencoded::byte_serialize(query.trim().as_bytes()).collect();
        format!("https://html.duckduckgo.com/html/?q={encoded}")
    }

    /// `DuckDuckGo` routes every result through `/l/?uddg=<target>&rut=<signature>`
    /// and encodes the target as a query value, so the wrapper comes off here.
    fn unwrap_redirects(markdown: &str) -> String {
        let marker = "](//duckduckgo.com/l/?";
        let mut out = String::with_capacity(markdown.len());
        let mut rest = markdown;
        while let Some(at) = rest.find(marker) {
            let from = at + marker.len();
            let to = from + rest[from..].find(')').unwrap_or(rest.len() - from);
            let query = &rest[from..to];
            let target = form_urlencoded::parse(query.as_bytes())
                .find(|(key, _)| key == "uddg")
                .map_or_else(|| query.to_string(), |(_, value)| value.into_owned());
            out.push_str(&rest[..at]);
            out.push_str("](");
            out.push_str(&target);
            out.push(')');
            rest = rest[to..].strip_prefix(')').unwrap_or(&rest[to..]);
        }
        out.push_str(rest);
        out
    }

    /// The page `query` returns on `DuckDuckGo`, with its result links pointing
    /// at the pages themselves.
    ///
    /// # Errors
    ///
    /// Returns [`ExtractorError`] on browser, navigation, or conversion failure.
    pub async fn search(&self, query: &str) -> Result<ExtractedContent, ExtractorError> {
        let mut content = self.fetch(&Self::search_url(query)).await?;
        content.markdown = Self::unwrap_redirects(&content.markdown);
        Ok(content)
    }

    /// Navigate to each `url` and extract its content, all at once, in the
    /// order given. One page failing says nothing about the others.
    ///
    /// # Errors
    ///
    /// Each entry carries its own [`ExtractorError`], or the failure of the
    /// thread that drove that page.
    pub async fn fetch_all(
        &self,
        urls: &[String],
    ) -> Vec<Result<ExtractedContent, ExtractorError>> {
        let mut running = Vec::with_capacity(urls.len());
        for url in urls {
            let pages = Arc::clone(&self.pages);
            let url = url.clone();
            running.push(tokio::task::spawn(
                async move { Self::read(&pages, &url).await },
            ));
        }
        let mut pages = Vec::with_capacity(urls.len());
        for handle in running {
            pages.push(
                handle
                    .await
                    .map_err(|e| ExtractorError::Browser(e.to_string()))
                    .and_then(|page| page),
            );
        }
        pages
    }

    /// Navigate to `url` and extract its content as Markdown.
    ///
    /// # Errors
    ///
    /// Returns [`ExtractorError`] on browser, navigation, or conversion failure,
    /// or when the thread driving the browser dies.
    pub async fn fetch(&self, url: &str) -> Result<ExtractedContent, ExtractorError> {
        Self::read(&self.pages, url).await
    }

    /// One page, once a permit says a browser fits.
    ///
    /// # Errors
    ///
    /// Returns [`ExtractorError`] on browser, navigation, or conversion failure,
    /// or when the thread driving the browser dies.
    async fn read(
        pages: &Arc<tokio::sync::Semaphore>,
        url: &str,
    ) -> Result<ExtractedContent, ExtractorError> {
        let page = Arc::clone(pages)
            .acquire_owned()
            .await
            .map_err(|e| ExtractorError::Browser(e.to_string()))?;
        let url = url.to_string();
        tauri::async_runtime::spawn_blocking(move || {
            let _page = page;
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| ExtractorError::Browser(e.to_string()))
                .and_then(|runtime| runtime.block_on(UrlExtractor::load(&url)))
        })
        .await
        .map_err(|e| ExtractorError::Browser(e.to_string()))?
    }

    /// One browser, opened here and dropped here, on the runtime `read` built.
    ///
    /// # Errors
    ///
    /// Returns [`ExtractorError`] on browser, navigation, or conversion failure.
    async fn load(url: &str) -> Result<ExtractedContent, ExtractorError> {
        let browser = Browser::builder()
            .build()
            .map_err(|e| ExtractorError::Browser(e.to_string()))?;
        let mut page = browser
            .new_page()
            .await
            .map_err(|e| ExtractorError::Browser(e.to_string()))?;

        page.goto(url)
            .await
            .map_err(|e| ExtractorError::Navigation {
                url: url.to_string(),
                reason: e.to_string(),
            })?;

        let final_url = page.url();
        let html = page.content();

        // Extract title via JS
        let title_val = page.evaluate("document.title");
        let title = title_val.as_str().unwrap_or("").to_string();

        let markdown = convert::html_to_markdown(&html)
            .map_err(|e| ExtractorError::Conversion(e.to_string()))?;

        Ok(ExtractedContent {
            url: final_url,
            title,
            markdown,
        })
    }
}

/// Extract the `DuckDuckGo` results page for `query`, and log what it said.
///
/// # Errors
///
/// Returns [`ExtractorError`] if the browser cannot start, the navigation
/// fails, or the page will not convert.
#[tauri::command]
pub async fn search_web(
    extractor: State<'_, Arc<UrlExtractor>>,
    query: String,
) -> Result<(), ExtractorError> {
    let content = extractor.search(&query).await?;
    log::info!("query {query:?} -> {} [{}]", content.url, content.title);
    log::info!("{}", content.markdown);
    Ok(())
}
