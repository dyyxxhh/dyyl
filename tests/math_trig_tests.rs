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
//! Trig and log/ln/lg/exp integration tests for Task 5.

use dyyl::runtime::execute::run_script;
use dyyl::runtime::Value;

const PI: &str = "\u{03C0}";
const ONE_HALF: &str = "\u{00BD}";

fn exec_one(source: &str) -> Value {
    let output = run_script(source, false);
    assert_eq!(output.values.len(), 1, "from {source:?}");
    output.values.into_iter().next().unwrap()
}

fn assert_display(source: &str, expected: &str) {
    assert_eq!(exec_one(source).to_string(), expected, "source: {source:?}");
}

// ── Trig: sin ──────────────────────────────────────────────────────────

#[test]
fn sin_0() {
    assert_display("math.sin 0", "0");
}
#[test]
fn sin_pi_6() {
    assert_display(&format!("math.sin math.div {PI}, 6"), ONE_HALF);
}

// ── Trig: cos ──────────────────────────────────────────────────────────

#[test]
fn cos_0() {
    assert_display("math.cos 0", "1");
}
#[test]
fn cos_pi_3() {
    assert_display(&format!("math.cos math.div {PI}, 3"), ONE_HALF);
}

// ── Trig: tan ──────────────────────────────────────────────────────────

#[test]
fn tan_0() {
    assert_display("math.tan 0", "0");
}
#[test]
fn tan_pi_4() {
    assert_display(&format!("math.tan math.div {PI}, 4"), "1");
}

// ── Trig: asin ─────────────────────────────────────────────────────────

#[test]
fn asin_0() {
    assert_display("math.asin 0", "0");
}
#[test]
fn asin_1() {
    assert_display("math.asin 1", "\u{00BD} \u{00D7} \u{03C0}");
}
// asin(1) = π/2 displayed as Prod(Rat(1,2), Const(Pi)) = "½ × π"

// ── Trig: acos ─────────────────────────────────────────────────────────

#[test]
fn acos_1() {
    assert_display("math.acos 1", "0");
}
#[test]
fn acos_0() {
    assert_display("math.acos 0", "\u{00BD} \u{00D7} \u{03C0}");
}
// acos(0) = π/2 displayed as "½ × π"

// ── Trig: atan ─────────────────────────────────────────────────────────

#[test]
fn atan_0() {
    assert_display("math.atan 0", "0");
}
#[test]
fn atan_1() {
    assert_display("math.atan 1", "\u{00BC} \u{00D7} \u{03C0}");
}
// atan(1) = π/4 displayed as Prod(Rat(1,4), Const(Pi)) = "¼ × π"

// ── Log: ln ────────────────────────────────────────────────────────────

#[test]
fn ln_e() {
    // ln(e) ≈ 1 (rounded to i64)
    let s = exec_one("math.ln math.e").to_string();
    assert_eq!(s, "1", "ln(e) should be ~1, got {s}");
}

// ── Log: lg ────────────────────────────────────────────────────────────

#[test]
fn lg_100() {
    // lg(100) = log10(100) = 2
    assert_display("math.lg 100", "2");
}

// ── Log: log with base ─────────────────────────────────────────────────

#[test]
fn log_8_2() {
    // log base 2 of 8 = 3
    assert_display("math.log 8, 2", "3");
}

// ── Exp ────────────────────────────────────────────────────────────────

#[test]
fn exp_0() {
    // e^0 = 1
    assert_display("math.exp 0", "1");
}
