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
