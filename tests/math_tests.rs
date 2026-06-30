//! Basic math integration tests: arithmetic, constants, rounding, hash, char-code.

use dyyl::runtime::execute::run_script;
use dyyl::runtime::Value;

const SQRT: &str = "\u{221A}";
const ONE_HALF: &str = "\u{00BD}";
const ONE_THIRD: &str = "\u{2153}";
const TWO_THIRDS: &str = "\u{2154}";

fn exec_one(source: &str) -> Value {
    let output = run_script(source, false);
    assert_eq!(output.values.len(), 1, "from {source:?}");
    output.values.into_iter().next().unwrap()
}

fn assert_display(source: &str, expected: &str) {
    assert_eq!(exec_one(source).to_string(), expected, "source: {source:?}");
}

// ── Rational arithmetic ────────────────────────────────────────────────

#[test]
fn one_third_times_three() {
    assert_display("math.multi 1/3, 3", "1");
}
#[test]
fn five_thirds_display() {
    assert_display("io.out 5/3", &format!("1{TWO_THIRDS}"));
}
#[test]
fn one_third_plus_one_sixth() {
    assert_display("math.add 1/3, 1/6", ONE_HALF);
}
#[test]
fn two_thirds_times_three_fourths() {
    assert_display("math.multi 2/3, 3/4", ONE_HALF);
}

// ── Sqrt ───────────────────────────────────────────────────────────────

#[test]
fn sqrt_2() {
    assert_display(&format!("io.out {SQRT}2"), &format!("{SQRT}2"));
}
#[test]
fn sqrt_2_div_2() {
    assert_display(&format!("math.div {SQRT}2, 2"), &format!("({SQRT}2)/2"));
}

// ── Basic math commands ────────────────────────────────────────────────

#[test]
fn add_int() {
    assert_display("math.add 3, 4", "7");
}
#[test]
fn sub_int() {
    assert_display("math.sub 10, 3", "7");
}
#[test]
fn multi_int() {
    assert_display("math.multi 6, 7", "42");
}
#[test]
fn div_int() {
    assert_display("math.div 10, 3", &format!("3{ONE_THIRD}"));
}
#[test]
fn strike_pos() {
    assert_display("math.strike 7, 2", "3");
}
#[test]
fn strike_neg() {
    assert_display("math.strike -7, 2", "-3");
}
#[test]
fn surplus_pos() {
    assert_display("math.surplus 7, 3", "1");
}
#[test]
fn surplus_neg() {
    assert_display("math.surplus -7, 2", "-1");
}
#[test]
fn pow_2_3() {
    assert_display("math.pow 2, 3", "8");
}
#[test]
fn pow_2_half() {
    assert_display("math.pow 2, 1/2", &format!("{SQRT}2"));
}
#[test]
fn pow_2_neg_1() {
    assert_display("math.pow 2, -1", ONE_HALF);
}
#[test]
fn sqrt_9() {
    assert_display("math.sqrt 9", "3");
}
#[test]
fn sqrt_2_cmd() {
    assert_display("math.sqrt 2", &format!("{SQRT}2"));
}
#[test]
fn abs_pos() {
    assert_display("math.abs 5", "5");
}
#[test]
fn abs_neg() {
    assert_display("math.abs -5", "5");
}

// ── Round/floor/ceil ───────────────────────────────────────────────────

#[test]
fn round_half() {
    assert_display("math.round 1/2", "1");
}
#[test]
fn round_neg_half() {
    assert_display("math.round -1/2", "-1");
}
#[test]
fn floor_pos() {
    assert_display("math.floor 7/3", "2");
}
#[test]
fn floor_neg() {
    assert_display("math.floor -7/3", "-3");
}
#[test]
fn ceil_pos() {
    assert_display("math.ceil 7/3", "3");
}
#[test]
fn ceil_neg() {
    assert_display("math.ceil -7/3", "-2");
}

// ── Constants commands ─────────────────────────────────────────────────

#[test]
fn pi_cmd() {
    assert_display("math.pi", "\u{03C0}");
}
#[test]
fn e_cmd() {
    assert_display("math.e", "e");
}
#[test]
fn tau_cmd() {
    assert_display("math.tau", "\u{03C4}");
}

// ── math.approx ────────────────────────────────────────────────────────

#[test]
fn approx_pi() {
    let s = exec_one(&format!("math.approx \u{03C0}")).to_string();
    assert_eq!(s, "3.14159265358979", "got: {s}");
    assert_eq!(s.chars().filter(|&c| c != '.' && c != '-').count(), 15);
}
#[test]
fn approx_int() {
    assert_eq!(exec_one("math.approx 42").to_string(), "42");
}

// ── math.hash ──────────────────────────────────────────────────────────

#[test]
fn hash_md5() {
    assert_display("math.hash hello, md5", "5d41402abc4b2a76b9719d911017c592");
}
#[test]
fn hash_sha256_default() {
    assert_eq!(
        exec_one("math.hash hello").to_string(),
        "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
    );
}

// ── Char-code arithmetic ───────────────────────────────────────────────

#[test]
fn char_add_a_1() {
    assert_display("math.add \"a\", 1", "b");
}
#[test]
fn char_add_1_a() {
    assert_display("math.add 1, \"a\"", "b");
}
#[test]
fn char_sub_b_1() {
    assert_display("math.sub \"b\", 1", "a");
}
#[test]
fn char_multi_sentinel() {
    assert_display("math.add \"hello\", 1", "");
}
#[test]
fn char_nonint_sentinel() {
    assert_display("math.add \"a\", 1/2", "-1");
}

// ── String concat + mixed sentinel ─────────────────────────────────────

#[test]
fn str_concat() {
    assert_display("math.add \"hello\", \"world\"", "helloworld");
}
#[test]
fn mixed_sentinel() {
    assert_display("math.add \u{03C0}, \"hello\"", "-1");
}
