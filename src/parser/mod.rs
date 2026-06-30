use crate::lexer::{self, Token};
use crate::parser::types::{Call, Expr, ParseError, ParsedCommand};

pub mod arity;
mod helpers;
pub mod types;

#[cfg(test)]
mod tests;

// ── Public API ───────────────────────────────────────────────────────────

/// Parse a complete dyyl source string into parsed commands.
///
/// Iterates source lines individually so line numbers are correct even when
/// comment/empty lines are skipped by the lexer.
pub fn parse_source(source: &str) -> Result<Vec<ParsedCommand>, ParseError> {
    let mut commands = Vec::new();
    for (i, raw_line) in source.lines().enumerate() {
        let line_number = i + 1;
        match lexer::lex_line(raw_line, line_number) {
            Ok(tokens) if tokens.is_empty() => continue,
            Ok(tokens) => {
                let parsed = parse_line(&tokens, line_number, raw_line)?;
                commands.push(parsed);
            }
            Err(e) => {
                return Err(ParseError {
                    line: e.line,
                    text: raw_line.to_string(),
                    message: e.message,
                });
            }
        }
    }
    Ok(commands)
}

/// Parse a single line's token stream into a `ParsedCommand`.
///
/// `line` is 1-based; `text` is the raw source line.
fn parse_line(tokens: &[Token], line: usize, text: &str) -> Result<ParsedCommand, ParseError> {
    let cmd_token = tokens.first().ok_or_else(|| ParseError {
        line,
        text: text.to_string(),
        message: "empty token line".to_string(),
    })?;

    let command = match cmd_token {
        Token::Command(c) => c.clone(),
        _ => {
            return Err(ParseError {
                line,
                text: text.to_string(),
                message: format!("expected command token, got {cmd_token:?}"),
            });
        }
    };

    let call = parse_call_from_tokens(&command, &tokens[1..], line, text)?;
    Ok(ParsedCommand {
        line,
        text: text.to_string(),
        call,
    })
}

// ── Core parsing ─────────────────────────────────────────────────────────

/// Parse a command call given the command name and remaining param tokens.
fn parse_call_from_tokens(
    command: &str,
    param_tokens: &[Token],
    line: usize,
    text: &str,
) -> Result<Call, ParseError> {
    let arity = arity::known_arity(command);

    // Determine the split between non-greedy and greedy params.
    let non_greedy_count = match arity {
        Some(k) if k == 0 => 0,
        Some(k) if param_tokens.len() > k => {
            if k == 1 {
                0
            } else {
                k - 1
            }
        }
        Some(k) => {
            if k == 1 {
                0
            } else if param_tokens.len() >= k {
                k - 1
            } else {
                param_tokens.len()
            }
        }
        None => param_tokens.len(),
    };

    let mut args: Vec<Expr> = Vec::new();

    // --- Non-greedy params (may consume multiple tokens for nested calls) ---
    let mut i = 0;
    let mut consumed_non_greedy = 0;
    while consumed_non_greedy < non_greedy_count && i < param_tokens.len() {
        let token = &param_tokens[i];

        // Try to parse as a nested call without parentheses by looking ahead
        if let Token::Param(s) = token {
            if helpers::is_command_start(s) {
                let first = helpers::first_word(s);

                // Zero-arity commands: parse as nested call directly
                if let Some(0) = crate::parser::arity::known_arity(first) {
                    let call = parse_call_from_tokens(first, &[], line, text)?;
                    args.push(Expr::Call(call));
                    i += 1;
                    consumed_non_greedy += 1;
                    continue;
                }

                // Paren calls: parse with parentheses
                if helpers::is_paren_call(s) {
                    let call = parse_paren_call_str(s, line, text)?;
                    args.push(Expr::Call(call));
                    i += 1;
                    consumed_non_greedy += 1;
                    continue;
                }

                // Non-paren nested call: try to resolve by arity
                if let Some(nested_arity) = crate::parser::arity::known_arity(first) {
                    if nested_arity > 0 {
                        // Extract first param from current token (after command name)
                        let first_param = s[first.len()..].trim();
                        let has_first_param = !first_param.is_empty();
                        let params_needed = if has_first_param {
                            nested_arity - 1
                        } else {
                            nested_arity
                        };

                        // Check if we have enough remaining tokens
                        let remaining_after_current = &param_tokens[i + 1..];
                        if remaining_after_current.len() >= params_needed {
                            // Also ensure the outer command still gets its greedy param
                            let tokens_left =
                                remaining_after_current.len() - params_needed;
                            let outer_has_greedy = non_greedy_count < arity.unwrap_or(0);
                            if !outer_has_greedy || tokens_left >= 1 {
                                // Build inner tokens for the nested call
                                let mut inner_tokens: Vec<Token> = Vec::new();
                                inner_tokens.push(Token::Command(first.to_string()));
                                if has_first_param {
                                    // Lex the first param to get correct token type
                                    let synthetic = format!("{first} {first_param}");
                                    let lexed = lexer::lex_line(&synthetic, line)
                                        .map_err(|e| ParseError {
                                            line,
                                            text: text.to_string(),
                                            message: format!(
                                                "parse error in nested call: {}",
                                                e.message
                                            ),
                                        })?;
                                    // Skip the command token (index 0), take the param
                                    if lexed.len() > 1 {
                                        inner_tokens.push(lexed[1].clone());
                                    }
                                }
                                for t in remaining_after_current.iter().take(params_needed)
                                {
                                    inner_tokens.push(t.clone());
                                }

                                let call = parse_call_from_tokens(
                                    first,
                                    &inner_tokens[1..],
                                    line,
                                    text,
                                )?;
                                args.push(Expr::Call(call));

                                // Skip consumed tokens
                                i += 1 + params_needed;
                                consumed_non_greedy += 1;
                                continue;
                            }
                        }
                    }
                }
            }
        }

        // Fall back: single-token non-greedy param
        let expr = parse_non_greedy_token(token, line, text, command)?;
        args.push(expr);
        i += 1;
        consumed_non_greedy += 1;
    }

    // The greedy param starts after all consumed tokens
    let actual_greedy_start = i;

    // --- Greedy last param (rest of tokens joined) ---
    if actual_greedy_start < param_tokens.len() {
        let is_greedy = match arity {
            Some(k) => {
                let last_param_idx = if k == 0 { 0 } else { k - 1 };
                non_greedy_count <= last_param_idx
            }
            None => false,
        };

        let remaining_tokens = &param_tokens[actual_greedy_start..];

        if is_greedy && !remaining_tokens.is_empty() {
            let expr = if remaining_tokens.len() == 1 {
                if let Token::QuotedParam(s) = &remaining_tokens[0] {
                    Expr::Param(s.clone())
                } else {
                    let greedy_str = helpers::join_tokens_for_greedy(remaining_tokens);
                    parse_greedy_rhs(&greedy_str, line, text, command)?
                }
            } else {
                let greedy_str = helpers::join_tokens_for_greedy(remaining_tokens);
                parse_greedy_rhs(&greedy_str, line, text, command)?
            };
            args.push(expr);
        } else if !is_greedy {
            for token in remaining_tokens {
                let expr = helpers::token_to_expr_literal(token);
                args.push(expr);
            }
        }
    }

    Ok(Call {
        command: command.to_string(),
        args,
    })
}

