use std::sync::Arc;

use dyyl::runtime::execute::run_script_with_provider;
use dyyl::runtime::io_provider::{IoProvider, MockIoProvider};
use dyyl::runtime::Value;

fn mock(lines: Vec<&str>, keys: Vec<&str>, passwords: Vec<&str>) -> Arc<dyn IoProvider> {
    let m = MockIoProvider::with_lines(lines.into_iter().map(String::from).collect());
    for k in keys {
        m.push_key(k.to_string());
    }
    for p in passwords {
        m.push_password(p.to_string());
    }
    Arc::new(m)
}

#[test]
fn terminal_io_with_mock_input() {
    let provider = mock(vec!["hello from stdin"], vec!["Enter"], vec!["secret123"]);

    let source = "\
io.out 42
io.changeline
set $line, io.in
io.out $line
set $key, io.get
io.out $key
set $pwd, io.inpasswd
io.out $pwd
";

    let output = run_script_with_provider(source, false, Arc::clone(&provider));

    assert_eq!(output.values.len(), 8, "expected 8 result values");

    assert_eq!(output.values[0], Value::Num(42), "io.out 42 → Num(42)");
    assert_eq!(output.values[1], Value::Empty, "io.changeline → Empty");

    assert_eq!(
        output.values[2],
        Value::Str("hello from stdin".to_string()),
        "io.in returns mocked line"
    );
    assert_eq!(
        output.values[3],
        Value::Str("hello from stdin".to_string()),
        "io.out $line echoes the line"
    );

    assert_eq!(
        output.values[4],
        Value::Str("Enter".to_string()),
        "io.get returns mocked key"
    );
    assert_eq!(
        output.values[5],
        Value::Str("Enter".to_string()),
        "io.out $key echoes the key"
    );

    assert_eq!(
        output.values[6],
        Value::Str("secret123".to_string()),
        "io.inpasswd returns mocked password"
    );
    assert_eq!(
        output.values[7],
        Value::Str("secret123".to_string()),
        "io.out $pwd echoes the password"
    );
}

#[test]
fn terminal_io_no_input_returns_sentinel() {
    let provider = mock(vec![], vec![], vec![]);

    let source = "\
set $line, io.in
io.out $line
set $key, io.get
io.out $key
set $pwd, io.inpasswd
io.out $pwd
";

    let output = run_script_with_provider(source, false, Arc::clone(&provider));

    assert_eq!(output.values.len(), 6, "expected 6 result values");

    assert_eq!(
        output.values[0],
        Value::Str(String::new()),
        "io.in no-input → sentinel Str(\"\")"
    );
    assert_eq!(
        output.values[1],
        Value::Str(String::new()),
        "io.out $line of empty sentinel"
    );
    assert_eq!(
        output.values[2],
        Value::Str(String::new()),
        "io.get no-input → sentinel Str(\"\")"
    );
    assert_eq!(
        output.values[3],
        Value::Str(String::new()),
        "io.out $key of empty sentinel"
    );
    assert_eq!(
        output.values[4],
        Value::Str(String::new()),
        "io.inpasswd no-input → sentinel Str(\"\")"
    );
    assert_eq!(
        output.values[5],
        Value::Str(String::new()),
        "io.out $pwd of empty sentinel"
    );
}

#[test]
fn terminal_io_changeline_returns_empty() {
    let provider = mock(vec![], vec![], vec![]);

    let source = "\
io.changeline
io.changeline
";

    let output = run_script_with_provider(source, false, Arc::clone(&provider));
    assert_eq!(output.values.len(), 2);
    assert_eq!(output.values[0], Value::Empty, "first changeline");
    assert_eq!(output.values[1], Value::Empty, "second changeline");
}

#[test]
fn terminal_io_out_single_arg() {
    let provider = mock(vec![], vec![], vec![]);

    let source = "io.out 1/2";
    let output = run_script_with_provider(source, false, Arc::clone(&provider));

    assert_eq!(output.values.len(), 1);
    assert_eq!(
        output.values[0],
        Value::Expr(dyyl::math::CasNumber::reduce(1, 2)),
        "io.out with CAS fraction"
    );
}

#[test]
fn terminal_io_no_input_debug_warns() {
    let provider = mock(vec![], vec![], vec![]);
    let source = "io.in\n";
    let output = run_script_with_provider(source, true, Arc::clone(&provider));
    assert_eq!(output.values.len(), 1);
    assert_eq!(
        output.values[0],
        Value::Str(String::new()),
        "io.in no-input returns sentinel"
    );
}

#[test]
fn bare_zero_arity_io_in_in_greedy_rhs_evaluates_as_command() {
    let provider = mock(vec!["injected-value"], vec![], vec![]);
    let source = "io.out io.in";
    let output = run_script_with_provider(source, false, Arc::clone(&provider));

    assert_eq!(output.values.len(), 1, "one output value");
    assert_eq!(
        output.values[0],
        Value::Str("injected-value".to_string()),
        "io.out io.in must evaluate io.in as zero-arity command, not literal"
    );
}

#[test]
fn bare_zero_arity_io_in_via_set_then_io_out_evaluates() {
    let provider = mock(vec!["set-via-io-in"], vec![], vec![]);
    let source = "\
create.str a
set a,io.in
io.out $a";
    let output = run_script_with_provider(source, false, Arc::clone(&provider));

    assert_eq!(output.values.len(), 3, "create+set+out = 3 values");
    assert_eq!(
        output.values[2],
        Value::Str("set-via-io-in".to_string()),
        "set a,io.in then io.out $a must output the provider value"
    );
}

#[test]
fn quoted_io_in_is_literal_string_not_command() {
    let provider = mock(vec![], vec![], vec![]);
    let source = r#"io.out "io.in""#;
    let output = run_script_with_provider(source, false, Arc::clone(&provider));

    assert_eq!(output.values.len(), 1, "one output value");
    assert_eq!(
        output.values[0],
        Value::Str("io.in".to_string()),
        r#"io.out "io.in" must output literal string "io.in""#
    );
}
