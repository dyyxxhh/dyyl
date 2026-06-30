//! dyyl runtime error model.
//!
//! `RuntimeError` carries the source location and a human-readable reason.
//! The `error_to_sentinel` function maps errors to type-appropriate sentinel
//! `Value`s based on the command family prefix (Decision 3).
//! `debug_diagnostic` writes structured warnings to stderr.

use std::fmt;

use crate::i18n::{self, Lang};
use crate::runtime::value::Value;

/// A dyyl runtime error with source location and reason.
#[derive(Debug, Clone)]
pub struct RuntimeError {
    /// 1-based source line number.
    pub line: usize,
    /// Command name that caused the error.
    pub command: String,
    /// Human-readable reason for the error.
    pub reason: String,
}

impl RuntimeError {
    /// Create a new runtime error.
    #[must_use]
    pub fn new(line: usize, command: impl Into<String>, reason: impl Into<String>) -> Self {
        Self {
            line,
            command: command.into(),
            reason: reason.into(),
        }
    }
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}: {} — {}", self.line, self.command, self.reason)
    }
}

/// Convert a `RuntimeError` to the appropriate sentinel `Value` based on the
/// command's family prefix (Decision 3).
///
/// | Command prefix   | Sentinel     |
/// |------------------|--------------|
/// | `math.`/`create.`| `Num(-1)`    |
/// | `str.`           | `Str("")`    |
/// | `logic.`         | `Num(0)`     |
/// | `dict.`          | `Dict({})`   |
/// | `list.`          | `List([])`   |
/// | `io.`/`net.`/`file.` | `Str("")` |
/// | (unknown/other)  | `Num(-1)`    |
#[must_use]
pub fn error_to_sentinel(err: &RuntimeError) -> Value {
    let command = &err.command;
    if command.starts_with("math.") || command.starts_with("create.") {
        Value::sentinel_num()
    } else if command.starts_with("str.") {
        Value::sentinel_str()
    } else if command.starts_with("logic.") {
        Value::sentinel_logic()
    } else if command.starts_with("dict.") {
        Value::sentinel_dict()
    } else if command.starts_with("list.") {
        Value::sentinel_list()
    } else if command.starts_with("io.")
        || command.starts_with("net.")
        || command.starts_with("file.")
    {
        Value::sentinel_str()
    } else if command.starts_with("user.")
        || command.starts_with("system.")
        || command.starts_with("time.")
    {
        Value::sentinel_default()
    } else {
        // Unknown / unclassified command → numeric sentinel.
        Value::sentinel_default()
    }
}

/// Write a debug diagnostic to stderr with line number, command text, and
/// error reason.
pub fn debug_diagnostic(err: &RuntimeError, text: &str, lang: Lang) {
    eprintln!("line {}: {}", err.line, text);
    eprintln!("{}{}", i18n::reason_prefix(lang), err.reason);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sentinel_num_for_math() {
        let err = RuntimeError::new(1, "math.add", "test");
        assert_eq!(error_to_sentinel(&err), Value::Num(-1));
    }

    #[test]
    fn sentinel_num_for_create() {
        let err = RuntimeError::new(2, "create.num", "test");
        assert_eq!(error_to_sentinel(&err), Value::Num(-1));
    }

    #[test]
    fn sentinel_str_for_str_cmd() {
        let err = RuntimeError::new(1, "str.len", "test");
        assert_eq!(error_to_sentinel(&err), Value::Str(String::new()));
    }

    #[test]
    fn sentinel_logic() {
        let err = RuntimeError::new(1, "logic.same", "test");
        assert_eq!(error_to_sentinel(&err), Value::Num(0));
    }

    #[test]
    fn sentinel_dict() {
        let err = RuntimeError::new(1, "dict.get", "test");
        assert_eq!(error_to_sentinel(&err), Value::Dict(Vec::new()));
    }

    #[test]
    fn sentinel_list() {
        let err = RuntimeError::new(1, "list.get", "test");
        assert_eq!(error_to_sentinel(&err), Value::List(Vec::new()));
    }

    #[test]
    fn sentinel_unknown_command() {
        let err = RuntimeError::new(1, "unknown.cmd", "test");
        assert_eq!(error_to_sentinel(&err), Value::Num(-1));
    }

    #[test]
    fn sentinel_empty_command() {
        let err = RuntimeError::new(1, "", "undefined variable '$x'");
        assert_eq!(error_to_sentinel(&err), Value::Num(-1));
    }

    #[test]
    fn runtime_error_display() {
        let err = RuntimeError::new(5, "io.out", "something went wrong");
        let msg = err.to_string();
        assert!(msg.contains("line 5"));
        assert!(msg.contains("io.out"));
        assert!(msg.contains("something went wrong"));
    }
}
