//! Streaming MCM host protocol — NDJSON over stdio.
//!
//! When dyyl encounters an `mcm.*` command, it emits an `McmCommand` event
//! as a single-line JSON object to the host and blocks until the host replies
//! with an `McmResponse`. The host correlation key is a monotonically
//! increasing `id` string.
//!
//! Protocol shapes (NDJSON, one object per line):
//!
//! ```jsonc
//! // dyyl → host
//! { "type":"mcm_command", "id":"1", "name":"mcm.game.choose", "args":["1.21.1"], "source_line":"mcm.game.choose 1.21.1" }
//! // host → dyyl success
//! { "type":"mcm_response", "id":"1", "ok":true, "value":"1.21.1" }
//! // host → dyyl failure
//! { "type":"mcm_response", "id":"1", "ok":false, "error":{"code":"unknown_command","message":"mcm.unknown not supported"} }
//! ```

use std::collections::VecDeque;
use std::io::{self, BufRead, BufReader, Write};
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};

use crate::runtime::error::RuntimeError;
use crate::runtime::value::Value;

// ── Protocol message types ──────────────────────────────────────────

/// A command event sent from dyyl to the MCM host.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct McmCommand {
    /// Always `"mcm_command"`.
    #[serde(rename = "type")]
    pub kind: String,
    /// Monotonically increasing correlation id (stringified u64).
    pub id: String,
    /// Fully-qualified command name, e.g. `"mcm.game.choose"`.
    pub name: String,
    /// Positional arguments as dyyl `Value` equivalents.
    pub args: Vec<McmArg>,
    /// Raw source line for diagnostics.
    pub source_line: String,
}

/// A response event sent from the MCM host to dyyl.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct McmResponse {
    /// Always `"mcm_response"`.
    #[serde(rename = "type")]
    pub kind: String,
    /// Must match the command `id`.
    pub id: String,
    /// Whether the command succeeded.
    pub ok: bool,
    /// Return value on success (`null` on error).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<McmArg>,
    /// Error payload on failure (`null` on success).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<McmError>,
}

/// Error payload inside an `McmResponse`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct McmError {
    /// Machine-readable code, e.g. `"unknown_command"`, `"host_timeout"`.
    pub code: String,
    /// Human-readable message.
    pub message: String,
}

/// A JSON-serialisable argument value.
///
/// This mirrors dyyl `Value` at the protocol level but stays JSON-native
/// so the host can parse it without knowing dyyl internals.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum McmArg {
    Num(i64),
    Str(String),
    Bool(bool),
    Null,
}

impl McmArg {
    /// Convert a dyyl runtime `Value` to an `McmArg`.
    #[must_use]
    pub fn from_value(v: &Value) -> Self {
        match v {
            Value::Num(n) => Self::Num(*n),
            Value::Str(s) => Self::Str(s.clone()),
            Value::Empty => Self::Null,
            // For non-scalar values, use display form.
            Value::Expr(e) => Self::Str(e.to_string()),
            Value::List(items) => {
                let strs: Vec<String> = items.iter().map(ToString::to_string).collect();
                Self::Str(strs.join(","))
            }
            Value::Dict(pairs) => {
                let strs: Vec<String> = pairs.iter().map(|(k, v)| format!("{k}:{v}")).collect();
                Self::Str(strs.join(","))
            }
        }
    }

    /// Convert an `McmArg` to a dyyl runtime `Value`.
    #[must_use]
    pub fn to_value(&self) -> Value {
        match self {
            Self::Num(n) => Value::Num(*n),
            Self::Str(s) => Value::Str(s.clone()),
            Self::Bool(b) => Value::Num(i64::from(*b)),
            Self::Null => Value::Empty,
        }
    }
}

// ── Host connection trait ───────────────────────────────────────────

/// Abstraction for the bidirectional NDJSON host connection.
///
/// Implementations send `McmCommand` events and receive `McmResponse`
/// events. Tests inject a mock; production uses stdin/stdout.
pub trait HostProvider: Send + Sync + std::fmt::Debug {
    /// Send a command to the host and block for the response.
    fn send_command(&self, cmd: &McmCommand) -> Result<McmResponse, HostError>;
}

