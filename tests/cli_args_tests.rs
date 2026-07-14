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
//! Tests for the cli.* command family — script command-line argument access.
//!
//! Args are injected via run_script_with_lang_and_args (added in a later task).
//! Until that function exists, these tests use run_script_with_lang and expect
//! empty args (backward-compat behavior).

use dyyl::i18n::Lang;
use dyyl::runtime::execute::run_script_with_lang;
use dyyl::runtime::execute::run_script_with_lang_and_args;
use dyyl::runtime::Value;

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

#[test]
fn cli_has_returns_zero_when_no_args() {
    let src = "io.out cli.has(\"--help\")\n";
    let out = run_script_with_lang(src, false, Lang::En);
    assert_eq!(out.values.len(), 1);
    assert_eq!(out.values[0], Value::Num(0));
}

#[test]
fn cli_value_not_found_returns_empty() {
    let src = "io.out cli.value(\"--out\")\n";
    let out = run_script_with_lang(src, false, Lang::En);
    assert_eq!(out.values.len(), 1);
    assert_eq!(out.values[0], Value::Empty);
}

#[test]
fn cli_script_name_empty_when_not_injected() {
    let src = "io.out cli.script_name\n";
    let out = run_script_with_lang(src, false, Lang::En);
    assert_eq!(out.values.len(), 1);
    assert_eq!(out.values[0], Value::Str(String::new()));
}

// ── 注入 args 的测试(通过 run_script_with_lang_and_args)──────────

#[test]
fn injected_cli_args_returns_list() {
    let src = "io.out cli.args\n";
    let out = run_script_with_lang_and_args(
        src,
        false,
        Lang::En,
        vec!["--help".to_string(), "foo".to_string()],
        "a.dyyl".to_string(),
    );
    assert_eq!(
        out.values[0],
        Value::List(vec![
            Value::Str("--help".to_string()),
            Value::Str("foo".to_string()),
        ])
    );
}

#[test]
fn injected_cli_count() {
    let src = "io.out cli.count\n";
    let out = run_script_with_lang_and_args(
        src,
        false,
        Lang::En,
        vec!["a".to_string(), "b".to_string(), "c".to_string()],
        "x.dyyl".to_string(),
    );
    assert_eq!(out.values[0], Value::Num(3));
}

#[test]
fn injected_cli_get_normal() {
    let src = "io.out cli.get(0)\nio.out cli.get(2)\n";
    let out = run_script_with_lang_and_args(
        src,
        false,
        Lang::En,
        vec![
            "first".to_string(),
            "second".to_string(),
            "third".to_string(),
        ],
        "x.dyyl".to_string(),
    );
    assert_eq!(out.values[0], Value::Str("first".to_string()));
    assert_eq!(out.values[1], Value::Str("third".to_string()));
}

#[test]
fn injected_cli_has_exact_match() {
    let src = "io.out cli.has(\"--help\")\n";
    let out = run_script_with_lang_and_args(
        src,
        false,
        Lang::En,
        vec!["--help".to_string()],
        "x.dyyl".to_string(),
    );
    assert_eq!(out.values[0], Value::Num(1));
}

#[test]
fn injected_cli_has_equals_suffix() {
    let src = "io.out cli.has(\"--mode\")\nio.out cli.has(\"--mode=fast\")\n";
    let out = run_script_with_lang_and_args(
        src,
        false,
        Lang::En,
        vec!["--mode=fast".to_string()],
        "x.dyyl".to_string(),
    );
    assert_eq!(out.values[0], Value::Num(1));
    assert_eq!(out.values[1], Value::Num(1));
}

#[test]
fn injected_cli_has_no_prefix_match() {
    let src = "io.out cli.has(\"--h\")\nio.out cli.has(\"--help\")\n";
    let out = run_script_with_lang_and_args(
        src,
        false,
        Lang::En,
        vec!["--helper".to_string()],
        "x.dyyl".to_string(),
    );
    assert_eq!(out.values[0], Value::Num(0));
    assert_eq!(out.values[1], Value::Num(0));
}

#[test]
fn injected_cli_value_space_separated() {
    let src = "io.out cli.value(\"--out\")\n";
    let out = run_script_with_lang_and_args(
        src,
        false,
        Lang::En,
        vec!["--out".to_string(), "foo.txt".to_string()],
        "x.dyyl".to_string(),
    );
    assert_eq!(out.values[0], Value::Str("foo.txt".to_string()));
}

#[test]
fn injected_cli_value_equals_form() {
    let src = "io.out cli.value(\"--mode\")\n";
    let out = run_script_with_lang_and_args(
        src,
        false,
        Lang::En,
        vec!["--mode=fast".to_string()],
        "x.dyyl".to_string(),
    );
    assert_eq!(out.values[0], Value::Str("fast".to_string()));
}

#[test]
fn injected_cli_value_flag_no_value_returns_empty() {
    let src = "io.out cli.value(\"--out\")\n";
    let out = run_script_with_lang_and_args(
        src,
        false,
        Lang::En,
        vec!["--out".to_string(), "--verbose".to_string()],
        "x.dyyl".to_string(),
    );
    assert_eq!(out.values[0], Value::Empty);
}

#[test]
fn injected_cli_value_first_wins() {
    let src = "io.out cli.value(\"--out\")\n";
    let out = run_script_with_lang_and_args(
        src,
        false,
        Lang::En,
        vec![
            "--out".to_string(),
            "a".to_string(),
            "--out".to_string(),
            "b".to_string(),
        ],
        "x.dyyl".to_string(),
    );
    assert_eq!(out.values[0], Value::Str("a".to_string()));
}

#[test]
fn injected_cli_script_name_basename() {
    let src = "io.out cli.script_name\n";
    let out = run_script_with_lang_and_args(
        src,
        false,
        Lang::En,
        vec![],
        "/home/user/a.dyyl".to_string(),
    );
    assert_eq!(out.values[0], Value::Str("a.dyyl".to_string()));
}

#[test]
fn injected_cli_dashdash_passthrough() {
    let src = "io.out cli.count\n";
    let out = run_script_with_lang_and_args(
        src,
        false,
        Lang::En,
        vec!["--help".to_string(), "--".to_string(), "--foo".to_string()],
        "x.dyyl".to_string(),
    );
    assert_eq!(out.values[0], Value::Num(3));
}

use std::process::Command;

#[test]
fn binary_passes_args_after_filename() {
    // 写一个临时脚本,打印 cli.count
    let tmp = std::env::temp_dir().join("dyyl_cli_arg_test.dyyl");
    std::fs::write(&tmp, "io.out cli.count\n").unwrap();
    let output = Command::new("cargo")
        .args(["run", "--", tmp.to_str().unwrap(), "--help", "foo"])
        .output()
        .expect("spawn");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("2"),
        "expected count=2, stdout was: {stdout}"
    );
}

#[test]
fn binary_preserves_existing_flags_before_filename() {
    let tmp = std::env::temp_dir().join("dyyl_cli_lang_test.dyyl");
    std::fs::write(&tmp, "io.out cli.count\n").unwrap();
    let output = Command::new("cargo")
        .args([
            "run",
            "--",
            "--lang",
            "zh",
            tmp.to_str().unwrap(),
            "a",
            "b",
            "c",
        ])
        .output()
        .expect("spawn");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("3"),
        "expected count=3, stdout was: {stdout}"
    );
}
