//! Single-URL extraction trait and `obscura`-backed implementation.

use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use obscura::{Browser, Page, ResourceType};

use super::convert::{DocumentKind, classify_mime, html_to_markdown, pdf_to_markdown};
use super::{ExtractedContent, ExtractorError};

/// Single-URL extractor invoked on a worker thread for one page load.
pub trait PageExtractor: Send + Sync + 'static {
    /// Load `url` within `timeout` and return its extracted Markdown content.
    /// # Errors
    /// [`ExtractorError`] on browser, navigation, status, MIME, timeout, or conversion failure.
    fn extract(&self, url: &str, timeout: Duration) -> Result<ExtractedContent, ExtractorError>;
}

/// Headless browser single-URL extractor backed by `obscura`.
#[derive(Default, Clone, Copy)]
pub struct ObscuraPageExtractor;

struct DocumentResponse {
    status: u16,
    content_type: Option<String>,
    body: Vec<u8>,
}

impl ObscuraPageExtractor {
    async fn load(url: &str) -> Result<ExtractedContent, ExtractorError> {
        let browser = Browser::builder()
            .build()
            .map_err(|e| ExtractorError::Browser(e.to_string()))?;
        let mut page = browser
            .new_page()
            .await
            .map_err(|e| ExtractorError::Browser(e.to_string()))?;
        let shown = crate::clipped(url, 160);
        let captured = Self::observe_document(&mut page);

        log::debug!("extract: navigating to {shown}");
        page.goto(url)
            .await
            .map_err(|e| {
                log::warn!("extract: navigation to {shown} failed: {e}");
                ExtractorError::Navigation {
                    url: url.to_string(),
                    reason: e.to_string(),
                }
            })?;

        let final_url = page.url();
        let landed = crate::clipped(&final_url, 160);
        log::debug!("extract: {shown} resolved to {landed}");
        // The guard drops with this statement, so the log below cannot hold it
        // while obscura's response callback wants the same lock.
        let taken = captured
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        let doc = taken.ok_or_else(|| {
            log::warn!("extract: {landed} sent no document response");
            ExtractorError::Navigation {
                url: url.to_string(),
                reason: "no document response received".to_string(),
            }
        })?;
        Self::into_content(&page, final_url, doc)
    }

    fn observe_document(page: &mut Page) -> Arc<Mutex<Option<DocumentResponse>>> {
        let captured = Arc::new(Mutex::new(None));
        let slot = Arc::clone(&captured);
        page.on_response(Arc::new(move |req, resp| {
            if req.resource_type == ResourceType::Document {
                let content_type = resp.content_type().map(str::to_owned);
                let keep_body = (200..300).contains(&resp.status)
                    && matches!(
                        classify_mime(content_type.as_deref()),
                        Some(DocumentKind::PlainText | DocumentKind::Pdf)
                    );
                let body = if keep_body {
                    resp.body.clone()
                } else {
                    Vec::new()
                };
                let mut guard = slot.lock().unwrap_or_else(PoisonError::into_inner);
                *guard = Some(DocumentResponse {
                    status: resp.status,
                    content_type,
                    body,
                });
            }
        }));
        captured
    }

    fn into_content(
        page: &Page,
        final_url: String,
        doc: DocumentResponse,
    ) -> Result<ExtractedContent, ExtractorError> {
        let shown = crate::clipped(&final_url, 160);
        if !(200..300).contains(&doc.status) {
            log::warn!("extract: {shown} answered status {}", doc.status);
            return Err(ExtractorError::HttpStatus {
                url: final_url,
                status: doc.status,
            });
        }
        let Some(kind) = classify_mime(doc.content_type.as_deref()) else {
            let content_type = doc
                .content_type
                .unwrap_or_else(|| "unknown".to_string());
            log::warn!("extract: {shown} serves {content_type}, which cannot convert");
            return Err(ExtractorError::UnsupportedContentType {
                url: final_url,
                content_type,
            });
        };
        let (title, markdown) = match kind {
            DocumentKind::Html => {
                let html = page.content();
                let title = page
                    .evaluate("document.title")
                    .as_str()
                    .unwrap_or("")
                    .to_string();
                let md = html_to_markdown(&html)
                    .map_err(|e| ExtractorError::Conversion(e.to_string()))?;
                (title, md)
            }
            DocumentKind::PlainText => (
                String::new(),
                String::from_utf8_lossy(&doc.body).into_owned(),
            ),
            DocumentKind::Pdf => (
                String::new(),
                pdf_to_markdown(&doc.body).map_err(ExtractorError::Conversion)?,
            ),
        };
        log::debug!(
            "extract: {shown} converted {kind:?} to {} bytes of markdown",
            markdown.len()
        );
        Ok(ExtractedContent {
            url: final_url,
            title,
            markdown,
        })
    }
}

impl PageExtractor for ObscuraPageExtractor {
    fn extract(&self, url: &str, timeout: Duration) -> Result<ExtractedContent, ExtractorError> {
        let shown = crate::clipped(url, 160);
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| {
                log::error!("extract: no runtime to load {shown} on: {e}");
                ExtractorError::Browser(e.to_string())
            })
            .and_then(|runtime| {
                runtime.block_on(async {
                    if let Ok(result) = tokio::time::timeout(timeout, Self::load(url)).await {
                        result
                    } else {
                        log::warn!("extract: {shown} timed out after {timeout:?}");
                        Err(ExtractorError::TimedOut {
                            url: url.to_string(),
                            timeout,
                        })
                    }
                })
            })
    }
}
