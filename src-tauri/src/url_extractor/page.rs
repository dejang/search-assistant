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
        let captured = Self::observe_document(&mut page);

        page.goto(url)
            .await
            .map_err(|e| ExtractorError::Navigation {
                url: url.to_string(),
                reason: e.to_string(),
            })?;

        let final_url = page.url();
        let doc = captured
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()
            .ok_or_else(|| ExtractorError::Navigation {
                url: url.to_string(),
                reason: "no document response received".to_string(),
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
        if !(200..300).contains(&doc.status) {
            return Err(ExtractorError::HttpStatus {
                url: final_url,
                status: doc.status,
            });
        }
        let Some(kind) = classify_mime(doc.content_type.as_deref()) else {
            return Err(ExtractorError::UnsupportedContentType {
                url: final_url,
                content_type: doc.content_type.unwrap_or_else(|| "unknown".to_string()),
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
        Ok(ExtractedContent {
            url: final_url,
            title,
            markdown,
        })
    }
}

impl PageExtractor for ObscuraPageExtractor {
    fn extract(&self, url: &str, timeout: Duration) -> Result<ExtractedContent, ExtractorError> {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| ExtractorError::Browser(e.to_string()))
            .and_then(|runtime| {
                runtime.block_on(async {
                    if let Ok(result) = tokio::time::timeout(timeout, Self::load(url)).await {
                        result
                    } else {
                        Err(ExtractorError::TimedOut {
                            url: url.to_string(),
                            timeout,
                        })
                    }
                })
            })
    }
}
