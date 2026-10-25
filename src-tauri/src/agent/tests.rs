//! Tests for the agent module: unit tests pin the error contract the UI renders;

#[must_use]
fn request_line(request: &str) -> &str {
    request.lines().next().unwrap_or_default()
}

#[cfg(test)]
mod construction {
    use crate::agent::BuddyAgent;

    #[test]
    fn records_the_model_it_was_configured_with() {
        let agent = BuddyAgent::with_api_key("sk-test", "gpt-4o-mini", "Answer briefly.");
        assert_eq!(agent.model(), "gpt-4o-mini");
    }

    #[test]
    fn an_explicit_endpoint_does_not_override_the_model() {
        let agent =
            BuddyAgent::with_endpoint("sk-test", "http://127.0.0.1:9/v1", "llama", "Be brief.");
        assert_eq!(agent.model(), "llama");
    }
}

#[cfg(test)]
mod error_contract {
    use crate::agent::AgentError;
    use rig_core::client::env::EnvError;
    use std::time::Duration;

    #[test]
    fn display_strings_are_pinned() {
        assert_eq!(
            AgentError::Empty.to_string(),
            "the provider returned an empty answer"
        );
        assert_eq!(
            AgentError::TimedOut {
                timeout: Duration::from_secs(30),
            }
            .to_string(),
            "the provider did not answer within 30s"
        );
    }

    #[test]
    fn the_config_error_keeps_its_source_chain() {
        let name = "RESEARCH_BUDDY_DEFINITELY_UNSET_VAR";
        let source = EnvError::Variable {
            name,
            source: std::env::var(name)
                .err()
                .unwrap_or_else(|| panic!("{name} must be unset for this fixture")),
        };

        let error = AgentError::Config { source };
        assert!(
            std::error::Error::source(&error).is_some(),
            "AgentError must expose the underlying EnvError as its source, \
             otherwise callers cannot branch on the failure"
        );
    }

    #[test]
    fn only_an_answered_request_has_a_status() {
        let name = "RESEARCH_BUDDY_DEFINITELY_UNSET_VAR";
        let config = AgentError::Config {
            source: EnvError::Variable {
                name,
                source: std::env::var(name)
                    .err()
                    .unwrap_or_else(|| panic!("{name} must be unset for this fixture")),
            },
        };

        assert_eq!(
            config.status(),
            None,
            "an unanswered config error has no status"
        );
        assert_eq!(AgentError::Empty.status(), None);
        assert_eq!(
            AgentError::TimedOut {
                timeout: Duration::from_secs(1),
            }
            .status(),
            None
        );
    }

    #[test]
    fn serializes_as_the_display_string_for_ipc() {
        let json = serde_json::to_string(&AgentError::Empty).unwrap();
        assert_eq!(json, "\"the provider returned an empty answer\"");
    }
}

#[cfg(test)]
mod endpoint {
    use super::request_line;
    use crate::agent::{AgentError, BuddyAgent};
    use std::future::Future;
    use std::time::Duration;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::{TcpListener, TcpStream};

    /// Read a full request: headers to the blank line, then the declared body.
    async fn read_request(stream: &mut TcpStream) -> String {
        let mut buf: Vec<u8> = Vec::new();
        let mut chunk = [0u8; 1024];
        let mut note = String::new();

        let head_end = loop {
            match stream.read(&mut chunk).await {
                Ok(0) => break buf.len(),
                Ok(read) => {
                    buf.extend_from_slice(&chunk[..read]);
                    // `windows` panics when the slice is shorter than 4, so the
                    // length guard comes first.
                    if buf.len() >= 4
                        && let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n")
                    {
                        break pos + 4;
                    }
                }
                // A read error is reported through the returned request, so a
                // broken harness fails with a reason rather than an empty
                // "unexpected request target: ".
                Err(error) => {
                    note = format!("\n<harness: header read failed: {error}>");
                    break buf.len();
                }
            }
        };

        let declared = String::from_utf8_lossy(&buf[..head_end])
            .to_ascii_lowercase()
            .lines()
            .find_map(|line| line.strip_prefix("content-length:"))
            .and_then(|value| value.trim().parse::<usize>().ok())
            .unwrap_or(0);

        while buf.len() - head_end < declared {
            match stream.read(&mut chunk).await {
                Ok(0) => break,
                Ok(read) => {
                    let take = read.min(declared - (buf.len() - head_end));
                    buf.extend_from_slice(&chunk[..take]);
                }
                Err(error) => {
                    note = format!("\n<harness: body read failed: {error}>");
                    break;
                }
            }
        }

        let request = String::from_utf8_lossy(&buf).into_owned();
        if note.is_empty() {
            request
        } else {
            format!("{request}{note}")
        }
    }

