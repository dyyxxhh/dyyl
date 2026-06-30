use std::fmt;

/// A single lexical token.
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    /// Command name, e.g. `"io.out"`, `"set"`, `"math.add"`.
    Command(String),
    /// A bare-word parameter (with escapes resolved).
    Param(String),
    /// A quoted string parameter (with escapes resolved).
    QuotedParam(String),
    /// Integer numeric literal.
    Num(i64),
    /// Fraction literal `a/b`.
    Fraction(i64, i64),
    /// Sqrt literal `√<radicand>`.
    Sqrt(String),
    /// Pi constant `π`.
    Pi,
    /// Placeholder `_` or `empty`.
    Empty,
}

impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Command(s) => write!(f, "Command({s})"),
            Self::Param(s) => write!(f, "Param({s:?})"),
            Self::QuotedParam(s) => write!(f, "QuotedParam({s:?})"),
            Self::Num(n) => write!(f, "Num({n})"),
            Self::Fraction(a, b) => write!(f, "Fraction({a}/{b})"),
            Self::Sqrt(r) => write!(f, "Sqrt(√{r})"),
            Self::Pi => write!(f, "Pi(π)"),
            Self::Empty => write!(f, "Empty"),
        }
    }
}

/// A lexical error with source location.
#[derive(Debug, Clone, PartialEq)]
pub struct LexError {
    /// 1-based source line number.
    pub line: usize,
    /// Human-readable error message.
    pub message: String,
}

impl LexError {
    #[must_use]
    pub fn new(line: usize, message: impl Into<String>) -> Self {
        Self {
            line,
            message: message.into(),
        }
    }
}

impl fmt::Display for LexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}: {}", self.line, self.message)
    }
}
