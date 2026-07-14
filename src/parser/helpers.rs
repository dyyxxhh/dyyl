//! Parser helper functions: heuristics, token reconstruction, literal
//! classification, and fraction parsing.

use crate::lexer::Token;
use crate::parser::arity::known_arity;
use crate::parser::types::Expr;

// ── Heuristics ───────────────────────────────────────────────────────────

/// Extract the first whitespace-delimited word from `s`.
pub(super) fn first_word(s: &str) -> &str {
    match s.split_whitespace().next() {
        Some(word) => word,
        None => s,
    }
}

/// Check if `name` looks like a plugin command name: `<identifier>.<rest>`,
/// not in the known arity table. The first segment (before the first dot)
/// must be a valid identifier (starts with ASCII letter/underscore,
/// alphanumeric/underscore only) so decimal numbers like `1.5` and other
/// non-command literals are not misclassified. These are routed by the
/// dispatcher's plugin fallback arm (`<plugin>.<sub>[.<sub>...]`).
pub(super) fn is_plugin_command_name(name: &str) -> bool {
    if !name.contains('.') {
        return false;
    }
    if known_arity(name).is_some() {
        return false;
    }
    let first_segment = name.split('.').next().unwrap_or("");
    if first_segment.is_empty() {
        return false;
    }
    let mut chars = first_segment.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Check if `s` starts with a known command name (followed by whitespace or
/// `(`).
pub(super) fn is_command_start(s: &str) -> bool {
    let first = first_word(s);
    if known_arity(first).is_some() {
        return true;
    }
    // Plugin command names (dotted, not in known_arity) — routed by the
    // dispatcher's plugin fallback arm.
    if is_plugin_command_name(first) {
        return true;
    }
    // Also match command_name(...) directly
    if let Some(paren_pos) = s.find('(') {
        let candidate = &s[..paren_pos];
        if known_arity(candidate).is_some() {
            return true;
        }
        // Plugin command paren form: `plugin.sub(...)`
        if is_plugin_command_name(candidate) {
            return true;
        }
    }
    false
}

/// Check if `s` has the form `name(...)` — a known command with parens.
pub(super) fn is_paren_call(s: &str) -> bool {
    if let Some(paren_pos) = s.find('(') {
        let candidate = &s[..paren_pos];
        if candidate.contains(char::is_whitespace) {
            return false;
        }
        if known_arity(candidate).is_some() || is_plugin_command_name(candidate) {
            return s[paren_pos..].starts_with('(');
        }
    }
    false
}

// ── Token helpers ────────────────────────────────────────────────────────

/// Join remaining tokens for greedy RHS reconstruction.
pub(super) fn join_tokens_for_greedy(tokens: &[Token]) -> String {
    let parts: Vec<String> = tokens.iter().map(|t| token_raw_string(t)).collect();
    parts.join(", ")
}

/// Get the raw string representation of a token (as it appeared in source).
fn token_raw_string(token: &Token) -> String {
    match token {
        Token::Param(s) => s.clone(),
        Token::QuotedParam(s) => format!("\"{}\"", s),
        Token::Num(n) => n.to_string(),
        Token::Fraction(a, b) => format!("{a}/{b}"),
        Token::Sqrt(r) => format!("\u{221a}{r}"),
        Token::Pi => "\u{3c0}".to_string(),
        Token::Empty => "_".to_string(),
        Token::Command(c) => c.clone(),
    }
}

/// Classify a trimmed string as a literal `Expr` (no command call).
pub(super) fn classify_literal_str(s: &str) -> Expr {
    match s {
        "_" | "empty" => return Expr::Empty,
        "\u{3c0}" => return Expr::Pi,
        _ => {}
    }
    if let Some(rest) = s.strip_prefix('\u{221a}') {
        if !rest.is_empty() {
            return Expr::Sqrt(rest.to_string());
        }
    }
    if let Some(pos) = s.find('/') {
        if let Some(f) = try_parse_fraction_literal(s, pos) {
            return f;
        }
    }
    if let Ok(n) = s.parse::<i64>() {
        return Expr::Num(n);
    }
    Expr::Param(s.to_string())
}

/// Convert a lexer Token to an `Expr` literal (no command call).
pub(super) fn token_to_expr_literal(token: &Token) -> Expr {
    match token {
        Token::Param(s) => Expr::Param(s.clone()),
        Token::QuotedParam(s) => Expr::Param(s.clone()),
        Token::Num(n) => Expr::Num(*n),
        Token::Fraction(a, b) => Expr::Fraction(*a, *b),
        Token::Sqrt(r) => Expr::Sqrt(r.clone()),
        Token::Pi => Expr::Pi,
        Token::Empty => Expr::Empty,
        Token::Command(c) => Expr::Param(c.clone()),
    }
}

// ── Fraction helper (duplicated from lexer to avoid coupling) ─────────────

fn try_parse_fraction_literal(s: &str, pos: usize) -> Option<Expr> {
    if s.contains('\\') {
        return None;
    }
    let (left, right) = s.split_at(pos);
    let right = &right[1..];
    let numerator: i64 = left.trim().parse().ok()?;
    let denominator: i64 = right.trim().parse().ok()?;
    if denominator == 0 {
        return None;
    }
    Some(Expr::Fraction(numerator, denominator))
}