    /// Answer one request with `status`, without ever being a real provider.
    /// Returns the base URL to point an agent at, and a future yielding the
    /// request it received.
    async fn serve_once(
        status: u16,
        reason: &str,
        body: &str,
    ) -> (String, impl Future<Output = String>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let body = body.to_string();

        let serve = async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let request = read_request(&mut stream).await;

            let head = format!(
                "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\n\
                 Content-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let _ = stream.write_all(head.as_bytes()).await;
            let _ = stream.write_all(body.as_bytes()).await;
            let _ = stream.flush().await;
            request
        };

        (format!("http://{addr}/v1"), serve)
    }

    /// Run one exchange under a hard ceiling, so a routing regression fails the
    /// test instead of hanging CI on an unbounded `accept`.
    async fn drive(
        agent: &BuddyAgent,
        prompt: &str,
        serve: impl Future<Output = String>,
    ) -> (Result<String, AgentError>, String) {
        tokio::time::timeout(Duration::from_secs(20), async {
            tokio::join!(agent.ask_within(prompt, Duration::from_secs(10)), serve)
        })
        .await
        .unwrap_or_else(|_| panic!("the exchange did not finish inside the test ceiling"))
    }

    #[tokio::test]
    async fn ask_sends_the_request_to_the_configured_endpoint() {
        let (base_url, serve) = serve_once(
            500,
            "Internal Server Error",
            r#"{"error":{"message":"upstream exploded"}}"#,
        )
        .await;

        let agent =
            BuddyAgent::with_endpoint("sk-test", base_url, "gpt-4o-mini", "Answer briefly.");
        let (result, request) = drive(&agent, "What is 2 + 2?", serve).await;

        assert!(
            request.contains("chat/completions"),
            "unexpected request target: {}",
            request_line(&request)
        );
        assert!(
            request
                .to_ascii_lowercase()
                .contains("authorization: bearer"),
            "no bearer credential was sent: {}",
            request_line(&request)
        );

        let error = match result {
            Err(error) => error,
            Ok(answer) => panic!("expected the failed request to error, got {answer:?}"),
        };
        assert!(
            matches!(error, AgentError::Completion { .. }),
            "a provider 500 must map to Completion, got: {error}"
        );
        // Proves the failure came from an answered 500, not from a transport
        // error the local harness caused by answering too early.
        assert_eq!(error.status(), Some(500), "expected an answered 500");
    }

    #[tokio::test]
    async fn ask_returns_the_providers_answer() {
        let (base_url, serve) = serve_once(
            200,
            "OK",
            r#"{"id":"chatcmpl-test","object":"chat.completion","created":1,
                "model":"gpt-4o-mini",
                "choices":[{"index":0,"message":{"role":"assistant","content":"Four."},
                "finish_reason":"stop"}],
                "usage":{"prompt_tokens":5,"completion_tokens":2,"total_tokens":7}}"#,
        )
        .await;

        let agent =
            BuddyAgent::with_endpoint("sk-test", base_url, "gpt-4o-mini", "Answer briefly.");
        let (result, _) = drive(&agent, "What is 2 + 2?", serve).await;

