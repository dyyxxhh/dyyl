//! Escape-handling integration tests for the dyyl lexer.

use dyyl::lexer;

// ── Escaping inside quotes ────────────────────────────────────────────

#[test]
fn backslash_newline_inside_quotes() {
    let tokens = lexer::lex_line(r#"cmd "a\nb""#, 1).expect("should lex");
    assert_eq!(tokens.len(), 2);
    assert_eq!(tokens[1], lexer::Token::QuotedParam("a\nb".to_string()));
}

#[test]
fn backslash_tab_inside_quotes() {
    let tokens = lexer::lex_line(r#"cmd "a\tb""#, 1).expect("should lex");
    assert_eq!(tokens.len(), 2);
    assert_eq!(tokens[1], lexer::Token::QuotedParam("a\tb".to_string()));
}

#[test]
fn backslash_quote_inside_quotes() {
    let tokens = lexer::lex_line(r#"cmd "a\"b""#, 1).expect("should lex");
    assert_eq!(tokens.len(), 2);
    assert_eq!(tokens[1], lexer::Token::QuotedParam("a\"b".to_string()));
}

#[test]
fn backslash_backslash_inside_quotes() {
    let tokens = lexer::lex_line(r#"cmd "a\\b""#, 1).expect("should lex");
    assert_eq!(tokens.len(), 2);
    assert_eq!(tokens[1], lexer::Token::QuotedParam("a\\b".to_string()));
}

// ── Escaping in bare words ────────────────────────────────────────────

#[test]
fn escaped_comma_in_bare_word() {
    let tokens = lexer::lex_line(r#"cmd hello\, world"#, 1).expect("should lex");
    assert_eq!(tokens.len(), 2);
    assert_eq!(tokens[1], lexer::Token::Param("hello, world".to_string()));
}

#[test]
fn escaped_backslash_in_bare_word() {
    let tokens = lexer::lex_line(r#"cmd hello\\world"#, 1).expect("should lex");
    assert_eq!(tokens.len(), 2);
    assert_eq!(tokens[1], lexer::Token::Param("hello\\world".to_string()));
}

#[test]
fn escaped_quote_in_bare_word() {
    let tokens = lexer::lex_line(r#"cmd hello\"world"#, 1).expect("should lex");
    assert_eq!(tokens.len(), 2);
    assert_eq!(tokens[1], lexer::Token::Param("hello\"world".to_string()));
}
