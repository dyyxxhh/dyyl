//! Integration tests for the dyyl lexer.
//!
//! RED phase: these tests MUST fail because the lexer is still a stub.
//! They will pass once the full lexer implementation is complete.

use dyyl::lexer;

// ── Happy path: the main acceptance test ──────────────────────────────

/// Covers inline comments, bare-word params, quoted params, escaped comma,
/// escaped space, `\n`/`\t`, string `#`, bare comma delimiter, and numeric
/// literals (`1/3`, `√2`, `π`).
#[test]
fn parser_lexes_comments_strings_literals_quoting_escaping() {
    let source = r#"
# this is a full-line comment
io.out hello world
io.out "hello world"
io.out hello\, world
io.out "line one\nline two"
io.out hello#trailing comment
set $i, 42, 1/3
io.out √2
io.out π
"#;

    let lines = lexer::lex_source(source).expect("lex_source should succeed");

    // Line 1: full-line comment → skipped
    // Line 2: io.out hello world → one param "hello world"
    assert_eq!(
        lines[0][0],
        lexer::Token::Command("io.out".to_string()),
        "line 2: command"
    );
    assert_eq!(lines[0].len(), 2, "line 2: command + 1 param");

    // Line 3: io.out "hello world" → quoted param
    assert_eq!(
        lines[1][0],
        lexer::Token::Command("io.out".to_string()),
        "line 3: command"
    );
    assert_eq!(
        lines[1][1],
        lexer::Token::QuotedParam("hello world".to_string()),
        "line 3: quoted param"
    );

    // Line 4: io.out hello\, world → escaped comma in bare word
    assert_eq!(
        lines[2][0],
        lexer::Token::Command("io.out".to_string()),
        "line 4: command"
    );
    assert_eq!(
        lines[2][1],
        lexer::Token::Param("hello, world".to_string()),
        "line 4: escaped comma"
    );

    // Line 5: io.out "line one\nline two" → standard escapes in quotes
    assert_eq!(
        lines[3][0],
        lexer::Token::Command("io.out".to_string()),
        "line 5: command"
    );
    assert_eq!(
        lines[3][1],
        lexer::Token::QuotedParam("line one\nline two".to_string()),
        "line 5: newline escape"
    );

    // Line 6: io.out hello#trailing comment → param is just "hello"
    assert_eq!(
        lines[4][0],
        lexer::Token::Command("io.out".to_string()),
        "line 6: command"
    );
    assert_eq!(
        lines[4][1],
        lexer::Token::Param("hello".to_string()),
        "line 6: comment stripped"
    );

    // Line 7: set $i, 42, 1/3 → three params: $i, 42, 1/3
    assert_eq!(
        lines[5][0],
        lexer::Token::Command("set".to_string()),
        "line 7: command"
    );
    assert_eq!(
        lines[5][1],
        lexer::Token::Param("$i".to_string()),
        "line 7: var param"
    );
    assert_eq!(lines[5][2], lexer::Token::Num(42), "line 7: int literal");
    assert_eq!(
        lines[5][3],
        lexer::Token::Fraction(1, 3),
        "line 7: fraction literal"
    );

    // Line 8: io.out √2 → sqrt literal
    assert_eq!(
        lines[6][0],
        lexer::Token::Command("io.out".to_string()),
        "line 8: command"
    );
    assert_eq!(
        lines[6][1],
        lexer::Token::Sqrt("2".to_string()),
        "line 8: sqrt literal"
    );

    // Line 9: io.out π → pi constant
    assert_eq!(
        lines[7][0],
        lexer::Token::Command("io.out".to_string()),
        "line 9: command"
    );
    assert_eq!(lines[7][1], lexer::Token::Pi, "line 9: pi literal");
}

// ── Comment edge cases ────────────────────────────────────────────────

#[test]
fn comment_inside_quotes_is_preserved() {
    let source = r#"io.out "hello # world""#;
    let tokens = lexer::lex_line(source, 1).expect("should lex");
    assert_eq!(tokens.len(), 2);
    assert_eq!(
        tokens[1],
        lexer::Token::QuotedParam("hello # world".to_string())
    );
}

#[test]
fn hash_without_comment_at_start() {
    let tokens = lexer::lex_line("#", 1).expect("should lex");
    assert!(tokens.is_empty(), "comment-only line yields no tokens");
}

#[test]
fn hash_in_bare_word_is_comment() {
    let source = "cmd before#after";
    let tokens = lexer::lex_line(source, 1).expect("should lex");
    assert_eq!(tokens.len(), 2);
    assert_eq!(tokens[1], lexer::Token::Param("before".to_string()));
}

// ── Quoting edge cases ────────────────────────────────────────────────

