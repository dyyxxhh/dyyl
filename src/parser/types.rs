use std::fmt;

/// A parsed dyyl expression — either a literal value or a command call.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    /// A command call with arguments.
    Call(Call),
    /// Quoted string or bare-word parameter (includes `$var` references).
    Param(String),
    /// Integer literal.
    Num(i64),
    /// Fraction literal `a/b`.
    Fraction(i64, i64),
    /// Sqrt literal `√<radicand>`.
    Sqrt(String),
    /// Pi constant.
    Pi,
    /// Placeholder `_` / `empty`.
    Empty,
}

/// A command call expression.
#[derive(Debug, Clone, PartialEq)]
pub struct Call {
    /// Command name, e.g. `"set"`, `"math.add"`.
    pub command: String,
    /// Argument expressions.
    pub args: Vec<Expr>,
}

/// A fully parsed command line.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedCommand {
    /// 1-based source line number.
    pub line: usize,
    /// Raw command text (as it appears in source).
    pub text: String,
    /// The parsed top-level call.
    pub call: Call,
}

/// A parse error with source location.
#[derive(Debug, Clone, PartialEq)]
pub struct ParseError {
    /// 1-based source line number.
    pub line: usize,
    /// The raw command text that caused the error.
    pub text: String,
    /// Human-readable error message.
    pub message: String,
}

impl fmt::Display for Expr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Call(call) => {
                let args: Vec<String> = call.args.iter().map(|a| a.to_string()).collect();
                write!(f, "Call({}({}))", call.command, args.join(", "))
            }
            Self::Param(s) => write!(f, "Param({s:?})"),
            Self::Num(n) => write!(f, "Num({n})"),
            Self::Fraction(a, b) => write!(f, "Fraction({a}/{b})"),
            Self::Sqrt(r) => write!(f, "Sqrt(√{r})"),
            Self::Pi => write!(f, "Pi(π)"),
            Self::Empty => write!(f, "Empty"),
        }
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}: {}", self.line, self.message)
    }
}

impl ParseError {
    #[must_use]
    pub fn new(line: usize, text: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            line,
            text: text.into(),
            message: message.into(),
        }
    }
}