/// Errors from host communication.
#[derive(Debug)]
pub enum HostError {
    /// Serialisation failure.
    Serialize(String),
    /// IO failure (broken pipe, etc.).
    Io(io::Error),
    /// The host response could not be parsed.
    Deserialize(String),
}

impl std::fmt::Display for HostError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Serialize(s) => write!(f, "host serialize error: {s}"),
            Self::Io(e) => write!(f, "host I/O error: {e}"),
            Self::Deserialize(s) => write!(f, "host deserialize error: {s}"),
        }
    }
}

impl std::error::Error for HostError {}

/// Convert a `HostError` into a `RuntimeError` for `mcm.*` dispatch.
pub fn host_error_to_runtime(err: &HostError, line: usize, command: &str) -> RuntimeError {
    RuntimeError::new(line, command, err.to_string())
}

/// Convert an `McmResponse` error into a `RuntimeError`.
pub fn mcm_response_error_to_runtime(resp: &McmResponse, line: usize, command: &str) -> RuntimeError {
    let reason = resp.error.as_ref().map_or_else(
        || "unknown host error".to_string(),
        |e| format!("[{}] {}", e.code, e.message),
    );
    RuntimeError::new(line, command, reason)
}

// ── Stdio host connection ───────────────────────────────────────────

/// Production host connection over stdin (reads responses) and stdout
/// (writes commands), with a shared counter for correlation ids.
pub struct StdioHostConnection {
    id_counter: AtomicU64,
}

impl std::fmt::Debug for StdioHostConnection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StdioHostConnection")
            .field("next_id", &self.id_counter.load(Ordering::Relaxed))
            .finish()
    }
}

impl StdioHostConnection {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            id_counter: AtomicU64::new(1),
        }
    }

    fn next_id(&self) -> String {
        self.id_counter.fetch_add(1, Ordering::Relaxed).to_string()
    }
}

impl Default for StdioHostConnection {
    fn default() -> Self {
        Self::new()
    }
}

impl HostProvider for StdioHostConnection {
    fn send_command(&self, cmd: &McmCommand) -> Result<McmResponse, HostError> {
        let mut stdout = io::stdout();
        let line = serde_json::to_string(cmd).map_err(|e| HostError::Serialize(e.to_string()))?;
        writeln!(stdout, "{line}").map_err(HostError::Io)?;
        stdout.flush().map_err(HostError::Io)?;

        let stdin = io::stdin();
        let mut reader = BufReader::new(stdin.lock());
        let mut buf = String::new();
        reader.read_line(&mut buf).map_err(HostError::Io)?;
        drop(reader);
        buf.truncate(buf.trim_end().len());
        if buf.is_empty() {
            return Err(HostError::Deserialize(
                "host closed connection (empty line)".to_string(),
            ));
        }
        let resp: McmResponse =
            serde_json::from_str(&buf).map_err(|e| HostError::Deserialize(e.to_string()))?;
        Ok(resp)
    }
}

impl StdioHostConnection {
    /// Build the next command with auto-incremented id.
    #[must_use]
    pub fn build_command(&self, name: &str, args: Vec<McmArg>, source_line: &str) -> McmCommand {
        McmCommand {
            kind: "mcm_command".to_string(),
            id: self.next_id(),
            name: name.to_string(),
            args,
            source_line: source_line.to_string(),
        }
    }
}

// ── Mock host provider for testing ──────────────────────────────────

/// A mock host provider that returns pre-configured responses.
///
/// The responses are consumed in order. When a response is missing,
/// the mock returns `unknown_command`.
pub struct MockHostProvider {
    responses: std::sync::Mutex<VecDeque<McmResponse>>,
    commands_sent: std::sync::Mutex<Vec<McmCommand>>,
}

impl std::fmt::Debug for MockHostProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MockHostProvider")
            .field("commands_sent", &self.commands_sent())
            .finish()
    }
}

