//! Parse the credentials JSON (built by dyyl's `creds_inject.rs`) and
//! populate `PluginState`.
//!
//! The injected JSON is a flat object:
//! ```json
//! {
//!   "passphrase": "...",
//!   "default_key": "FINGERPRINT",
//!   "__credentials_dir": "/abs/path/to/creds/openpgp"
//! }
//! ```
//! Missing fields are OK — the corresponding state field keeps its default.

use serde_json::Value;

use crate::state::PluginState;

/// Apply the credentials JSON to `state`.
///
/// - `"passphrase"` → `state.default_passphrase` (only if non-empty)
/// - `"default_key"` → `state.default_key` (only if non-empty)
/// - `"__credentials_dir"` → `state.credentials_dir`
pub fn apply_credentials(state: &mut PluginState, json: &str) -> Result<(), String> {
    let parsed: Value =
        serde_json::from_str(json).map_err(|e| format!("parse credentials json: {e}"))?;
    let obj = parsed
        .as_object()
        .ok_or_else(|| "credentials json is not an object".to_string())?;

    if let Some(pass) = obj.get("passphrase").and_then(|v| v.as_str()) {
        if !pass.is_empty() {
            state.default_passphrase = Some(pass.to_string());
        }
    }

    if let Some(key) = obj.get("default_key").and_then(|v| v.as_str()) {
        if !key.is_empty() {
            state.default_key = Some(key.to_string());
        }
    }

    if let Some(dir) = obj.get("__credentials_dir").and_then(|v| v.as_str()) {
        state.credentials_dir = std::path::PathBuf::from(dir);
    }

    Ok(())
}
