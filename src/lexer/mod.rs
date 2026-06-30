//! dyyl lexer — lexical scanning for one command per line.
//!
//! Splits dyyl source into tokens: command name and typed parameters.
//! Handles inline `#` comments (outside quoted strings), optional double-quoted
//! strings, backslash escaping, bare commas as parameter delimiters, numeric
//! literal lexemes (`1/3`, `√2`, `π`), and no continuation lines.

pub mod types;

pub use types::{LexError, Token};

// ── Public API ────────────────────────────────────────────────────────

/// Lex a complete dyyl source string, returning one token-vector per
/// non-empty, non-comment line.
pub fn lex_source(source: &str) -> Result<Vec<Vec<Token>>, LexError> {
    let mut lines: Vec<Vec<Token>> = Vec::new();
    for (i, raw_line) in source.lines().enumerate() {
        let line_number = i + 1;
        let tokens = lex_line(raw_line, line_number)?;
        if !tokens.is_empty() {
            lines.push(tokens);
        }
    }
    Ok(lines)
}

/// Lex a single dyyl source line: strip comments, reject continuation,
/// extract command, split comma-separated parameters, classify each.
pub fn lex_line(line: &str, line_number: usize) -> Result<Vec<Token>, LexError> {
    let cleaned = preprocess_line(line, line_number)?;
    let cleaned = cleaned.trim();
    if cleaned.is_empty() {
        return Ok(Vec::new());
    }

    let (cmd, rest) = split_command(cleaned);
    let mut tokens = vec![Token::Command(cmd.to_string())];
    if rest.is_empty() {
        return Ok(tokens);
    }

    let param_strs = split_params(rest, line_number)?;
    for ps in &param_strs {
        tokens.push(classify_param(ps));
    }
    Ok(tokens)
}

// ── Internal helpers ──────────────────────────────────────────────────

/// Remove inline `#` comments (respecting quoted strings) and check for
/// unsupported continuation (bare backslash at end of line).
fn preprocess_line(line: &str, line_number: usize) -> Result<String, LexError> {
    let mut out = String::with_capacity(line.len());
    let mut in_quotes = false;
    let mut escape_next = false;

    for c in line.chars() {
        if escape_next {
            out.push('\\');
            out.push(c);
            escape_next = false;
            continue;
        }
        match c {
            '\\' => escape_next = true,
            '"' => {
                in_quotes = !in_quotes;
                out.push('"');
            }
            '#' if !in_quotes => break,
            _ => out.push(c),
        }
    }

    if escape_next && !in_quotes {
        return Err(LexError {
            line: line_number,
            message: format!("line {}: continuation not supported", line_number),
        });
    }
    if escape_next {
        out.push('\\');
    }
    if in_quotes {
        return Err(LexError {
            line: line_number,
            message: "unterminated quoted string".to_string(),
        });
    }
    Ok(out)
}

/// Split cleaned line into (command, rest).
///
/// Recognises `name(...)` at the start when there is no whitespace before `(`.
/// Falls back to splitting at first ASCII whitespace.
///
/// When parens are found, the matched closing paren delimits `rest` (the content
/// inside the parens).  Any trailing text after `)` is discarded — the dyyl
/// command call is the whole line.
fn split_command(line: &str) -> (&str, &str) {
    // Check for name(...) pattern: alphanumeric/dot/underscore chars
    // immediately followed by `(`.
    if let Some(paren_pos) = line.find('(') {
        let cmd_candidate = &line[..paren_pos];
        if !cmd_candidate.is_empty() && !cmd_candidate.contains(|c: char| c.is_ascii_whitespace()) {
            let after_paren = &line[paren_pos + 1..];
            let mut depth: u32 = 1;
            for (i, c) in after_paren.char_indices() {
                match c {
                    '(' => depth += 1,
                    ')' => {
                        depth -= 1;
                        if depth == 0 {
                            return (cmd_candidate, &after_paren[..i]);
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    // Fallback: split at first whitespace.
    for (i, c) in line.char_indices() {
        if c.is_ascii_whitespace() {
            return (&line[..i], line[i..].trim_start());
        }
    }
    (line, "")
}

/// Split parameter section into individual param strings.
/// Comma delimits only when outside quotes, outside parens, and not
/// backslash-escaped.
fn split_params(rest: &str, line_number: usize) -> Result<Vec<String>, LexError> {
    let mut params: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut paren_depth: u32 = 0;
    let mut escape_next = false;

    for c in rest.chars() {
        if escape_next {
            current.push('\\');
            current.push(c);
            escape_next = false;
            continue;
        }
        match c {
            '\\' => escape_next = true,
            '"' => {
                in_quotes = !in_quotes;
                current.push('"');
            }
            '(' if !in_quotes => {
                paren_depth += 1;
                current.push('(');
            }
            ')' if !in_quotes => {
                paren_depth = paren_depth.saturating_sub(1);
                current.push(')');
            }
            ',' if !in_quotes && paren_depth == 0 => {
                let trimmed = current.trim().to_string();
                if !trimmed.is_empty() {
                    params.push(trimmed);
                }
                current.clear();
            }
            _ => {
                current.push(c);
            }
        }
    }

    if escape_next {
        return Err(LexError {
            line: line_number,
            message: "dangling escape at end of line".to_string(),
        });
    }
    if in_quotes {
        return Err(LexError {
            line: line_number,
            message: "unterminated quoted string".to_string(),
        });
    }

    let last = current.trim().to_string();
    if !last.is_empty() {
        params.push(last);
    }
    Ok(params)
}

/// Classify a trimmed parameter string into a `Token`.
fn classify_param(s: &str) -> Token {
    match s {
        "_" | "empty" => return Token::Empty,
        "\u{3c0}" => return Token::Pi,
        _ => {}
    }
    if s.starts_with('"') {
        if let Some(inner) = extract_quoted_content(s) {
            return Token::QuotedParam(resolve_escapes(&inner));
        }
    }
    if let Some(rest) = s.strip_prefix('\u{221a}') {
        if !rest.is_empty() {
            return Token::Sqrt(rest.to_string());
        }
    }
    if let Some(pos) = s.find('/') {
        if let Some(f) = try_parse_fraction(s, pos) {
            return f;
        }
    }
    if let Ok(n) = s.parse::<i64>() {
        return Token::Num(n);
    }
    Token::Param(resolve_escapes(s))
}

/// Try to parse `s` as a fraction `a/b` with `/` at `pos`.
fn try_parse_fraction(s: &str, pos: usize) -> Option<Token> {
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
    Some(Token::Fraction(numerator, denominator))
}

/// Extract content inside a quoted string `"..."` verbatim.
fn extract_quoted_content(s: &str) -> Option<String> {
    let inner = s.get(1..)?;
    let mut result = String::new();
    let mut chars = inner.chars();
    let mut escape_next = false;
    loop {
        match chars.next() {
            None => return None,
            Some('\\') if !escape_next => {
                escape_next = true;
                result.push('\\');
            }
            Some('"') if !escape_next => return Some(result),
            Some(c) => {
                escape_next = false;
                result.push(c);
            }
        }
    }
}

/// Resolve backslash escape sequences in a string.
fn resolve_escapes(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars();
    let mut escape_next = false;
    while let Some(c) = chars.next() {
        if escape_next {
            match c {
                'n' => result.push('\n'),
                't' => result.push('\t'),
                '\\' => result.push('\\'),
                '"' => result.push('"'),
                _ => result.push(c),
            }
            escape_next = false;
        } else if c == '\\' {
            escape_next = true;
        } else {
            result.push(c);
        }
    }
    if escape_next {
        result.push('\\');
    }
    result
}