impl MockHostProvider {
    /// Create a mock with pre-loaded responses.
    #[must_use]
    pub fn with_responses(responses: Vec<McmResponse>) -> Self {
        Self {
            responses: std::sync::Mutex::new(responses.into_iter().collect()),
            commands_sent: std::sync::Mutex::new(Vec::new()),
        }
    }

    /// Create an empty mock (all commands get `unknown_command` error).
    #[must_use]
    pub fn new() -> Self {
        Self::with_responses(Vec::new())
    }

    /// Return all commands that were sent through this mock.
    #[must_use]
    pub fn commands_sent(&self) -> Vec<McmCommand> {
        self.commands_sent
            .lock()
            .map(|v| v.clone())
            .unwrap_or_default()
    }

    /// Push an additional response.
    pub fn push_response(&self, resp: McmResponse) {
        if let Ok(mut q) = self.responses.lock() {
            q.push_back(resp);
        }
    }

    /// Helper: create an OK response.
    #[must_use]
    pub fn ok_response(id: &str, value: McmArg) -> McmResponse {
        McmResponse {
            kind: "mcm_response".to_string(),
            id: id.to_string(),
            ok: true,
            value: Some(value),
            error: None,
        }
    }

    /// Helper: create an error response.
    #[must_use]
    pub fn error_response(id: &str, code: &str, message: &str) -> McmResponse {
        McmResponse {
            kind: "mcm_response".to_string(),
            id: id.to_string(),
            ok: false,
            value: None,
            error: Some(McmError {
                code: code.to_string(),
                message: message.to_string(),
            }),
        }
    }
}

impl Default for MockHostProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl HostProvider for MockHostProvider {
    fn send_command(&self, cmd: &McmCommand) -> Result<McmResponse, HostError> {
        // Record the command.
        if let Ok(mut sent) = self.commands_sent.lock() {
            sent.push(cmd.clone());
        }
        // Pop the next pre-loaded response.
        let resp = self
            .responses
            .lock()
            .ok()
            .and_then(|mut q| q.pop_front())
            .unwrap_or_else(|| McmResponse {
                kind: "mcm_response".to_string(),
                id: cmd.id.clone(),
                ok: false,
                value: None,
                error: Some(McmError {
                    code: "unknown_command".to_string(),
                    message: format!("{} not supported by host", cmd.name),
                }),
            });
        Ok(resp)
    }
}

// ── Game-choose scope state ─────────────────────────────────────────

/// Tracks `mcm.game.choose` session state within a single script run.
///
/// The selected version is valid from the `mcm.game.choose` call until
/// the next `mcm.game.choose` or script end.
#[derive(Debug, Clone, Default)]
pub struct GameChooseScope {
    /// The currently selected game version, if any.
    pub selected_version: Option<String>,
}

impl GameChooseScope {
    /// Select a new game version.
    pub fn select(&mut self, version: String) {
        self.selected_version = Some(version);
    }

    /// Reset scope (called on script end or next choose).
    pub fn reset(&mut self) {
        self.selected_version = None;
    }

    /// Get the currently selected version.
    #[must_use]
    pub fn current(&self) -> Option<&str> {
        self.selected_version.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mcm_arg_roundtrip() {
        let args = vec![
            McmArg::Num(42),
            McmArg::Str("hello".to_string()),
            McmArg::Bool(true),
            McmArg::Null,
        ];
        for arg in &args {
            let json = serde_json::to_string(arg).expect("serialize");
            let back: McmArg = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(arg, &back);
        }
    }

    #[test]
    fn mcm_command_serialization() {
        let cmd = McmCommand {
            kind: "mcm_command".to_string(),
            id: "1".to_string(),
            name: "mcm.game.choose".to_string(),
            args: vec![McmArg::Str("1.21.1".to_string())],
            source_line: "mcm.game.choose 1.21.1".to_string(),
        };
        let json = serde_json::to_string(&cmd).expect("serialize");
        assert!(json.contains("\"type\":\"mcm_command\""));
        assert!(json.contains("\"name\":\"mcm.game.choose\""));
        assert!(json.contains("\"args\":[\"1.21.1\"]"));

        let back: McmCommand = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, cmd);
    }

