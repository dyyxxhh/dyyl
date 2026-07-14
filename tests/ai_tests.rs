#![allow(
    clippy::all,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::panic,
    clippy::expect_used,
    clippy::todo,
    clippy::unimplemented,
    clippy::as_underscore,
    clippy::fn_to_numeric_cast_any,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::redundant_pub_crate,
    clippy::missing_const_for_fn
)]
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

use dyyl::ai::client::{http_request_with_retry, HttpRequest, HttpResponse};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
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
    assert!(req
        .headers
        .iter()
        .any(|(k, v)| k == "Authorization" && v == "Bearer sk-test"));
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

// ── Task 6: OpenAI Responses API provider tests ─────────────────────

use dyyl::ai::provider_openai_response::OpenaiResponseProvider;

#[test]
fn openai_response_builds_correct_request_body() {
    let provider =
        OpenaiResponseProvider::new("sk-test".to_string(), "gpt-4o".to_string(), String::new());
    let req = provider.build_request("Be concise", "Hi");
    assert!(req.url.ends_with("/responses"));
    let body: serde_json::Value = serde_json::from_str(&req.body).expect("json");
    assert_eq!(body["model"], "gpt-4o");
    assert_eq!(body["instructions"], "Be concise");
    assert_eq!(body["input"], "Hi");
}

#[test]
fn openai_response_parses_output_text() {
    let provider =
        OpenaiResponseProvider::new("sk-test".to_string(), "gpt-4o".to_string(), String::new());
    let body = r#"{"output":[{"content":[{"type":"output_text","text":"Hello"}]}]}"#;
    assert_eq!(provider.parse_response(body), Ok("Hello".to_string()));
}

// ── Task 7: Anthropic Messages API provider tests ──────────────────

use dyyl::ai::provider_anthropic::AnthropicProvider;

#[test]
fn anthropic_builds_correct_request_body() {
    let provider = AnthropicProvider::new(
        "sk-ant".to_string(),
        "claude-3-5-sonnet-20241022".to_string(),
        String::new(),
    );
    let req = provider.build_request("You are helpful", "Hi");
    assert_eq!(req.url, "https://api.anthropic.com/v1/messages");
    assert!(req
        .headers
        .iter()
        .any(|(k, v)| k == "x-api-key" && v == "sk-ant"));
    assert!(req
        .headers
        .iter()
        .any(|(k, v)| k == "anthropic-version" && v == "2023-06-01"));
    let body: serde_json::Value = serde_json::from_str(&req.body).expect("json");
    assert_eq!(body["model"], "claude-3-5-sonnet-20241022");
    assert_eq!(body["max_tokens"], 4096);
    assert_eq!(body["system"], "You are helpful");
    assert_eq!(body["messages"][0]["role"], "user");
    assert_eq!(body["messages"][0]["content"], "Hi");
}

#[test]
fn anthropic_parses_content_text() {
    let provider = AnthropicProvider::new(
        "sk-ant".to_string(),
        "claude-3-5-sonnet-20241022".to_string(),
        String::new(),
    );
    let body = r#"{"content":[{"type":"text","text":"Hello"}]}"#;
    assert_eq!(provider.parse_response(body), Ok("Hello".to_string()));
}

#[test]
fn anthropic_omits_system_when_empty() {
    let provider =
        AnthropicProvider::new("sk-ant".to_string(), "claude".to_string(), String::new());
    let req = provider.build_request("", "Hi");
    let body: serde_json::Value = serde_json::from_str(&req.body).expect("json");
    assert!(body.get("system").is_none() || body["system"].as_str() == Some(""));
}

// ── Task 8: batch prompt construction + response parsing tests ─────

use dyyl::ai::prompt::{build_batch, parse_response, Placeholder};

#[test]
fn build_batch_marks_placeholders_with_ids() {
    let content = "set $port, ai.auto \"端口常用25565\"\nset $name, ai.auto\n";
    let placeholders = vec![
        Placeholder {
            id: 1,
            line: 1,
            hint: Some("端口常用25565".to_string()),
            original_text: "ai.auto \"端口常用25565\"".to_string(),
        },
        Placeholder {
            id: 2,
            line: 2,
            hint: None,
            original_text: "ai.auto".to_string(),
        },
    ];
    let (system, user) = build_batch(content, &placeholders);
    assert!(system.contains("filling placeholder values"));
    assert!(user.contains("<<<AUTO_1: 端口常用25565>>>"));
    assert!(user.contains("<<<AUTO_2: (no hint, infer from position)>>>"));
    assert!(user.contains("set $port, <<<AUTO_1"));
}

#[test]
fn parse_response_extracts_typed_values() {
    let body = r#"{"1":{"type":"string","value":"Steve"},"2":{"type":"number","value":25565}}"#;
    let values = parse_response(body).expect("parse");
    assert_eq!(values.len(), 2);
    assert_eq!(values.get("1").unwrap().value, "Steve");
    assert_eq!(values.get("1").unwrap().is_number, false);
    assert_eq!(values.get("2").unwrap().value, "25565");
    assert_eq!(values.get("2").unwrap().is_number, true);
}

#[test]
fn parse_response_strips_markdown_code_fence() {
    let body = "```json\n{\"1\":{\"type\":\"string\",\"value\":\"x\"}}\n```";
    let values = parse_response(body).expect("parse");
    assert_eq!(values.get("1").unwrap().value, "x");
}

#[test]
fn parse_response_extracts_json_from_surrounding_text() {
    let body = "Here are the values:\n{\"1\":{\"type\":\"number\",\"value\":42}}\nDone.";
    let values = parse_response(body).expect("parse");
    assert_eq!(values.get("1").unwrap().value, "42");
}

#[test]
fn parse_response_empty_json_object() {
    let body = "{}";
    let values = parse_response(body).expect("parse");
    assert!(values.is_empty());
}

// ── Task 9: ai.auto.filled command handler tests ───────────────────

use dyyl::runtime::execute::run_script;
use dyyl::runtime::Value;

#[test]
fn ai_auto_filled_returns_string_value() {
    let v = run_script("set $x, ai.auto.filled _, \"hello\"\nio.out $x", false).values;
    assert_eq!(v[1], Value::Str("hello".to_string()));
}

#[test]
fn ai_auto_filled_number_returns_num() {
    let v = run_script("set $x, ai.auto.filled _, 42\nio.out $x", false).values;
    assert_eq!(v[1], Value::Num(42));
}

#[test]
fn ai_auto_filled_with_hint_ignores_hint() {
    let v = run_script(
        "set $x, ai.auto.filled \"some hint\", \"value\"\nio.out $x",
        false,
    )
    .values;
    assert_eq!(v[1], Value::Str("value".to_string()));
}
