use dyyl::prepass::scan_placeholders;

#[test]
fn scan_finds_ai_auto_without_hint() {
    let content = "set $x, ai.auto\nio.out $x\n";
    let phs = scan_placeholders(content);
    assert_eq!(phs.len(), 1);
    assert_eq!(phs[0].line, 1);
    assert!(phs[0].hint.is_none());
    assert_eq!(phs[0].original_text, "ai.auto");
}

#[test]
fn scan_finds_ai_auto_with_quoted_hint() {
    let content = "set $port, ai.auto \"端口常用25565\"\n";
    let phs = scan_placeholders(content);
    assert_eq!(phs.len(), 1);
    assert_eq!(phs[0].hint.as_deref(), Some("端口常用25565"));
}

#[test]
fn scan_finds_ai_auto_with_bareword_hint() {
    let content = "set $x, ai.auto some_hint\n";
    let phs = scan_placeholders(content);
    assert_eq!(phs.len(), 1);
    assert_eq!(phs[0].hint.as_deref(), Some("some_hint"));
}

#[test]
fn scan_finds_multiple_placeholders() {
    let content = "set $a, ai.auto \"first\"\nset $b, ai.auto\nset $c, ai.auto \"third\"\n";
    let phs = scan_placeholders(content);
    assert_eq!(phs.len(), 3);
    assert_eq!(phs[0].id, 1);
    assert_eq!(phs[1].id, 2);
    assert_eq!(phs[2].id, 3);
}

#[test]
fn scan_ignores_ai_auto_filled() {
    let content = "set $x, ai.auto.filled _, \"value\"\nset $y, ai.auto\n";
    let phs = scan_placeholders(content);
    assert_eq!(phs.len(), 1, "ai.auto.filled should not be scanned");
    assert_eq!(phs[0].line, 2);
}

#[test]
fn scan_finds_inline_placeholder() {
    let content = "file.write ai.auto \"路径\", \"content\"\n";
    let phs = scan_placeholders(content);
    assert_eq!(phs.len(), 1);
    assert_eq!(phs[0].hint.as_deref(), Some("路径"));
}

#[test]
fn scan_returns_empty_when_no_placeholders() {
    let content = "io.out hello\nset $x, 42\n";
    let phs = scan_placeholders(content);
    assert!(phs.is_empty());
}

use dyyl::ai::prompt::FilledValue;
use dyyl::prepass::{reset_filled, rewrite_placeholders};
use std::collections::HashMap;

#[test]
fn rewrite_empty_hint_string_value() {
    let content = "set $x, ai.auto\n";
    let phs = dyyl::prepass::scan_placeholders(content);
    let mut values = HashMap::new();
    values.insert(
        "1".to_string(),
        FilledValue {
            value: "Steve".to_string(),
            is_number: false,
        },
    );
    let result = rewrite_placeholders(content, &phs, &values);
    assert_eq!(result, "set $x, ai.auto.filled _, \"Steve\"\n");
}

#[test]
fn rewrite_empty_hint_number_value() {
    let content = "set $x, ai.auto\n";
    let phs = dyyl::prepass::scan_placeholders(content);
    let mut values = HashMap::new();
    values.insert(
        "1".to_string(),
        FilledValue {
            value: "42".to_string(),
            is_number: true,
        },
    );
    let result = rewrite_placeholders(content, &phs, &values);
    assert_eq!(result, "set $x, ai.auto.filled _, 42\n");
}

#[test]
fn rewrite_hint_number_value() {
    let content = "set $port, ai.auto \"端口\"\n";
    let phs = dyyl::prepass::scan_placeholders(content);
    let mut values = HashMap::new();
    values.insert(
        "1".to_string(),
        FilledValue {
            value: "25565".to_string(),
            is_number: true,
        },
    );
    let result = rewrite_placeholders(content, &phs, &values);
    assert_eq!(result, "set $port, ai.auto.filled \"端口\", 25565\n");
}

#[test]
fn rewrite_escapes_special_chars_in_string() {
    let content = "set $x, ai.auto\n";
    let phs = dyyl::prepass::scan_placeholders(content);
    let mut values = HashMap::new();
    values.insert(
        "1".to_string(),
        FilledValue {
            value: "hello \"world\"\n".to_string(),
            is_number: false,
        },
    );
    let result = rewrite_placeholders(content, &phs, &values);
    assert_eq!(
        result,
        "set $x, ai.auto.filled _, \"hello \\\"world\\\"\\n\"\n"
    );
}

#[test]
fn rewrite_missing_value_keeps_original() {
    let content = "set $x, ai.auto \"hint\"\n";
    let phs = dyyl::prepass::scan_placeholders(content);
    let values = HashMap::new();
    let result = rewrite_placeholders(content, &phs, &values);
    assert_eq!(result, content, "missing value should keep original");
}

#[test]
fn reset_filled_strips_value_and_keeps_hint() {
    let content = "set $x, ai.auto.filled \"hint\", \"value\"\n";
    let result = reset_filled(content);
    assert_eq!(result, "set $x, ai.auto \"hint\"\n");
}

#[test]
fn reset_filled_empty_hint() {
    let content = "set $x, ai.auto.filled _, \"value\"\n";
    let result = reset_filled(content);
    assert_eq!(result, "set $x, ai.auto\n");
}

#[test]
fn reset_filled_number_value() {
    let content = "set $x, ai.auto.filled \"hint\", 42\n";
    let result = reset_filled(content);
    assert_eq!(result, "set $x, ai.auto \"hint\"\n");
}

#[test]
fn reset_filled_no_change_if_no_filled() {
    let content = "set $x, ai.auto \"hint\"\nio.out $x\n";
    let result = reset_filled(content);
    assert_eq!(result, content);
}

use dyyl::prepass::{run, build_only};
use std::fs;
use tempfile::tempdir;

#[test]
fn run_skips_when_no_placeholders() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("script.dyyl");
    fs::write(&path, "io.out hello\n").unwrap();
    run(&path, dyyl::i18n::Lang::En).expect("ok");
    assert_eq!(fs::read_to_string(&path).unwrap(), "io.out hello\n");
}

#[test]
fn build_only_no_change_when_no_ai_auto() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("script.dyyl");
    fs::write(&path, "io.out hello\n").unwrap();
    build_only(&path, dyyl::i18n::Lang::En).expect("ok");
    assert_eq!(fs::read_to_string(&path).unwrap(), "io.out hello\n");
}