/// Parse a non-greedy (single-token) param position.
///
/// If the token is a `Param(string)` and the string starts with a known
/// command name, this is a LEFT AMBIGUITY — the caller must use `_` or `()`
/// to disambiguate, UNLESS the string has the form `name(...)` (explicit
/// parenthesized call).
fn parse_non_greedy_token(
    token: &Token,
    line: usize,
    text: &str,
    parent_command: &str,
) -> Result<Expr, ParseError> {
    match token {
        Token::QuotedParam(s) => Ok(Expr::Param(s.clone())),
        Token::Param(s) if helpers::is_command_start(s) => {
            // Zero-arity commands can't own parameters — parse as nested call
            let first = helpers::first_word(s);
            if let Some(0) = crate::parser::arity::known_arity(first) {
                // Parse as a nested zero-arity command call
                let call = parse_call_from_tokens(first, &[], line, text);
                return call.map(Expr::Call);
            }

            if helpers::is_paren_call(s) {
                parse_paren_call_str(s, line, text).map(Expr::Call)
            } else {
                Err(ParseError {
                    line,
                    text: text.to_string(),
                    message: format!(
                        "line {}: ambiguous left-nested call in '{command}' — \
                     param starts with '{cmd_name}' but has no parentheses; \
                     use _ or () to disambiguate",
                        line,
                        command = parent_command,
                        cmd_name = helpers::first_word(s),
                    ),
                })
            }
        }
        _ => Ok(helpers::token_to_expr_literal(token)),
    }
}

/// Parse the greedy RHS string: if it starts with a known command, parse it
/// as a nested call; otherwise treat as a literal expression.
fn parse_greedy_rhs(
    s: &str,
    line: usize,
    text: &str,
    _parent_command: &str,
) -> Result<Expr, ParseError> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return Ok(Expr::Empty);
    }

    if helpers::is_command_start(trimmed) {
        let inner_tokens = lexer::lex_line(trimmed, line).map_err(|e| ParseError {
            line,
            text: text.to_string(),
            message: format!("parse error in nested call: {}", e.message),
        })?;

        if inner_tokens.is_empty() {
            return Ok(Expr::Param(trimmed.to_string()));
        }

        let cmd = match &inner_tokens[0] {
            Token::Command(c) => c.clone(),
            _ => {
                return Ok(Expr::Param(trimmed.to_string()));
            }
        };

        let call = parse_call_from_tokens(&cmd, &inner_tokens[1..], line, text)?;
        Ok(Expr::Call(call))
    } else {
        Ok(helpers::classify_literal_str(trimmed))
    }
}

// ── Paren call handling ──────────────────────────────────────────────────

/// Parse a string of the form `name(...)` as a nested call.
fn parse_paren_call_str(s: &str, line: usize, text: &str) -> Result<Call, ParseError> {
    let paren_pos = s.find('(').ok_or_else(|| ParseError {
        line,
        text: text.to_string(),
        message: "expected '(' in paren call".to_string(),
    })?;

    let command = &s[..paren_pos];
    let inner = &s[paren_pos + 1..];
    let mut depth: u32 = 1;
    let mut end = inner.len();
    for (i, c) in inner.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    end = i;
                    break;
                }
            }
            _ => {}
        }
    }
    let inner_content = &inner[..end];

    let synthetic_line = format!("{command} {inner_content}");
    let inner_tokens = lexer::lex_line(&synthetic_line, line).map_err(|e| ParseError {
        line,
        text: text.to_string(),
        message: format!("parse error in paren call: {}", e.message),
    })?;

    if inner_tokens.is_empty() {
        return Ok(Call {
            command: command.to_string(),
            args: Vec::new(),
        });
    }

    parse_call_from_tokens(command, &inner_tokens[1..], line, text)
}
