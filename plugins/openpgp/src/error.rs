//! Plugin error type and the 10 error codes from spec §6.5.
//!
//! Errors are returned to the host as JSON:
//! `{"code":"<code>","message":"<message>"}`.

use std::ffi::CString;
use std::os::raw::c_char;

/// A plugin error — carries a stable code string and a human message.
#[derive(Debug)]
pub struct PluginError {
    code: &'static str,
    message: String,
}

impl PluginError {
    /// Construct a new error with the given code and message.
    #[must_use]
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    /// The stable error code (e.g. `"arity_mismatch"`).
    #[must_use]
    pub fn code(&self) -> &'static str {
        self.code
    }

    /// The human-readable error message.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    // ── Convenience constructors for the 10 codes ───────────────────

    #[must_use]
    pub fn arity_mismatch(msg: impl Into<String>) -> Self {
        Self::new("arity_mismatch", msg)
    }

    #[must_use]
    pub fn type_error(msg: impl Into<String>) -> Self {
        Self::new("type_error", msg)
    }

    #[must_use]
    pub fn unknown_command(cmd: &str) -> Self {
        Self::new("unknown_command", format!("unknown command: {cmd}"))
    }

    #[must_use]
    pub fn runtime(msg: impl Into<String>) -> Self {
        Self::new("runtime", msg)
    }

    #[must_use]
    pub fn key_not_found(msg: impl Into<String>) -> Self {
        Self::new("key_not_found", msg)
    }

    #[must_use]
    pub fn passphrase_wrong(msg: impl Into<String>) -> Self {
        Self::new("passphrase_wrong", msg)
    }

    #[must_use]
    pub fn parse_failed(msg: impl Into<String>) -> Self {
        Self::new("parse_failed", msg)
    }

    #[must_use]
    pub fn verify_failed(msg: impl Into<String>) -> Self {
        Self::new("verify_failed", msg)
    }

    #[must_use]
    pub fn gpg_not_installed(msg: impl Into<String>) -> Self {
        Self::new("gpg_not_installed", msg)
    }

    #[must_use]
    pub fn gpg_exec_failed(msg: impl Into<String>) -> Self {
        Self::new("gpg_exec_failed", msg)
    }
}

impl std::fmt::Display for PluginError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for PluginError {}

/// Write a `{"code":"<code>","message":"<message>"}` JSON object to the
/// `out` parameter (allocates via `CString::into_raw`). The caller must
/// free the buffer with `dyyl_plugin_free_string`.
///
/// Contract: `out` must be a valid, non-null pointer to a `*mut c_char`
/// slot owned by the caller.
pub fn write_error(out: *mut *mut c_char, code: &str, message: &str) {
    let json = format!(
        r#"{{"code":{},"message":{}}}"#,
        serde_json::to_string(code).unwrap_or_else(|_| "\"\"".to_string()),
        serde_json::to_string(message).unwrap_or_else(|_| "\"\"".to_string()),
    );
    let c = cstring_from_str(&json);
    // SAFETY: caller guarantees `out` is a valid pointer to a slot.
    unsafe {
        *out = c.into_raw();
    }
}

/// Build a `CString` from `&str`, stripping any NUL bytes (which would
/// otherwise make `CString::new` fail). After stripping, construction is
/// infallible; the `unwrap_or_else` branch is unreachable but kept to
/// satisfy the type system without `unwrap`/`expect` (denied by clippy).
fn cstring_from_str(s: &str) -> CString {
    let bytes: Vec<u8> = s.bytes().filter(|b| *b != 0).collect();
    CString::new(bytes).unwrap_or_else(|_| empty_cstring())
}

/// Returns a guaranteed-valid empty `CString`.
fn empty_cstring() -> CString {
    // SAFETY: A single NUL byte is a valid CString representing the empty
    // string (no interior NULs, last byte is NUL).
    unsafe { CString::from_vec_with_nul_unchecked(vec![0u8]) }
}
