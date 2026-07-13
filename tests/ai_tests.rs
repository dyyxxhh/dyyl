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
