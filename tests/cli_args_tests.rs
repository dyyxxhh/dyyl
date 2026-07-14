//! Tests for the cli.* command family — script command-line argument access.
//!
//! Args are injected via run_script_with_lang_and_args (added in a later task).
//! Until that function exists, these tests use run_script_with_lang and expect
//! empty args (backward-compat behavior).

use dyyl::runtime::execute::run_script_with_lang;
use dyyl::runtime::Value;
use dyyl::i18n::Lang;

#[test]
fn cli_args_empty_when_no_args() {
    let src = "io.out cli.args\n";
    let out = run_script_with_lang(src, false, Lang::En);
    // io.out pushes the value; cli.args with no injected args → empty list
    assert_eq!(out.values.len(), 1);
    assert_eq!(out.values[0], Value::List(vec![]));
}

#[test]
fn cli_count_zero_when_no_args() {
    let src = "io.out cli.count\n";
    let out = run_script_with_lang(src, false, Lang::En);
    assert_eq!(out.values.len(), 1);
    assert_eq!(out.values[0], Value::Num(0));
}

#[test]
fn cli_get_oob_returns_minus_one() {
    // 无 args 时任何下标都越界 → -1
    let src = "io.out cli.get(0)\n";
    let out = run_script_with_lang(src, false, Lang::En);
    assert_eq!(out.values.len(), 1);
    assert_eq!(out.values[0], Value::Num(-1));
}

#[test]
fn cli_get_negative_returns_minus_one() {
    let src = "io.out cli.get(-1)\n";
    let out = run_script_with_lang(src, false, Lang::En);
    assert_eq!(out.values.len(), 1);
    assert_eq!(out.values[0], Value::Num(-1));
}
