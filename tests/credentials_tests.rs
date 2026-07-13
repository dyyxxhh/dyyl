use dyyl::credentials::{AiCredentials, AiProviderKind, CredentialsFile};
use tempfile::tempdir;

#[test]
fn load_missing_file_returns_default() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("credentials.toml");
    let creds = CredentialsFile::load(&path).expect("load missing");
    assert!(creds.ai.is_none());
    assert!(creds.plugins.is_empty());
}

#[test]
fn roundtrip_ai_credentials() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("credentials.toml");
    let mut creds = CredentialsFile::default();
    creds.ai = Some(AiCredentials {
        provider: AiProviderKind::OpenaiChat,
        api_key: "sk-test".to_string(),
        model: "gpt-4o-mini".to_string(),
        base_url: String::new(),
        auto_system_prompt: String::new(),
    });
    creds.save(&path).expect("save");
    let loaded = CredentialsFile::load(&path).expect("load");
    assert_eq!(loaded.ai.as_ref().unwrap().api_key, "sk-test");
    assert_eq!(
        loaded.ai.as_ref().unwrap().provider,
        AiProviderKind::OpenaiChat
    );
}

#[test]
fn load_partial_ai_segment_returns_none() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("credentials.toml");
    std::fs::write(&path, "[ai]\nprovider = \"openai-chat\"\n").expect("write");
    let creds = CredentialsFile::load(&path).expect("load");
    assert!(creds.ai.is_none(), "missing api_key should yield None");
}

#[test]
fn plugin_credentials_roundtrip() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("credentials.toml");
    let mut creds = CredentialsFile::default();
    let mut plugin = std::collections::HashMap::new();
    plugin.insert("token".to_string(), "ghp_x".to_string());
    plugin.insert("repo".to_string(), "foo/bar".to_string());
    creds.plugins.insert("migpt".to_string(), plugin);
    creds.save(&path).expect("save");
    let loaded = CredentialsFile::load(&path).expect("load");
    assert_eq!(
        loaded.plugins.get("migpt").unwrap().get("token"),
        Some(&"ghp_x".to_string())
    );
}

#[test]
fn prompt_ai_reads_all_fields_from_lines() {
    use dyyl::credentials::prompt_ai_from_lines;
    let lines = vec![
        "1".to_string(),
        "sk-abc".to_string(),
        "gpt-4o".to_string(),
        "".to_string(),
    ];
    let (creds, consumed) = prompt_ai_from_lines(&lines).expect("prompt");
    assert_eq!(consumed, 4);
    assert_eq!(
        creds.provider,
        dyyl::credentials::AiProviderKind::OpenaiChat
    );
    assert_eq!(creds.api_key, "sk-abc");
    assert_eq!(creds.model, "gpt-4o");
    assert!(creds.base_url.is_empty());
}

#[test]
fn prompt_ai_invalid_choice_returns_error() {
    use dyyl::credentials::prompt_ai_from_lines;
    let lines = vec!["9".to_string()];
    let result = prompt_ai_from_lines(&lines);
    assert!(result.is_err());
}

#[test]
fn prompt_ai_empty_api_key_returns_error() {
    use dyyl::credentials::prompt_ai_from_lines;
    let lines = vec![
        "1".to_string(),
        "".to_string(),
        "model".to_string(),
        "".to_string(),
    ];
    assert!(prompt_ai_from_lines(&lines).is_err());
}
