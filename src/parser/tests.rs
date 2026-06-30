//! Unit tests for the dyyl parser: command grammar, greedy RHS,
//! parentheses, disambiguation, and edge cases.

use super::*;

/// Helper: parse a single line string and return the parsed command.
fn parse_one(source: &str) -> Result<ParsedCommand, ParseError> {
    let tokens = lexer::lex_line(source, 1).expect("lex should succeed");
    parse_line(&tokens, 1, source)
}

#[test]
fn parse_greedy_rhs_matches_paren_form() {
    let no_paren = parse_one("set $i, math.add $i, 1").expect("no-paren should parse");
    let with_paren = parse_one("set $i, math.add($i, 1)").expect("paren form should parse");

    assert_eq!(
        no_paren.call, with_paren.call,
        "set $i, math.add $i, 1 should be structurally equal to set $i, math.add($i, 1)"
    );

    let call = &no_paren.call;
    assert_eq!(call.command, "set", "outer command is set");
    assert_eq!(call.args.len(), 2, "set has 2 args");

    assert_eq!(
        call.args[0],
        Expr::Param("$i".to_string()),
        "first arg is $i"
    );

    match &call.args[1] {
        Expr::Call(inner) => {
            assert_eq!(inner.command, "math.add", "inner command is math.add");
            assert_eq!(inner.args.len(), 2, "math.add has 2 args");
            assert_eq!(
                inner.args[0],
                Expr::Param("$i".to_string()),
                "inner first arg is $i"
            );
            assert_eq!(inner.args[1], Expr::Num(1), "inner second arg is 1");
        }
        other => panic!("second arg should be a Call, got {other:?}"),
    }
}

#[test]
fn disambiguate_with_underscore() {
    let parsed =
        parse_one("math.add _, math.add 1, 2, 3").expect("disambiguated with _ should parse");

    assert_eq!(parsed.call.command, "math.add");
    assert_eq!(parsed.call.args.len(), 2);
    assert_eq!(parsed.call.args[0], Expr::Empty);
    match &parsed.call.args[1] {
        Expr::Call(inner) => {
            assert_eq!(inner.command, "math.add");
        }
        other => panic!("expected Call, got {other:?}"),
    }
}

#[test]
fn disambiguate_with_parens() {
    let parsed =
        parse_one("math.add math.add(1, 2), 3").expect("disambiguated with () should parse");

    assert_eq!(parsed.call.command, "math.add");
    assert_eq!(parsed.call.args.len(), 2);

    match &parsed.call.args[0] {
        Expr::Call(inner) => {
            assert_eq!(inner.command, "math.add");
            assert_eq!(inner.args.len(), 2);
            assert_eq!(inner.args[0], Expr::Num(1));
            assert_eq!(inner.args[1], Expr::Num(2));
        }
        other => panic!("expected Call, got {other:?}"),
    }

    match &parsed.call.args[1] {
        Expr::Num(3) => {}
        other => panic!("expected Num(3), got {other:?}"),
    }
}

#[test]
fn left_nested_call_resolves_by_arity() {
    let parsed = parse_one("math.add math.add 1, 2, 3")
        .expect("left-nested should resolve by arity");
    assert_eq!(parsed.call.command, "math.add");
    assert_eq!(parsed.call.args.len(), 2);
    match &parsed.call.args[0] {
        Expr::Call(inner) => {
            assert_eq!(inner.command, "math.add");
            assert_eq!(inner.args.len(), 2);
            assert_eq!(inner.args[0], Expr::Num(1));
            assert_eq!(inner.args[1], Expr::Num(2));
        }
        other => panic!("expected inner Call, got {other:?}"),
    }
    assert_eq!(parsed.call.args[1], Expr::Num(3));
}

#[test]
fn logic_while_logic_same_resolves_by_arity() {
    let parsed = parse_one("logic.while logic.same a,0,3")
        .expect("logic.while logic.same should resolve by arity");
    assert_eq!(parsed.call.command, "logic.while");
    assert_eq!(parsed.call.args.len(), 2);
    match &parsed.call.args[0] {
        Expr::Call(inner) => {
            assert_eq!(inner.command, "logic.same");
            assert_eq!(inner.args.len(), 2);
            assert_eq!(inner.args[0], Expr::Param("a".to_string()));
            assert_eq!(inner.args[1], Expr::Num(0));
        }
        other => panic!("expected inner Call, got {other:?}"),
    }
    assert_eq!(parsed.call.args[1], Expr::Num(3));
}

#[test]
fn parse_simple_commands() {
    let parsed = parse_one("io.out hello").expect("should parse");
    assert_eq!(parsed.call.command, "io.out");
    assert_eq!(parsed.call.args.len(), 1);
    assert_eq!(parsed.call.args[0], Expr::Param("hello".to_string()));

    let parsed = parse_one("logic.or 1, 0").expect("should parse");
    assert_eq!(parsed.call.command, "logic.or");
    assert_eq!(parsed.call.args.len(), 2);
}

#[test]
fn parse_arity_one_is_greedy() {
    let parsed = parse_one("io.out hello world").expect("should parse");
    assert_eq!(parsed.call.args.len(), 1);
    assert_eq!(parsed.call.args[0], Expr::Param("hello world".to_string()));
}

#[test]
fn parse_arity_one_is_greedy_with_nested() {
    let parsed = parse_one("io.out math.add 1, 2").expect("should parse");
    assert_eq!(parsed.call.args.len(), 1);
    match &parsed.call.args[0] {
        Expr::Call(inner) => {
            assert_eq!(inner.command, "math.add");
            assert_eq!(inner.args.len(), 2);
        }
        other => panic!("expected Call, got {other:?}"),
    }
}

#[test]
fn unknown_arity_is_variable() {
    let parsed = parse_one("unknown.cmd a, b, c").expect("should parse");
    assert_eq!(parsed.call.command, "unknown.cmd");
    assert_eq!(parsed.call.args.len(), 3);
    assert_eq!(parsed.call.args[0], Expr::Param("a".to_string()));
    assert_eq!(parsed.call.args[1], Expr::Param("b".to_string()));
    assert_eq!(parsed.call.args[2], Expr::Param("c".to_string()));
}

#[test]
fn parse_source_handles_multiple_lines() {
    let source = "set $i, 1\nmath.add $i, 2\n";
    let commands = parse_source(source).expect("should parse");
    assert_eq!(commands.len(), 2);
    assert_eq!(commands[0].call.command, "set");
    assert_eq!(commands[1].call.command, "math.add");
}

#[test]
fn placeholder_tokens() {
    let parsed = parse_one("math.add _, _").expect("should parse");
    assert_eq!(parsed.call.args.len(), 2);
    assert_eq!(parsed.call.args[0], Expr::Empty);
    assert_eq!(parsed.call.args[1], Expr::Empty);
}

#[test]
fn parse_paren_call_as_top_level() {
    let parsed = parse_one("math.add($i, 1)").expect("should parse");
    assert_eq!(parsed.call.command, "math.add");
    assert_eq!(parsed.call.args.len(), 2);
    assert_eq!(parsed.call.args[0], Expr::Param("$i".to_string()));
    assert_eq!(parsed.call.args[1], Expr::Num(1));
}

#[test]
fn parse_source_empty_is_empty() {
    let commands = parse_source("").expect("empty should parse");
    assert!(commands.is_empty());
}

#[test]
fn parse_source_comments_only() {
    let commands = parse_source("# comment\n  # another\n").expect("comments should parse");
    assert!(commands.is_empty());
}