        match result {
            Ok(answer) => assert_eq!(answer, "Four."),
            Err(error) => panic!("expected the answer to come back, got: {error}"),
        }
    }

    #[tokio::test]
    async fn an_answer_with_no_text_is_an_error_not_a_blank_success() {
        let (base_url, serve) = serve_once(
            200,
            "OK",
            r#"{"id":"chatcmpl-empty","object":"chat.completion","created":1,
                "model":"gpt-4o-mini",
                "choices":[{"index":0,"message":{"role":"assistant","content":""},
                "finish_reason":"stop"}],
                "usage":{"prompt_tokens":5,"completion_tokens":0,"total_tokens":5}}"#,
        )
        .await;

        let agent =
            BuddyAgent::with_endpoint("sk-test", base_url, "gpt-4o-mini", "Answer briefly.");
        let (result, _) = drive(&agent, "Say nothing", serve).await;

        assert!(
            matches!(result, Err(AgentError::Empty)),
            "an empty reply must surface as Empty, got: {result:?}"
        );
    }

    #[tokio::test]
    async fn ask_within_reports_a_deadline_instead_of_hanging() {
        // A listener that never accepts: the handshake completes in the kernel
        // backlog and no response is ever written, which is the failure mode a
        // hung gateway produces. An unbounded `ask` would wait on it forever.
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let agent = BuddyAgent::with_endpoint(
            "sk-test",
            format!("http://{addr}/v1"),
            "gpt-4o-mini",
            "Answer briefly.",
        );

        let result = tokio::time::timeout(
            Duration::from_millis(3000),
            agent.ask_within("hello", Duration::from_millis(500)),
        )
        .await
        .unwrap_or_else(|_| panic!("ask_within must respect its own deadline"));

        match result {
            Err(AgentError::TimedOut { timeout }) => {
                assert_eq!(timeout, Duration::from_millis(500));
            }
            other => panic!("expected TimedOut, got: {other:?}"),
        }

        drop(listener);
    }
}

#[cfg(test)]
mod in_process {
    use crate::agent::BuddyAgent;
    use rig_core::http_client::{
        Bytes, HeaderValue, HttpClientExt, LazyBody, MultipartForm, Request, Response, StatusCode,
        StreamingResponse,
    };

    #[derive(Clone)]
    struct Engine {
        status: u16,
        document: &'static str,
    }

    impl HttpClientExt for Engine {
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
            assert!(
                request.uri().path().ends_with("/chat/completions"),
                "the wire asked for {}",
                request.uri(),
            );

            let bytes = Bytes::from_static(self.document.as_bytes());
            let status = StatusCode::from_u16(self.status).unwrap();
            async move {
                let mut response =
                    Response::new(Box::pin(async move { Ok(U::from(bytes)) }) as LazyBody<U>);
                *response.status_mut() = status;
                response
                    .headers_mut()
                    .insert("content-type", HeaderValue::from_static("application/json"));
                Ok(response)
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
            std::future::ready(Err(rig_core::http_client::Error::instance(
                std::io::Error::other("no files here"),
            )))
        }

        fn send_streaming<T>(
            &self,
            _request: Request<T>,
        ) -> impl std::future::Future<
            Output = Result<StreamingResponse, rig_core::http_client::Error>,
        > + Send
        where
            T: Into<Bytes> + Send,
        {
            std::future::ready(Err(rig_core::http_client::Error::instance(
                std::io::Error::other("one piece"),
            )))
        }
    }

    #[tokio::test]
    async fn the_agent_answers_from_a_client_in_this_process() {
        let answer = r#"{"id":"chatcmpl-local","object":"chat.completion","created":1,
            "model":"qwen3-8b",
            "choices":[{"index":0,"message":{"role":"assistant","content":"Four."},
            "finish_reason":"stop"}],
            "usage":{"prompt_tokens":5,"completion_tokens":2,"total_tokens":7}}"#;
        let agent = BuddyAgent::with_client(
            Engine {
                status: 200,
                document: answer,
            },
            "qwen3-8b",
            "Answer briefly.",
        );

        assert_eq!(agent.ask("What is 2 + 2?").await.unwrap(), "Four.");
    }

    #[tokio::test]
    async fn an_in_process_failure_keeps_the_status_the_ui_branches_on() {
        let refusal = r#"{"error":{"message":"the model is not answering right now",
            "type":"engine_unavailable"}}"#;
        let agent = BuddyAgent::with_client(
            Engine {
                status: 503,
                document: refusal,
            },
            "qwen3-8b",
            "Answer briefly.",
        );

        let error = agent
            .ask("What is 2 + 2?")
            .await
            .expect_err("a dead engine fails the turn");

        assert_eq!(error.status(), Some(503));
        assert!(
            error.to_string().contains("not answering"),
            "the sentence the webview shows carries the engine's reason: {error}"
        );
    }
}

