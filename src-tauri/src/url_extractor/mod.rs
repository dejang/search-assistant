//! URL Extractor — visit a URL with a headless browser and return its content as Markdown.
//!
//! Uses [obscura](https://github.com/h4ckf0r0day/obscura) for headless browsing
//! and [htmd](https://crates.io/crates/htmd) for HTML→Markdown conversion.
//! Links are preserved in the output.

mod convert;

#[cfg(test)]
mod tests;

pub use convert::html_to_markdown;

use obscura::Browser;
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

/// Fetch a URL using the headless browser and return its content as Markdown.
///
/// This creates a fresh browser instance per call. For repeated use, prefer
/// [`UrlExtractor`] which reuses a single browser.
///
/// # Errors
///
/// Returns [`ExtractorError`] if the browser cannot start, navigation fails,
/// or HTML→Markdown conversion fails.
pub async fn extract(url: &str) -> Result<ExtractedContent, ExtractorError> {
    let extractor = UrlExtractor::new()?;
    extractor.fetch(url).await
}

/// A reusable URL extractor backed by a persistent headless browser instance.
pub struct UrlExtractor {
    browser: Browser,
}

impl UrlExtractor {
    /// Create a new extractor with a headless browser.
    ///
    /// # Errors
    ///
    /// Returns [`ExtractorError::Browser`] if the browser engine fails to initialize.
    pub fn new() -> Result<Self, ExtractorError> {
        let browser = Browser::builder()
            .build()
            .map_err(|e| ExtractorError::Browser(e.to_string()))?;
        Ok(Self { browser })
    }

    /// Navigate to `url` and extract its content as Markdown.
    ///
    /// # Errors
    ///
    /// Returns [`ExtractorError`] on browser, navigation, or conversion failure.
    pub async fn fetch(&self, url: &str) -> Result<ExtractedContent, ExtractorError> {
        let mut page = self
            .browser
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