    #[test]
    fn mcm_response_ok_serialization() {
        let resp = McmResponse {
            kind: "mcm_response".to_string(),
            id: "1".to_string(),
            ok: true,
            value: Some(McmArg::Str("ok".to_string())),
            error: None,
        };
        let json = serde_json::to_string(&resp).expect("serialize");
        assert!(json.contains("\"ok\":true"));
        // error should be omitted when None.
        assert!(!json.contains("error"));

        let back: McmResponse = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, resp);
    }

    #[test]
    fn mcm_response_error_serialization() {
        let resp = McmResponse {
            kind: "mcm_response".to_string(),
            id: "1".to_string(),
            ok: false,
            value: None,
            error: Some(McmError {
                code: "unknown_command".to_string(),
                message: "mcm.foo not supported".to_string(),
            }),
        };
        let json = serde_json::to_string(&resp).expect("serialize");
        assert!(json.contains("\"ok\":false"));
        assert!(json.contains("\"code\":\"unknown_command\""));
        // value should be omitted when None.
        assert!(!json.contains("\"value\""));

        let back: McmResponse = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, resp);
    }

    #[test]
    fn mock_host_provider_returns_preloaded_responses() {
        let mock = MockHostProvider::with_responses(vec![
            MockHostProvider::ok_response("1", McmArg::Str("1.21.1".to_string())),
            MockHostProvider::ok_response("2", McmArg::Num(0)),
        ]);

        let cmd1 = McmCommand {
            kind: "mcm_command".to_string(),
            id: "1".to_string(),
            name: "mcm.game.choose".to_string(),
            args: vec![McmArg::Str("1.21.1".to_string())],
            source_line: "mcm.game.choose 1.21.1".to_string(),
        };
        let cmd2 = McmCommand {
            kind: "mcm_command".to_string(),
            id: "2".to_string(),
            name: "mcm.game.install".to_string(),
            args: vec![McmArg::Str("1.21.1".to_string())],
            source_line: "mcm.game.install 1.21.1".to_string(),
        };

        let resp1 = mock.send_command(&cmd1).expect("send 1");
        assert!(resp1.ok);
        let resp2 = mock.send_command(&cmd2).expect("send 2");
        assert!(resp2.ok);

        let sent = mock.commands_sent();
        assert_eq!(sent.len(), 2);
        assert_eq!(sent[0].name, "mcm.game.choose");
        assert_eq!(sent[1].name, "mcm.game.install");
    }

    #[test]
    fn mock_host_provider_unknown_when_exhausted() {
        let mock = MockHostProvider::new();
        let cmd = McmCommand {
            kind: "mcm_command".to_string(),
            id: "1".to_string(),
            name: "mcm.game.choose".to_string(),
            args: vec![],
            source_line: "mcm.game.choose".to_string(),
        };
        let resp = mock.send_command(&cmd).expect("send");
        assert!(!resp.ok);
        assert_eq!(
            resp.error.as_ref().map(|e| e.code.as_str()),
            Some("unknown_command")
        );
    }

    #[test]
    fn game_choose_scope_select_and_reset() {
        let mut scope = GameChooseScope::default();
        assert!(scope.current().is_none());

        scope.select("1.21.1".to_string());
        assert_eq!(scope.current(), Some("1.21.1"));

        scope.reset();
        assert!(scope.current().is_none());

        scope.select("1.20.4".to_string());
        assert_eq!(scope.current(), Some("1.20.4"));

        // New choose replaces old.
        scope.select("1.21".to_string());
        assert_eq!(scope.current(), Some("1.21"));
    }

    #[test]
    fn value_to_mcm_arg_roundtrip() {
        let cases = vec![
            (Value::Num(42), McmArg::Num(42)),
            (
                Value::Str("hello".to_string()),
                McmArg::Str("hello".to_string()),
            ),
            (Value::Empty, McmArg::Null),
        ];
        for (val, expected) in cases {
            let arg = McmArg::from_value(&val);
            assert_eq!(arg, expected);
            let back = arg.to_value();
            assert_eq!(back, val);
        }
    }
}