#[test]
fn empty_quoted_string() {
    let tokens = lexer::lex_line(r#"cmd """#, 1).expect("should lex");
    assert_eq!(tokens.len(), 2);
    assert_eq!(tokens[1], lexer::Token::QuotedParam(String::new()));
}

#[test]
fn unterminated_quote_is_error() {
    let result = lexer::lex_line(r#"cmd "hello"#, 1);
    assert!(result.is_err(), "unterminated quote should error");
    let err = result.unwrap_err();
    assert_eq!(err.line, 1);
}

// ── Continuation rejection ────────────────────────────────────────────

#[test]
fn continuation_backslash_at_end_of_line_is_error() {
    let source = r#"io.out "a" \"#;
    let result = lexer::lex_line(source, 1);
    assert!(result.is_err(), "continuation should be rejected");
    let err = result.unwrap_err();
    assert_eq!(err.line, 1);
    assert!(
        err.message.contains("continuation"),
        "error should mention continuation: {}",
        err.message
    );
}

// ── Numeric literals ──────────────────────────────────────────────────

#[test]
fn numeric_literals() {
    let source = "cmd 42, -7, 0";
    let tokens = lexer::lex_line(source, 1).expect("should lex");
    assert_eq!(tokens[1], lexer::Token::Num(42));
    assert_eq!(tokens[2], lexer::Token::Num(-7));
    assert_eq!(tokens[3], lexer::Token::Num(0));
}

#[test]
fn fraction_literal() {
    let tokens = lexer::lex_line("cmd 1/3", 1).expect("should lex");
    assert_eq!(tokens[1], lexer::Token::Fraction(1, 3));
}

#[test]
fn pi_literal() {
    let tokens = lexer::lex_line("cmd π", 1).expect("should lex");
    assert_eq!(tokens[1], lexer::Token::Pi);
}

#[test]
fn sqrt_literal() {
    let tokens = lexer::lex_line("cmd √2", 1).expect("should lex");
    assert_eq!(tokens[1], lexer::Token::Sqrt("2".to_string()));
}

// ── Comma delimiters ──────────────────────────────────────────────────

#[test]
fn bare_comma_delimits_params() {
    let source = "cmd a, b, c";
    let tokens = lexer::lex_line(source, 1).expect("should lex");
    assert_eq!(tokens.len(), 4);
    assert_eq!(tokens[1], lexer::Token::Param("a".to_string()));
    assert_eq!(tokens[2], lexer::Token::Param("b".to_string()));
    assert_eq!(tokens[3], lexer::Token::Param("c".to_string()));
}

#[test]
fn comma_inside_quotes_is_not_delimiter() {
    let source = r#"cmd "a, b", c"#;
    let tokens = lexer::lex_line(source, 1).expect("should lex");
    assert_eq!(tokens.len(), 3);
    assert_eq!(tokens[1], lexer::Token::QuotedParam("a, b".to_string()));
    assert_eq!(tokens[2], lexer::Token::Param("c".to_string()));
}

#[test]
fn escaped_comma_is_not_delimiter() {
    let source = r"cmd a\, b, c";
    let tokens = lexer::lex_line(source, 1).expect("should lex");
    assert_eq!(tokens.len(), 3);
    assert_eq!(tokens[1], lexer::Token::Param("a, b".to_string()));
    assert_eq!(tokens[2], lexer::Token::Param("c".to_string()));
}

// ── Empty / placeholder ───────────────────────────────────────────────

#[test]
fn underscore_is_empty_placeholder() {
    let tokens = lexer::lex_line("cmd _, empty", 1).expect("should lex");
    assert_eq!(tokens.len(), 3);
    assert_eq!(tokens[1], lexer::Token::Empty);
    assert_eq!(tokens[2], lexer::Token::Empty);
}

// ── lex_source multiline ──────────────────────────────────────────────

#[test]
fn lex_source_empty_returns_empty() {
    let lines = lexer::lex_source("").expect("empty source should be ok");
    assert!(lines.is_empty());
}

#[test]
fn lex_source_comment_only_returns_empty() {
    let lines = lexer::lex_source("# just a comment\n  # another").expect("should be ok");
    assert!(lines.is_empty());
}

#[test]
fn lex_source_multiple_lines() {
    let source = "cmd1 a\ncmd2 b, c\ncmd3";
    let lines = lexer::lex_source(source).expect("should succeed");
    assert_eq!(lines.len(), 3);
    assert_eq!(lines[0][0], lexer::Token::Command("cmd1".to_string()));
    assert_eq!(lines[1][0], lexer::Token::Command("cmd2".to_string()));
    assert_eq!(lines[2][0], lexer::Token::Command("cmd3".to_string()));
}

#[test]
fn lex_source_continuation_error_propagates() {
    let source = "ok\nbad \\\nok2";
    let result = lexer::lex_source(source);
    assert!(result.is_err(), "continuation on line 2 should be an error");
    let err = result.unwrap_err();
    assert_eq!(err.line, 2);
}
