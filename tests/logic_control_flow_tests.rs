//! Control flow tests for logic.if, logic.else, logic.while, logic.for,
//! nested blocks, underdeclared blocks, and the combined acceptance test.

use dyyl::runtime::execute::run_script;
use dyyl::runtime::Value;

fn exec_multi(source: &str) -> Vec<Value> {
    run_script(source, false).values
}

// ── Control flow: logic.if ────────────────────────────────────────────

#[test]
fn logic_if_true_executes_body() {
    let v = exec_multi(
        "\
create.num x
set $x, 0
logic.if 1, 1
  set $x, 42
io.out $x
",
    );
    assert_eq!(v[0], Value::Num(0));
    assert_eq!(v[1], Value::Num(0));
    assert_eq!(v[2], Value::Num(42), "body executes (set $x, 42)");
    assert_eq!(v[3], Value::Num(1), "if true returns 1");
    assert_eq!(v[4], Value::Num(42), "io.out $x after if body");
}

#[test]
fn logic_if_false_skips_body() {
    let v = exec_multi(
        "\
create.num x
set $x, 0
logic.if 0, 1
  set $x, 99
io.out $x
",
    );
    assert_eq!(v[0], Value::Num(0));
    assert_eq!(v[1], Value::Num(0));
    assert_eq!(v[2], Value::Num(0), "if false returns 0");
    assert_eq!(v[3], Value::Num(0), "x remains 0 (body skipped)");
}

// ── Control flow: logic.else ──────────────────────────────────────────

#[test]
fn logic_else_fires_when_if_false() {
    let v = exec_multi(
        "\
create.num x
set $x, 0
logic.if 0, 1
  set $x, 1
logic.else 1, 1
  set $x, 2
io.out $x
",
    );
    assert_eq!(v[0], Value::Num(0));
    assert_eq!(v[1], Value::Num(0));
    assert_eq!(v[2], Value::Num(0), "if false → 0");
    assert_eq!(v[3], Value::Num(2), "else body sets x to 2");
    assert_eq!(v[4], Value::Num(1), "else fired → 1");
    assert_eq!(v[5], Value::Num(2), "io.out $x = 2");
}

#[test]
fn logic_else_does_not_fire_when_if_true() {
    let v = exec_multi(
        "\
create.num x
set $x, 0
logic.if 1, 1
  set $x, 1
logic.else 1, 1
  set $x, 2
io.out $x
",
    );
    assert_eq!(v[0], Value::Num(0));
    assert_eq!(v[1], Value::Num(0));
    assert_eq!(v[2], Value::Num(1), "if body sets x to 1");
    assert_eq!(v[3], Value::Num(1), "if true → 1");
    assert_eq!(v[4], Value::Num(0), "else does not fire → 0");
    assert_eq!(v[5], Value::Num(1), "io.out $x = 1");
}

// ── Control flow: logic.while ─────────────────────────────────────────

#[test]
fn logic_while_counts_iterations() {
    let v = exec_multi(
        "\
create.num x
logic.while logic.less($x, 5), 1
  set $x, math.add($x, 1)
",
    );
    assert_eq!(v[0], Value::Num(0));
    assert_eq!(v[6], Value::Num(5), "while iterates 5 times");
}

#[test]
fn logic_while_zero_iterations() {
    let v = exec_multi(
        "\
create.num x
set $x, 10
logic.while logic.less($x, 5), 1
  set $x, math.add($x, 1)
",
    );
    assert_eq!(v[0], Value::Num(0));
    assert_eq!(v[1], Value::Num(10));
    assert_eq!(v[2], Value::Num(0), "while iterates 0 times");
}

// ── Control flow: logic.for ──────────────────────────────────────────

#[test]
fn logic_for_counts_iterations() {
    let v = exec_multi(
        "\
create.num x
logic.for 3, 1
  set $x, math.add($x, 1)
",
    );
    assert_eq!(v[0], Value::Num(0));
    assert_eq!(v[4], Value::Num(3), "for returns 3");
}

#[test]
fn logic_for_zero_iterations() {
    let v = exec_multi(
        "\
create.num x
logic.for 0, 1
  set $x, math.add($x, 1)
io.out $x
",
    );
    assert_eq!(v[0], Value::Num(0));
    assert_eq!(v[1], Value::Num(0), "for 0 returns 0");
    assert_eq!(v[2], Value::Num(0), "x still 0");
}

// ── Integration: logic with variables ──────────────────────────────────

#[test]
fn logic_un_with_var() {
    let v = exec_multi("create.num flag\nset $flag, 0\nlogic.un $flag");
    assert_eq!(v[2], Value::Num(1), "un of false should be true");
}

#[test]
fn logic_same_strings() {
    let v = exec_multi(
        "\
create.str s
set $s, hello
logic.same hello, $s
",
    );
    assert_eq!(v[2], Value::Num(1), "strings equal");
}

#[test]
fn logic_and_with_vars() {
    let v = exec_multi(
        "\
create.num a
create.num b
set $a, 1
set $b, 1
logic.and $a, $b
",
    );
    assert_eq!(v[4], Value::Num(1), "1 and 1 = 1");
}

#[test]
fn logic_between_with_vars() {
    let v = exec_multi(
        "\
create.num x
set $x, 5
logic.between $x, 1, 10
",
    );
    assert_eq!(v[2], Value::Num(1), "5 is between 1 and 10");
}

#[test]
fn logic_clamp_with_vars() {
    let v = exec_multi(
        "\
create.num x
set $x, 20
logic.clamp $x, 1, 10
",
    );
    assert_eq!(v[2], Value::Num(10), "clamp 20 to [1,10] = 10");
}

// ── Nested blocks ─────────────────────────────────────────────────────

#[test]
fn logic_nested_if_inside_while() {
    let v = exec_multi(
        "\
create.num x
create.num flag
set $flag, 0
logic.while logic.less($x, 3), 3
  logic.if logic.same($x, 1), 1
    set $flag, 1
  set $x, math.add($x, 1)
",
    );
    assert!(v.len() >= 8, "nested execution produces enough values");
}

// ── Underdeclared block ──────────────────────────────────────────────

#[test]
fn logic_underdeclared_block_does_not_panic() {
    let v = exec_multi(
        "\
logic.if 1, 1
  logic.if 1, 10
    io.out dcl
",
    );
    assert!(v.len() >= 1, "no panic from underdeclared block");
}

#[test]
fn logic_underdeclared_block_skips_leaked_body_lines() {
    let v = exec_multi(
        "\
create.num x
set $x, 1
logic.if $x, 1
  logic.if $x, 3
    io.out inner_line1
    io.out inner_line2
    io.out inner_line3
io.out done
",
    );
    let has_inner = |name: &str| v.iter().any(|val| *val == Value::Str(name.to_string()));
    assert!(
        !has_inner("inner_line1"),
        "inner_line1 must not leak from underdeclared block"
    );
    assert!(
        !has_inner("inner_line2"),
        "inner_line2 must not leak from underdeclared block"
    );
    assert!(
        !has_inner("inner_line3"),
        "inner_line3 must not leak from underdeclared block"
    );
    assert!(has_inner("done"), "top-level done must still execute");
}
