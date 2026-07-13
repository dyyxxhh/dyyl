//! End-to-end integration tests for ai.auto fill via prepass.
//!
//! These tests spin up a mock HTTP server that emulates the OpenAI Chat
//! Completions endpoint, point `DYYL_CREDENTIALS_PATH` at a temporary
//! credentials.toml, then invoke `dyyl::prepass::run()` / `build_only()`
//! and verify the script is rewritten with `ai.auto.filled` values.

mod fixtures;

use fixtures::mock_ai_server::MockAiServer;
use std::fs;
use std::sync::{Mutex, MutexGuard};
use tempfile::tempdir;

/// Serialises the two tests so they don't race on the process-global
/// `DYYL_CREDENTIALS_PATH` environment variable.
static ENV_LOCK: Mutex<()> = Mutex::new(());

fn lock_env() -> MutexGuard<'static, ()> {
    ENV_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

/// Wrap raw batch-JSON (the AI "content") in an OpenAI Chat Completions
/// response envelope so that `OpenaiChatProvider::parse_response` can
/// extract it via `choices[0].message.content`.
fn chat_response(batch_json: &str) -> String {
    serde_json::json!({
        "choices": [{"message": {"content": batch_json}}]
    })
    .to_string()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ai_auto_filled_via_prepass_with_mock_server() {
    let batch_json = r#"{"1":{"type":"string","value":"Steve"},"2":{"type":"number","value":25565}}"#;
    let response = chat_response(batch_json);
    let server = MockAiServer::start(response).await;
    let dir = tempdir().unwrap();
    let creds_path = dir.path().join("credentials.toml");
    fs::write(
        &creds_path,
        format!(
            "[ai]\nprovider = \"openai-chat\"\napi_key = \"sk-test\"\nmodel = \"gpt-4o-mini\"\nbase_url = \"http://127.0.0.1:{}\"\n",
            server.port
        ),
    )
    .unwrap();
    let script_path = dir.path().join("script.dyyl");
    fs::write(
        &script_path,
        "set $name, ai.auto \"用户名\"\nset $port, ai.auto \"端口\"\n",
    )
    .unwrap();

    // --- critical section: env var is process-global -----------------
    let result = {
        let _guard = lock_env();
        std::env::set_var("DYYL_CREDENTIALS_PATH", &creds_path);
        let r = dyyl::prepass::run(&script_path, dyyl::i18n::Lang::En);
        std::env::remove_var("DYYL_CREDENTIALS_PATH");
        r
    };
    // -----------------------------------------------------------------

    assert!(result.is_ok(), "prepass should succeed: {:?}", result);
    let filled = fs::read_to_string(&script_path).unwrap();
    assert!(
        filled.contains("ai.auto.filled \"用户名\", \"Steve\""),
        "got: {filled}"
    );
    assert!(
        filled.contains("ai.auto.filled \"端口\", 25565"),
        "got: {filled}"
    );
    server.stop();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn dyyl_build_resets_and_refills() {
    let batch_json = r#"{"1":{"type":"number","value":100}}"#;
    let response = chat_response(batch_json);
    let server = MockAiServer::start(response).await;
    let dir = tempdir().unwrap();
    let creds_path = dir.path().join("credentials.toml");
    fs::write(
        &creds_path,
        format!(
            "[ai]\nprovider = \"openai-chat\"\napi_key = \"sk-test\"\nmodel = \"gpt-4o-mini\"\nbase_url = \"http://127.0.0.1:{}\"\n",
            server.port
        ),
    )
    .unwrap();
    let script_path = dir.path().join("script.dyyl");
    fs::write(
        &script_path,
        "set $x, ai.auto.filled \"hint\", 42\n",
    )
    .unwrap();

    // --- critical section: env var is process-global -----------------
    let result = {
        let _guard = lock_env();
        std::env::set_var("DYYL_CREDENTIALS_PATH", &creds_path);
        let r = dyyl::prepass::build_only(&script_path, dyyl::i18n::Lang::En);
        std::env::remove_var("DYYL_CREDENTIALS_PATH");
        r
    };
    // -----------------------------------------------------------------

    assert!(result.is_ok(), "build_only should succeed: {:?}", result);
    let filled = fs::read_to_string(&script_path).unwrap();
    assert!(
        filled.contains("ai.auto.filled \"hint\", 100"),
        "should be refilled with 100, got: {filled}"
    );
    server.stop();
}
