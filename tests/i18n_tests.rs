use dyyl::runtime::execute::run_script;
use dyyl::runtime::Value;

#[test]
fn language_no_args_returns_en() {
    let output = run_script("language", false);
    assert_eq!(output.values.len(), 1);
    assert_eq!(output.values[0], Value::Str("en".to_string()));
}

#[test]
fn language_zh_switches_to_chinese() {
    let source = "\
language zh
language
";
    let output = run_script(source, false);
    assert_eq!(output.values.len(), 2);
    assert_eq!(output.values[0], Value::Str("zh".to_string()));
    assert_eq!(output.values[1], Value::Str("zh".to_string()));
}

#[test]
fn language_en_switches_to_english() {
    let source = "\
language zh
language en
language
";
    let output = run_script(source, false);
    assert_eq!(output.values.len(), 3);
    assert_eq!(output.values[0], Value::Str("zh".to_string()));
    assert_eq!(output.values[1], Value::Str("en".to_string()));
    assert_eq!(output.values[2], Value::Str("en".to_string()));
}

#[test]
fn language_unknown_returns_error() {
    let output = run_script("language fr", false);
    assert_eq!(output.values.len(), 1);
    assert_eq!(output.values[0], Value::Num(-1));
}

#[test]
fn default_language_is_english() {
    let output = run_script("unknown.cmd", false);
    assert_eq!(output.values.len(), 1);
    assert_eq!(output.values[0], Value::Num(-1));
}

// ── MessageStore + t() tests (Task 2) ───────────────────────────────

use dyyl::i18n::{t, Lang};

#[test]
fn t_looks_up_en_message() {
    let msg = t(Lang::En, "runtime.division_by_zero", &[]);
    assert_eq!(msg, "division by zero");
}

#[test]
fn t_looks_up_zh_message() {
    let msg = t(Lang::Zh, "runtime.division_by_zero", &[]);
    assert_eq!(msg, "除以零");
}

#[test]
fn t_interpolates_single_arg() {
    let msg = t(Lang::En, "runtime.undefined_variable", &[("name", "foo")]);
    assert_eq!(msg, "undefined variable 'foo'");
}

#[test]
fn t_interpolates_multiple_args() {
    let msg = t(
        Lang::En,
        "plugin.updated",
        &[("name", "migpt"), ("old", "0.1.0"), ("new", "0.2.0")],
    );
    assert_eq!(msg, "updated migpt 0.1.0 -> 0.2.0");
}

#[test]
fn t_en_and_zh_differ_for_same_key() {
    let en = t(Lang::En, "plugin.sha256_mismatch", &[("name", "x")]);
    let zh = t(Lang::Zh, "plugin.sha256_mismatch", &[("name", "x")]);
    assert_ne!(en, zh);
}