#[cfg(test)]
mod tool_concurrency {
    use std::future::Future;
    use std::sync::Arc;
    use std::time::Duration;

    use rig_agent::AgentBuilder;
    use rig_core::http_client::{
        Bytes, HeaderValue, HttpClientExt, LazyBody, MultipartForm, Request, Response, StatusCode,
        StreamingResponse,
    };
    use rig_core::providers::openai::OpenAIConfig;
    use rig_core::tool::{Tool, ToolContext};
    use rig_core::wasm_compat::WasmCompatSend;
    use serde_json::{Value, json};
    use tokio::sync::Barrier;

    use crate::agent::{AgentError, BuddyAgent, RunBudget};

    #[derive(Clone)]
    struct TwoCalls;

    impl HttpClientExt for TwoCalls {
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
            let body: Bytes = request.into_body().into();
            let answered = body
                .windows(b"tool_call_id".len())
                .any(|window| window == b"tool_call_id");
            let document = if answered {
                r#"{"id":"c2","object":"chat.completion","created":1,"model":"qwen3-8b",
                "choices":[{"index":0,"message":{"role":"assistant","content":"Both met."},
                "finish_reason":"stop"}],
                "usage":{"prompt_tokens":5,"completion_tokens":2,"total_tokens":7}}"#
            } else {
                r#"{"id":"c1","object":"chat.completion","created":1,"model":"qwen3-8b",
                "choices":[{"index":0,"message":{"role":"assistant","content":null,
                "tool_calls":[
                {"id":"call_a","type":"function","function":{"name":"meet","arguments":"{}"}},
                {"id":"call_b","type":"function","function":{"name":"meet","arguments":"{}"}}]},
                "finish_reason":"tool_calls"}],
                "usage":{"prompt_tokens":5,"completion_tokens":2,"total_tokens":7}}"#
            };
            let bytes = Bytes::from_static(document.as_bytes());
            async move {
                let mut response =
                    Response::new(Box::pin(async move { Ok(U::from(bytes)) }) as LazyBody<U>);
                *response.status_mut() = StatusCode::OK;
                response
                    .headers_mut()
                    .insert("content-type", HeaderValue::from_static("application/json"));
                Ok(response)
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
            std::future::ready(Err(rig_core::http_client::Error::instance(
                std::io::Error::other("no files here"),
            )))
        }

        fn send_streaming<T>(
            &self,
            _request: Request<T>,
        ) -> impl std::future::Future<
            Output = Result<StreamingResponse, rig_core::http_client::Error>,
        > + Send
        where
            T: Into<Bytes> + Send,
        {
            std::future::ready(Err(rig_core::http_client::Error::instance(
                std::io::Error::other("one piece"),
            )))
        }
    }

    struct Meet {
        barrier: Arc<Barrier>,
    }

    impl Tool for Meet {
        const NAME: &'static str = "meet";
        type Args = Value;
        type Output = String;
        type Error = AgentError;

        fn description(&self) -> String {
            "Waits until its sibling call arrives.".to_string()
        }

        fn parameters(&self) -> Value {
            json!({ "type": "object", "properties": {} })
        }

        fn call(
            &self,
            _context: &mut ToolContext,
            _args: Value,
        ) -> impl Future<Output = Result<String, AgentError>> + WasmCompatSend {
            let barrier = Arc::clone(&self.barrier);
            async move {
                barrier.wait().await;
                Ok("met".to_string())
            }
        }
    }

    #[tokio::test]
    async fn the_calls_of_one_turn_run_side_by_side() {
        let provider = OpenAIConfig::new("local")
            .with_base_url("http://local-engine/v1")
            .connect(TwoCalls);
        let agent = BuddyAgent {
            inner: AgentBuilder::new(provider.chat("qwen3-8b"))
                .tool(Meet {
                    barrier: Arc::new(Barrier::new(2)),
                })
                .build(),
            model: "qwen3-8b".to_string(),
            budget: RunBudget {
                turns: 3,
                retries: 3,
                tools: 2,
            },
        };

        let answer = tokio::time::timeout(Duration::from_secs(5), agent.ask("Meet twice."))
            .await
            .expect("two calls that wait on each other finish only when they run together");

        assert_eq!(answer.unwrap(), "Both met.");
    }
}
