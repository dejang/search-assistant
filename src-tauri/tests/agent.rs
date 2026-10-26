#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use axum::Router;
    use axum::extract::State;
    use axum::http::StatusCode;
    use axum::response::IntoResponse;
    use axum::routing::post;
    use research_buddy_lib::agent::progress::{ResearchAction, ResearchProgress, Researcher};
    use research_buddy_lib::agent::tools::Tooling;
    use research_buddy_lib::agent::{Agent, AgentError, RunBudget};
    use research_buddy_lib::url_extractor::{
        ExtractedContent, ExtractorError, PageExtractor, UrlExtractor,
    };
    use serde_json::{Value, json};
    use tokio::net::TcpListener;

    struct StubPages;

    impl PageExtractor for StubPages {
        fn extract(
            &self,
            url: &str,
            _timeout: Duration,
        ) -> Result<ExtractedContent, ExtractorError> {
            Ok(ExtractedContent {
                url: url.to_string(),
                title: "Rust Home".to_string(),
                markdown: "Rust empowers everyone to build reliable software.".to_string(),
            })
        }
    }

    async fn spawn_api(router: Router) -> String {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind test listener");
        let addr = listener.local_addr().expect("local addr");
        drop(tokio::task::spawn(async move {
            let _ = axum::serve(listener, router).await;
        }));
        format!("http://{addr}/v1")
    }

    fn completion_json(content: &str) -> Value {
        json!({
            "id": "chatcmpl-test",
            "object": "chat.completion",
            "created": 1,
            "model": "qwen3-8b",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": content,
                },
                "finish_reason": "stop",
            }],
            "usage": {
                "prompt_tokens": 5,
                "completion_tokens": 2,
                "total_tokens": 7,
            }
        })
    }

    #[tokio::test]
    async fn ask_returns_the_providers_answer() {
        let router = Router::new().route(
            "/v1/chat/completions",
            post(|| async { axum::Json(completion_json("Four.")) }),
        );
        let base_url = spawn_api(router).await;
        let agent = Agent::new(base_url, "qwen3-8b").with_prompt("Answer briefly.");

        let answer = agent.ask("What is 2 + 2?").await.expect("answer");

        assert_eq!(answer, "Four.");
    }

    #[tokio::test]
    async fn an_empty_answer_is_reported_as_empty_error() {
        let router = Router::new().route(
            "/v1/chat/completions",
            post(|| async { axum::Json(completion_json("   ")) }),
        );
        let base_url = spawn_api(router).await;
        let agent = Agent::new(base_url, "qwen3-8b").with_prompt("Answer briefly.");

        let result = agent.ask("Say nothing").await;

        assert!(matches!(result, Err(AgentError::Empty)));
    }

    #[tokio::test]
    async fn a_provider_failure_is_reported_as_completion_error() {
        let router = Router::new().route(
            "/v1/chat/completions",
            post(|| async {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    axum::Json(json!({ "error": { "message": "upstream exploded" } })),
                )
            }),
        );
        let base_url = spawn_api(router).await;
        let agent = Agent::new(base_url, "qwen3-8b").with_prompt("Answer briefly.");

        let result = agent.ask("What is 2 + 2?").await;

        assert!(matches!(result, Err(AgentError::Completion { .. })));
    }

    async fn research_turn(
        State(turns): State<Arc<Mutex<usize>>>,
        axum::Json(body): axum::Json<Value>,
    ) -> impl IntoResponse {
        let current = {
            let mut count = turns.lock().expect("lock");
            let value = *count;
            *count += 1;
            value
        };
        let payload = serde_json::to_string(&body).expect("json");
        let reply = match current {
            0 => json!({
                "id": "lead-1",
                "object": "chat.completion",
                "created": 1,
                "model": "qwen3-8b",
                "choices": [{
                    "index": 0,
                    "message": {
                        "role": "assistant",
                        "content": Value::Null,
                        "tool_calls": [{
                            "id": "call_sub_1",
                            "type": "function",
                            "function": {
                                "name": "subagent",
                                "arguments": "{\"sub_question\":\"What is Rust?\",\"target_angle\":\"Angle A\",\"context\":\"Systems\",\"exclusions\":\"None\"}"
                            }
                        }]
                    },
                    "finish_reason": "tool_calls"
                }],
                "usage": { "prompt_tokens": 10, "completion_tokens": 5, "total_tokens": 15 }
            }),
            1 => json!({
                "id": "sub-1",
                "object": "chat.completion",
                "created": 1,
                "model": "qwen3-8b",
                "choices": [{
                    "index": 0,
                    "message": {
                        "role": "assistant",
                        "content": Value::Null,
                        "tool_calls": [
                            {
                                "id": "call_search_1",
                                "type": "function",
                                "function": {
                                    "name": "search",
                                    "arguments": "{\"query\":\"rust language\"}"
                                }
                            },
                            {
                                "id": "call_read_1",
                                "type": "function",
                                "function": {
                                    "name": "read_url",
                                    "arguments": "{\"url\":\"https://www.rust-lang.org/\"}"
                                }
                            }
                        ]
                    },
                    "finish_reason": "tool_calls"
                }],
                "usage": { "prompt_tokens": 10, "completion_tokens": 5, "total_tokens": 15 }
            }),
            2 => {
                assert!(
                    payload.contains("reliable software"),
                    "subagent must receive extracted page content"
                );
                completion_json("Digest: Rust builds reliable software [S1].")
            }
            _ => {
                assert!(
                    payload.contains("Digest: Rust builds reliable software"),
                    "lead agent must receive the subagent digest"
                );
                completion_json("Final report: Rust builds reliable software [S1].")
            }
        };
        axum::Json(reply)
    }

    #[tokio::test]
    async fn with_research_delegates_to_subagents_runs_tools_and_records_progress() {
        let turns = Arc::new(Mutex::new(0usize));
        let router = Router::new()
            .route("/v1/chat/completions", post(research_turn))
            .with_state(turns);
        let endpoint = spawn_api(router).await;

        let events = Arc::new(Mutex::new(Vec::new()));
        let recorded = Arc::clone(&events);
        let progress = Arc::new(ResearchProgress::new().on_activity(move |activity| {
            recorded.lock().expect("lock").push((
                activity.by,
                activity.action,
                activity.detail.to_string(),
                activity.searches,
                activity.reads,
                activity.subagents,
            ));
        }));
        let extractor = Arc::new(
            UrlExtractor::with_page_extractor(StubPages)
                .with_search_base("http://127.0.0.1:9/html/"),
        );
        let agent = Agent::new(endpoint, "qwen3-8b")
            .with_budget(RunBudget {
                turns: 6,
                retries: 2,
                tools: 2,
            })
            .with_tooling(Tooling::new(extractor, Arc::clone(&progress), 8_000));

        let _guard = progress.begin().expect("run slot");
        let report = agent
            .ask("Tell me about Rust.")
            .await
            .expect("research report");

        assert_eq!(report, "Final report: Rust builds reliable software [S1].");

        let log = events.lock().expect("lock").clone();
        assert_eq!(log.len(), 3);
        assert_eq!(
            log[0],
            (
                Researcher::Lead,
                ResearchAction::Delegating,
                "What is Rust?".to_string(),
                0,
                0,
                1,
            )
        );
        let last = log.last().expect("final activity");
        assert_eq!((last.3, last.4, last.5), (1, 1, 1));
    }
}
