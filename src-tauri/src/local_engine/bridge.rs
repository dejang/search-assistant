use std::sync::Arc;

use rig_core::http_client::{
    Bytes, HeaderValue, HttpClientExt, LazyBody, MultipartForm, Request, Response, StatusCode,
    StreamingResponse,
};
use serde_json::{Value, json};

use super::{EngineError, LocalEngine};

#[derive(Clone)]
pub struct EngineClient {
    engine: Arc<LocalEngine>,
}

impl EngineClient {
    #[must_use]
    pub fn new(engine: Arc<LocalEngine>) -> Self {
        Self { engine }
    }

    async fn answer(&self, path: &str, body: Bytes) -> Result<(StatusCode, Bytes), EngineError> {
        if !path.ends_with("/v1/chat/completions") {
            return Err(EngineError::Request {
                reason: format!("the local model answers no {path}"),
            });
        }
        self.engine.complete(body).await
    }

    pub(super) fn reply<U: From<Bytes>>(
        outcome: Result<(StatusCode, Bytes), EngineError>,
    ) -> Response<LazyBody<U>> {
        let (status, bytes) = match outcome {
            Ok(reply) => reply,
            Err(error) => (
                error.status(),
                Bytes::from(serde_json::to_vec(&Self::error_document(&error)).unwrap_or_default()),
            ),
        };

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
            EngineError::Load { .. } | EngineError::Stopped | EngineError::Unavailable => {
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
            let outcome = client.answer(&path, body).await;
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
