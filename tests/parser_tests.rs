#![allow(
    clippy::all,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::panic,
    clippy::expect_used,
    clippy::todo,
    clippy::unimplemented,
    clippy::as_underscore,
    clippy::fn_to_numeric_cast_any,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::redundant_pub_crate,
    clippy::missing_const_for_fn
)]
//! Integration tests for the dyyl parser: command grammar, greedy RHS,
//! parentheses, and placeholder disambiguation.

use dyyl::parser;
use dyyl::parser::types::Expr;

#[test]
fn parser_handles_greedy_rhs_and_disambiguation() {
    // --- Greedy RHS: both forms produce the same AST ---

    // Form A: comma-separated params with implicit greedy RHS
    let result_a = parser::parse_source("set $i, math.add $i, 1");
    assert!(
        result_a.is_ok(),
        "greedy RHS form should parse: {:?}",
        result_a
    );
    let cmds_a = result_a.unwrap();
    assert_eq!(cmds_a.len(), 1);
    let call_a = &cmds_a[0].call;

    // Form B: explicit parenthesized call
    let result_b = parser::parse_source("set $i, math.add($i, 1)");
    assert!(result_b.is_ok(), "paren form should parse: {:?}", result_b);
    let cmds_b = result_b.unwrap();
    let call_b = &cmds_b[0].call;

    // The two forms must be structurally equal
    assert_eq!(
        call_a, call_b,
        "set $i, math.add $i, 1 should equal set $i, math.add($i, 1)"
    );

    // Verify the structure: set($i, math.add($i, 1))
    assert_eq!(call_a.command, "set", "outer command is set");
    assert_eq!(call_a.args.len(), 2, "set has 2 args");

    // First arg is the variable reference
    assert_eq!(
        call_a.args[0],
        Expr::Param("$i".to_string()),
        "first arg is $i"
    );

    // Second arg is a nested Call
    match &call_a.args[1] {
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

    // --- Disambiguation with `_` placeholder ---

    // `math.add _, math.add 1, 2, 3` — _ fills first param, rest forms greedy RHS
    let disambig = parser::parse_source("math.add _, math.add 1, 2, 3");
    assert!(
        disambig.is_ok(),
        "disambiguated with _ should parse: {:?}",
        disambig
    );
    let d_cmds = disambig.unwrap();
    assert_eq!(d_cmds[0].call.command, "math.add");
    assert_eq!(d_cmds[0].call.args.len(), 2);
    assert_eq!(d_cmds[0].call.args[0], Expr::Empty);
    match &d_cmds[0].call.args[1] {
        Expr::Call(inner) => {
            assert_eq!(inner.command, "math.add");
        }
        other => panic!("expected Call, got {other:?}"),
    }

    // --- Disambiguation with `()` parentheses ---

    // `math.add math.add(1, 2), 3` — parens make inner call explicit
    let parens = parser::parse_source("math.add math.add(1, 2), 3");
    assert!(
        parens.is_ok(),
        "disambiguated with () should parse: {:?}",
        parens
    );
    let p_cmds = parens.unwrap();
    assert_eq!(p_cmds[0].call.command, "math.add");
    assert_eq!(p_cmds[0].call.args.len(), 2);

    match &p_cmds[0].call.args[0] {
        Expr::Call(inner) => {
            assert_eq!(inner.command, "math.add");
            assert_eq!(inner.args[0], Expr::Num(1));
            assert_eq!(inner.args[1], Expr::Num(2));
        }
        other => panic!("expected Call, got {other:?}"),
    }
    assert_eq!(p_cmds[0].call.args[1], Expr::Num(3));

    // --- Left ambiguity errors ---

    // `math.add math.add 1, 2, 3` — both have arity 2, left-most param is command
    // without parens => ambiguous
    let ambiguous = parser::parse_source("math.add math.add 1, 2, 3");
    assert!(ambiguous.is_err(), "left-ambiguous should produce an error");
    let err = ambiguous.unwrap_err();
    assert!(
        err.message.contains("ambiguous"),
        "error should mention 'ambiguous': {}",
        err.message
    );
    assert!(
        err.message.contains("_ or ()"),
        "error should suggest '_ or ()': {}",
        err.message
    );

    // --- Unknown arity commands parse as individual params ---

    let unknown = parser::parse_source("unknown.cmd a, b, c");
    assert!(unknown.is_ok(), "unknown arity should parse");
    let u_cmds = unknown.unwrap();
    assert_eq!(u_cmds[0].call.command, "unknown.cmd");
    assert_eq!(u_cmds[0].call.args.len(), 3);
}
