use std::sync::Arc;

use rig_core::http_client::{
    Bytes, HeaderValue, HttpClientExt, LazyBody, MultipartForm, Request, Response, StatusCode,
    StreamingResponse,
};
use serde::Deserialize;
use serde_json::{Value, json};

use super::{EngineError, LocalEngine, RequestBuilder};

#[derive(Clone)]
pub struct EngineClient {
    engine: Arc<LocalEngine>,
}

impl EngineClient {
    #[must_use]
    pub fn new(engine: Arc<LocalEngine>) -> Self {
        Self { engine }
    }

    async fn answer(&self, path: &str, body: &[u8]) -> Result<Value, EngineError> {
        if !path.ends_with("/v1/chat/completions") {
            return Err(EngineError::Request {
                reason: format!("the local model answers no {path}"),
            });
        }
        let request = Self::request(body)?;
        self.engine.complete(request).await
    }

    pub(super) fn request(body: &[u8]) -> Result<RequestBuilder, EngineError> {
        let refused = |reason: String| EngineError::Request { reason };
        let parsed: WireRequest = serde_json::from_slice(body)
            .map_err(|error| refused(format!("the request could not be read: {error}")))?;
        if parsed.messages.is_empty() {
            return Err(refused("the request carried no messages".into()));
        }

        let mut request = RequestBuilder::new();
        for message in parsed.messages {
            let text = Self::text_of(&message.content)
                .ok_or_else(|| refused("only text content reaches the local model".to_string()))?;
            request = request.add_message(Self::role(&message.role), text);
        }
        Ok(request)
    }

    fn text_of(content: &Value) -> Option<String> {
        if let Some(text) = content.as_str() {
            return Some(text.to_owned());
        }
        let parts = content.as_array()?;
        let mut text = String::new();
        for part in parts {
            let piece = part
                .get("text")
                .and_then(Value::as_str)
                .or_else(|| part.as_str())?;
            text.push_str(piece);
        }
        (!text.is_empty()).then_some(text)
    }

    fn role(role: &str) -> mistralrs::TextMessageRole {
        match role {
            "system" | "developer" => mistralrs::TextMessageRole::System,
            "assistant" => mistralrs::TextMessageRole::Assistant,
            "tool" => mistralrs::TextMessageRole::Tool,
            other => mistralrs::TextMessageRole::Custom(other.to_owned()),
        }
    }

    pub(super) fn reply<U: From<Bytes>>(
        outcome: Result<Value, EngineError>,
    ) -> Response<LazyBody<U>> {
        let mut status = match &outcome {
            Ok(_) => StatusCode::OK,
            Err(error) => error.status(),
        };
        let body = match outcome {
            Ok(completion) => completion,
            Err(error) => Self::error_document(&error),
        };
        let encoded = serde_json::to_vec(&body).or_else(|_| {
            status = StatusCode::INTERNAL_SERVER_ERROR;
            serde_json::to_vec(&Self::error_document(&EngineError::Generation {
                reason: "the answer could not be encoded".into(),
            }))
        });
        let bytes = Bytes::from(encoded.unwrap_or_default());

        let mut response =
            Response::new(Box::pin(async move { Ok(U::from(bytes)) }) as LazyBody<U>);
        *response.status_mut() = status;
        response
            .headers_mut()
            .insert("content-type", HeaderValue::from_static("application/json"));
        response
    }

    fn error_document(error: &EngineError) -> Value {
        let kind = match error {
            EngineError::Request { .. } => "invalid_request_error",
            EngineError::Load { .. } | EngineError::Panicked | EngineError::Unavailable => {
                "engine_unavailable"
            }
            EngineError::Generation { .. } => "server_error",
            EngineError::TimedOut { .. } => "timeout",
        };
        json!({ "error": { "message": error.to_string(), "type": kind } })
    }

    fn refusal(why: &str) -> rig_core::http_client::Error {
        rig_core::http_client::Error::instance(std::io::Error::other(why.to_owned()))
    }
}

impl HttpClientExt for EngineClient {
    fn send<T, U>(
        &self,
        request: Request<T>,
    ) -> impl std::future::Future<
        Output = Result<Response<LazyBody<U>>, rig_core::http_client::Error>,
    > + Send
    + 'static
    where
        T: Into<Bytes> + Send,
        U: From<Bytes> + Send + 'static,
    {
        let client = self.clone();
        let (parts, body) = request.into_parts();
        let path = parts.uri.path().to_owned();
        let body = body.into();

        async move {
            let outcome = client.answer(&path, &body).await;
            Ok(EngineClient::reply(outcome))
        }
    }

    fn send_multipart<U>(
        &self,
        _request: Request<MultipartForm>,
    ) -> impl std::future::Future<
        Output = Result<Response<LazyBody<U>>, rig_core::http_client::Error>,
    > + Send
    + 'static
    where
        U: From<Bytes> + Send + 'static,
    {
        std::future::ready(Err(Self::refusal("the local model reads no files")))
    }

    fn send_streaming<T>(
        &self,
        _request: Request<T>,
    ) -> impl std::future::Future<Output = Result<StreamingResponse, rig_core::http_client::Error>> + Send
    where
        T: Into<Bytes> + Send,
    {
        std::future::ready(Err(Self::refusal("the local model answers in one piece")))
    }
}

#[derive(Deserialize)]
struct WireRequest {
    messages: Vec<WireMessage>,
}

#[derive(Deserialize)]
struct WireMessage {
    role: String,
    content: Value,
}
