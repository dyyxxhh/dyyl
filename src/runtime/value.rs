//! dyyl runtime value model.
//!
//! `Value` is the central enum for all dyyl script values:
//! numeric (both simple `Num(i64)` and CAS `Expr(CasNumber)`),
//! string, list, dict, and placeholder/empty.
//! Sentinel helpers provide type-appropriate error-indicator values.

use crate::math::CasNumber;

use std::fmt;

/// A dyyl runtime value.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// Numeric value (i64) — for simple integers and sentinels.
    Num(i64),
    /// CAS expression value — exact rationals, symbolic constants, roots.
    Expr(CasNumber),
    /// String value.
    Str(String),
    /// Mutable list of values.
    List(Vec<Value>),
    /// Mutable dictionary (ordered key–value pairs).
    ///
    /// Keys may be any `Value` type (Decision 49).  An ordered vec of pairs
    /// keeps implementation simple and does not require `Hash` on `Value`.
    Dict(Vec<(Value, Value)>),
    /// Placeholder / empty (`_` / `empty`).
    Empty,
}

impl Value {
    // ── Sentinel helpers ──────────────────────────────────────────────────

    /// Sentinel for numeric operations: `-1`.
    #[must_use]
    pub const fn sentinel_num() -> Self {
        Self::Num(-1)
    }

    /// Sentinel for string operations: `""`.
    #[must_use]
    pub fn sentinel_str() -> Self {
        Self::Str(String::new())
    }

    /// Sentinel for logic operations: `0` (false).
    #[must_use]
    pub const fn sentinel_logic() -> Self {
        Self::Num(0)
    }

    /// Sentinel for dict operations: empty dict.
    #[must_use]
    pub fn sentinel_dict() -> Self {
        Self::Dict(Vec::new())
    }

    /// Sentinel for list operations: empty list.
    #[must_use]
    pub fn sentinel_list() -> Self {
        Self::List(Vec::new())
    }

    /// Default / generic sentinel: `-1`.
    #[must_use]
    pub const fn sentinel_default() -> Self {
        Self::Num(-1)
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Num(n) => write!(f, "{n}"),
            Self::Expr(e) => write!(f, "{e}"),
            Self::Str(s) => write!(f, "{s}"),
            Self::List(items) => {
                let parts: Vec<String> = items.iter().map(Value::to_string).collect();
                write!(f, "[{}]", parts.join(", "))
            }
            Self::Dict(pairs) => {
                let parts: Vec<String> = pairs.iter().map(|(k, v)| format!("{k}: {v}")).collect();
                write!(f, "{{{}}}", parts.join(", "))
            }
            Self::Empty => write!(f, "empty"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn value_display_num() {
        assert_eq!(Value::Num(42).to_string(), "42");
        assert_eq!(Value::Num(-1).to_string(), "-1");
        assert_eq!(Value::Num(0).to_string(), "0");
    }

    #[test]
    fn value_display_str() {
        assert_eq!(Value::Str("hello".to_string()).to_string(), "hello");
        assert_eq!(Value::Str(String::new()).to_string(), "");
    }

    #[test]
    fn value_display_list() {
        let list = Value::List(vec![Value::Num(1), Value::Num(2)]);
        assert_eq!(list.to_string(), "[1, 2]");
        assert_eq!(Value::sentinel_list().to_string(), "[]");
    }

    #[test]
    fn value_display_dict() {
        let pairs = vec![(Value::Str("a".into()), Value::Num(1))];
        let dict = Value::Dict(pairs);
        assert_eq!(dict.to_string(), "{a: 1}");
        assert_eq!(Value::sentinel_dict().to_string(), "{}");
    }

    #[test]
    fn value_display_empty() {
        assert_eq!(Value::Empty.to_string(), "empty");
    }

    #[test]
    fn sentinel_helpers_produce_expected_values() {
        assert_eq!(Value::sentinel_num(), Value::Num(-1));
        assert_eq!(Value::sentinel_str(), Value::Str(String::new()));
        assert_eq!(Value::sentinel_logic(), Value::Num(0));
        assert_eq!(Value::sentinel_dict(), Value::Dict(Vec::new()));
        assert_eq!(Value::sentinel_list(), Value::List(Vec::new()));
        assert_eq!(Value::sentinel_default(), Value::Num(-1));
    }
}
