//! URL Extractor — coordinates concurrent page extraction and search redirect unwrapping.

mod convert;
mod page;
mod search;

use std::sync::Arc;
use std::time::Duration;

use thiserror::Error;

pub use self::page::{ObscuraPageExtractor, PageExtractor};
use self::search::{search_url, unwrap_redirects};

#[derive(Error, Debug)]
pub enum ExtractorError {
    #[error("browser error: {0}")]
    Browser(String),

    #[error("navigation failed for {url}: {reason}")]
    Navigation { url: String, reason: String },

    #[error("unexpected status {status} for {url}")]
    HttpStatus { url: String, status: u16 },

    #[error("unsupported content type {content_type} for {url}")]
    UnsupportedContentType { url: String, content_type: String },

    #[error("navigation to {url} timed out after {timeout:?}")]
    TimedOut { url: String, timeout: Duration },

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

/// Coordinates page extraction across worker threads bounded by a concurrency semaphore.
pub struct UrlExtractor<P = ObscuraPageExtractor> {
    pages: Arc<tokio::sync::Semaphore>,
    page_extractor: Arc<P>,
    timeout: Duration,
    search_base: String,
}

impl Default for UrlExtractor<ObscuraPageExtractor> {
    fn default() -> Self {
        Self::with_page_extractor(ObscuraPageExtractor)
    }
}

impl<P: PageExtractor> UrlExtractor<P> {
    /// Build an extractor backed by `page_extractor` with default parallelism and timeout.
    #[must_use]
    pub fn with_page_extractor(page_extractor: P) -> Self {
        Self {
            pages: Arc::new(tokio::sync::Semaphore::new(8)),
            page_extractor: Arc::new(page_extractor),
            timeout: Duration::from_secs(15),
            search_base: "https://html.duckduckgo.com/html/".to_string(),
        }
    }

    /// Cap concurrent page loads to `parallelism`.
    #[must_use]
    pub fn parallelism(mut self, parallelism: usize) -> Self {
        self.pages = Arc::new(tokio::sync::Semaphore::new(parallelism.max(1)));
        self
    }

    /// Bound each page load to `timeout`.
    #[must_use]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Override the search results endpoint URL.
    #[must_use]
    pub fn with_search_base(mut self, base: impl Into<String>) -> Self {
        self.search_base = base.into();
        self
    }

    /// Search `DuckDuckGo` for `query` and unwrap result redirect links.
    /// # Errors
    /// `ExtractorError` on browser, navigation, status, timeout, or conversion failure.
    pub async fn search(&self, query: &str) -> Result<ExtractedContent, ExtractorError> {
        let mut content = self.fetch(&search_url(&self.search_base, query)).await?;
        content.markdown = unwrap_redirects(&content.markdown);
        Ok(content)
    }

    /// Navigate to each `url` and extract its content, all at once, in the
    /// order given. One page failing says nothing about the others.
    pub async fn fetch_all(
        &self,
        urls: &[String],
    ) -> Vec<Result<ExtractedContent, ExtractorError>> {
        let mut running = Vec::with_capacity(urls.len());
        for url in urls {
            let pages = Arc::clone(&self.pages);
            let extractor = Arc::clone(&self.page_extractor);
            let url = url.clone();
            let timeout = self.timeout;
            running.push(tokio::task::spawn(async move {
                Self::read(&pages, &extractor, &url, timeout).await
            }));
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
    /// # Errors
    /// `ExtractorError` on failure, or when the driving thread dies.
    pub async fn fetch(&self, url: &str) -> Result<ExtractedContent, ExtractorError> {
        Self::read(&self.pages, &self.page_extractor, url, self.timeout).await
    }

    /// One page, once a permit says a worker fits.
    async fn read(
        pages: &Arc<tokio::sync::Semaphore>,
        extractor: &Arc<P>,
        url: &str,
        timeout: Duration,
    ) -> Result<ExtractedContent, ExtractorError> {
        let permit = Arc::clone(pages)
            .acquire_owned()
            .await
            .map_err(|e| ExtractorError::Browser(e.to_string()))?;
        let extractor = Arc::clone(extractor);
        let url = url.to_string();
        tauri::async_runtime::spawn_blocking(move || {
            let _permit = permit;
            extractor.extract(&url, timeout)
        })
        .await
        .map_err(|e| ExtractorError::Browser(e.to_string()))?
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{ExtractedContent, ExtractorError, PageExtractor, UrlExtractor};

    struct StubPageExtractor;

    impl PageExtractor for StubPageExtractor {
        fn extract(
            &self,
            url: &str,
            _timeout: Duration,
        ) -> Result<ExtractedContent, ExtractorError> {
            Ok(ExtractedContent {
                url: url.to_string(),
                title: "Stub".to_string(),
                markdown: "[Link](//duckduckgo.com/l/?uddg=https%3A%2F%2Frust-lang.org&rut=1)"
                    .to_string(),
            })
        }
    }

    #[tokio::test]
    async fn search_queries_configured_base_and_unwraps_redirects_without_browser() {
        let extractor = UrlExtractor::with_page_extractor(StubPageExtractor)
            .with_search_base("https://search.example/html/");

        let page = extractor.search("rust").await.unwrap();
        assert_eq!(page.url, "https://search.example/html/?q=rust");
        assert_eq!(page.markdown, "[Link](https://rust-lang.org)");
    }
}
