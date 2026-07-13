use dyyl::ai::{AiError, AiErrorKind, AiProviderKind};

#[test]
fn provider_kind_from_choice() {
    assert_eq!(
        AiProviderKind::from_choice(1),
        Some(AiProviderKind::OpenaiChat)
    );
    assert_eq!(
        AiProviderKind::from_choice(2),
        Some(AiProviderKind::OpenaiResponse)
    );
    assert_eq!(
        AiProviderKind::from_choice(3),
        Some(AiProviderKind::Anthropic)
    );
    assert_eq!(AiProviderKind::from_choice(4), None);
}

#[test]
fn ai_error_display() {
    let err = AiError::new(AiErrorKind::Auth, "invalid api key".to_string(), Some(401));
    assert!(err.to_string().contains("Auth"));
    assert!(err.to_string().contains("invalid api key"));
    assert_eq!(err.status, Some(401));
}

// ── Task 4: HTTP client retry tests ──────────────────────────────────

use dyyl::ai::client::{HttpRequest, HttpResponse, http_request_with_retry};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

#[test]
fn retry_succeeds_on_second_attempt() {
    let attempts = Arc::new(AtomicUsize::new(0));
    let counter = attempts.clone();
    let result = http_request_with_retry(
        HttpRequest {
            url: "http://example.invalid".to_string(),
            method: "POST".to_string(),
            headers: vec![],
            body: String::new(),
        },
        Duration::from_millis(1),
        3,
        Duration::from_millis(1),
        Box::new(move |_req| {
            let n = counter.fetch_add(1, Ordering::SeqCst);
            if n == 0 {
                Err(dyyl::ai::AiError::new(
                    dyyl::ai::AiErrorKind::ServerError,
                    "500".to_string(),
                    Some(500),
                ))
            } else {
                Ok(HttpResponse {
                    status: 200,
                    body: "ok".to_string(),
                })
            }
        }),
    );
    assert!(result.is_ok());
    assert_eq!(result.unwrap().body, "ok");
    assert_eq!(attempts.load(Ordering::SeqCst), 2);
}

#[test]
fn retry_exhausted_returns_error() {
    let result = http_request_with_retry(
        HttpRequest {
            url: "http://example.invalid".to_string(),
            method: "POST".to_string(),
            headers: vec![],
            body: String::new(),
        },
        Duration::from_millis(1),
        2,
        Duration::from_millis(1),
        Box::new(|_req| {
            Err(dyyl::ai::AiError::new(
                dyyl::ai::AiErrorKind::ServerError,
                "always fails".to_string(),
                Some(500),
            ))
        }),
    );
    assert!(result.is_err());
}

#[test]
fn no_retry_on_4xx_auth() {
    let attempts = Arc::new(AtomicUsize::new(0));
    let counter = attempts.clone();
    let result = http_request_with_retry(
        HttpRequest {
            url: "http://example.invalid".to_string(),
            method: "POST".to_string(),
            headers: vec![],
            body: String::new(),
        },
        Duration::from_millis(1),
        3,
        Duration::from_millis(1),
        Box::new(move |_req| {
            counter.fetch_add(1, Ordering::SeqCst);
            Err(dyyl::ai::AiError::new(
                dyyl::ai::AiErrorKind::Auth,
                "401".to_string(),
                Some(401),
            ))
        }),
    );
    assert!(result.is_err());
    assert_eq!(
        attempts.load(Ordering::SeqCst),
        1,
        "4xx auth errors should not retry"
    );
}

#[test]
fn retry_on_rate_limit_429() {
    let attempts = Arc::new(AtomicUsize::new(0));
    let counter = attempts.clone();
    let result = http_request_with_retry(
        HttpRequest {
            url: "http://example.invalid".to_string(),
            method: "POST".to_string(),
            headers: vec![],
            body: String::new(),
        },
        Duration::from_millis(1),
        2,
        Duration::from_millis(1),
        Box::new(move |_req| {
            let n = counter.fetch_add(1, Ordering::SeqCst);
            if n < 2 {
                Err(dyyl::ai::AiError::new(
                    dyyl::ai::AiErrorKind::RateLimit,
                    "429".to_string(),
                    Some(429),
                ))
            } else {
                Ok(HttpResponse {
                    status: 200,
                    body: "ok".to_string(),
                })
            }
        }),
    );
    assert!(result.is_ok());
    assert_eq!(attempts.load(Ordering::SeqCst), 3);
}

// ── Task 5: OpenAI Chat Completions provider tests ──────────────────

use dyyl::ai::provider_openai_chat::OpenaiChatProvider;
use dyyl::ai::AiProvider;

#[test]
fn openai_chat_builds_correct_request_body() {
    let provider = OpenaiChatProvider::new(
        "sk-test".to_string(),
        "gpt-4o-mini".to_string(),
        String::new(),
    );
    let req = provider.build_request("You are helpful", "What is 2+2?");
    assert_eq!(req.method, "POST");
    assert!(req.url.ends_with("/chat/completions"));
    assert!(req.headers.iter().any(|(k, v)| k == "Authorization" && v == "Bearer sk-test"));
    let body: serde_json::Value = serde_json::from_str(&req.body).expect("valid json");
    assert_eq!(body["model"], "gpt-4o-mini");
    assert_eq!(body["messages"][0]["role"], "system");
    assert_eq!(body["messages"][0]["content"], "You are helpful");
    assert_eq!(body["messages"][1]["role"], "user");
    assert_eq!(body["messages"][1]["content"], "What is 2+2?");
}

#[test]
fn openai_chat_parses_response() {
    let provider = OpenaiChatProvider::new(
        "sk-test".to_string(),
        "gpt-4o-mini".to_string(),
        String::new(),
    );
    let resp_body = r#"{"choices":[{"message":{"content":"4"}}]}"#;
    let result = provider.parse_response(resp_body);
    assert_eq!(result, Ok("4".to_string()));
}

#[test]
fn openai_chat_default_base_url() {
    let provider = OpenaiChatProvider::new(
        "sk-test".to_string(),
        "gpt-4o-mini".to_string(),
        String::new(),
    );
    let req = provider.build_request("sys", "usr");
    assert_eq!(req.url, "https://api.openai.com/v1/chat/completions");
}

#[test]
fn openai_chat_custom_base_url() {
    let provider = OpenaiChatProvider::new(
        "sk-test".to_string(),
        "gpt-4o-mini".to_string(),
        "http://localhost:8080".to_string(),
    );
    let req = provider.build_request("sys", "usr");
    assert_eq!(req.url, "http://localhost:8080/chat/completions");
}
