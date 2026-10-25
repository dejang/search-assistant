use super::bridge::EngineClient;
use super::{EngineError, EnginePhase, EngineStatus, LocalEngine};
use rig_core::http_client::{Bytes, StatusCode};
use serde_json::{Value, json};

#[cfg(test)]
mod contract {
    use super::*;

    async fn wire_reply(outcome: Result<Value, EngineError>) -> (StatusCode, Value) {
        let response = EngineClient::reply::<Bytes>(outcome);
        let status = response.status();
        let bytes = response
            .into_body()
            .await
            .expect("the engine's reply to be readable");
        (
            status,
            serde_json::from_slice(&bytes).expect("a json reply"),
        )
    }

    #[test]
    fn each_engine_failure_names_the_status_the_agent_reports() {
        let refused = EngineError::Request {
            reason: "too long".into(),
        };
        let died = EngineError::Generation {
            reason: "metal said no".into(),
        };
        let absent = EngineError::Load {
            reason: "no file".into(),
        };

        assert_eq!(refused.status(), StatusCode::BAD_REQUEST);
        assert_eq!(died.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(absent.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(
            EngineError::Panicked.status(),
            StatusCode::SERVICE_UNAVAILABLE
        );
        assert_eq!(
            EngineError::Unavailable.status(),
            StatusCode::SERVICE_UNAVAILABLE
        );
        assert_eq!(
            EngineError::TimedOut {
                timeout: std::time::Duration::from_secs(120),
            }
            .status(),
            StatusCode::GATEWAY_TIMEOUT
        );
    }

    #[tokio::test]
    async fn a_finished_turn_reaches_the_wire_as_the_completion_document() {
        let completion = json!({ "choices": [{ "message": { "content": "Paris" } }] });

        let (status, body) = wire_reply(Ok(completion.clone())).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, completion);
    }

    #[tokio::test]
    async fn an_engine_failure_reaches_the_wire_as_an_openai_error() {
        let (status, body) = wire_reply(Err(EngineError::Request {
            reason: "only text reaches the local model".into(),
        }))
        .await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"]["type"], "invalid_request_error");
        assert!(
            body["error"]["message"]
                .as_str()
                .is_some_and(|text| text.contains("only text")),
            "the sentence the webview shows carries the cause"
        );
    }

    #[tokio::test]
    async fn a_dead_engine_is_retryable_and_a_broken_answer_is_not() {
        let (unavailable, _) = wire_reply(Err(EngineError::Unavailable)).await;
        let (load, _) = wire_reply(Err(EngineError::Load {
            reason: "no chat template".into(),
        }))
        .await;
        let (generation, _) = wire_reply(Err(EngineError::Generation {
            reason: "halted mid-sentence".into(),
        }))
        .await;

        assert_eq!(unavailable, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(load, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(generation, StatusCode::INTERNAL_SERVER_ERROR);
    }

    fn wire(messages: &Value) -> Vec<u8> {
        serde_json::to_vec(&json!({ "model": "qwen3-8b", "messages": messages })).expect("json")
    }

    #[test]
    fn a_chat_turn_the_wire_carries_reaches_the_engine() {
        let body = wire(&json!([
            { "role": "system", "content": "Answer briefly." },
            { "role": "user", "content": [{ "type": "text", "text": "Capital of France?" }] },
        ]));

        assert!(EngineClient::request(&body).is_ok(), "text parts mean text");
    }

    #[test]
    fn a_turn_the_engine_cannot_answer_is_refused_before_any_work() {
        let empty = wire(&json!([]));
        let image = wire(&json!([{ "role": "user", "content": [{ "type": "image_url" }] }]));
        let other_role = wire(&json!([{ "role": "developer", "content": "hi" }]));

        assert!(
            EngineClient::request(&empty).is_err(),
            "no messages is no question"
        );
        assert!(
            EngineClient::request(&image).is_err(),
            "a vision part would be answered as an empty turn"
        );
        assert!(EngineClient::request(b"not json").is_err());
        assert!(EngineClient::request(&other_role).is_ok());
    }

    #[test]
    fn a_message_with_nothing_in_it_is_refused_too() {
        let bare = wire(&json!([{ "role": "user", "content": [] }]));
        let null = wire(&json!([{ "role": "user", "content": null }]));

        assert!(
            EngineClient::request(&bare).is_err(),
            "no parts is no message"
        );
        assert!(EngineClient::request(&null).is_err());
    }

    #[test]
    fn only_a_quiet_and_stale_idle_timer_frees_the_weights() {
        assert!(LocalEngine::quiescent(0, 7, 7));
        assert!(!LocalEngine::quiescent(0, 7, 8));
        assert!(!LocalEngine::quiescent(1, 7, 7));
        assert!(!LocalEngine::quiescent(3, 7, 8));
    }

    #[test]
    fn the_status_serializes_as_the_webview_reads_it() {
        let status = EngineStatus {
            phase: EnginePhase::Failed,
            message: Some("the model could not be loaded".into()),
        };

        let json = serde_json::to_value(status).expect("a serializable status");

        assert_eq!(json["phase"], "failed");
        assert_eq!(json["message"], "the model could not be loaded");
    }

    #[test]
    fn a_status_that_says_the_same_thing_is_not_news() {
        assert_eq!(
            EngineStatus::of(EnginePhase::Ready),
            EngineStatus::of(EnginePhase::Ready)
        );
        assert_ne!(
            EngineStatus::of(EnginePhase::Ready),
            EngineStatus {
                phase: EnginePhase::Ready,
                message: Some("later".into()),
            }
        );
    }
}
