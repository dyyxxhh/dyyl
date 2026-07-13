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
