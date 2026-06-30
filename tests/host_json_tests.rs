use std::sync::Arc;

use dyyl::runtime::execute::{run_script, run_script_with_lang_and_host};
use dyyl::runtime::host_provider::{McmArg, MockHostProvider};
use dyyl::runtime::value::Value;

fn run_with_host(source: &str, host: MockHostProvider) -> (Vec<Value>, Arc<MockHostProvider>) {
    let host_arc = Arc::new(host);
    let output =
        run_script_with_lang_and_host(source, false, dyyl::i18n::Lang::En, Some(host_arc.clone()));
    (output.values, host_arc)
}

#[test]
fn two_mcm_commands_roundtrip() {
    let host = MockHostProvider::with_responses(vec![
        MockHostProvider::ok_response("1", McmArg::Str("1.21.1".to_string())),
        MockHostProvider::ok_response("2", McmArg::Num(42)),
    ]);

    let source = "\
mcm.game.choose 1.21.1
mcm.mod.install mymod
";

    let (values, host) = run_with_host(source, host);
    assert_eq!(values.len(), 2, "expected 2 result values");
    assert_eq!(values[0], Value::Str("1.21.1".to_string()));
    assert_eq!(values[1], Value::Num(42));

    let sent = host.commands_sent();
    assert_eq!(sent.len(), 2);
    assert_eq!(sent[0].name, "mcm.game.choose");
    assert_eq!(sent[0].args, vec![McmArg::Str("1.21.1".to_string())]);
    assert_eq!(sent[1].name, "mcm.mod.install");
    assert_eq!(sent[1].args, vec![McmArg::Str("mymod".to_string())]);
}

#[test]
fn host_error_sentinel_propagates() {
    let host = MockHostProvider::with_responses(vec![MockHostProvider::error_response(
        "1",
        "unknown_command",
        "mcm.foo not supported",
    )]);

    let source = "mcm.foo arg1\n";

    let (values, _host) = run_with_host(source, host);
    assert_eq!(values.len(), 1);
    assert_eq!(values[0], Value::Num(-1));
}

#[test]
fn mixed_local_and_mcm_commands() {
    let host = MockHostProvider::with_responses(vec![MockHostProvider::ok_response(
        "1",
        McmArg::Str("1.21.1".to_string()),
    )]);

    let source = "\
create.num x
set $x, 10
mcm.game.choose 1.21.1
io.out $x
";

    let (values, host) = run_with_host(source, host);
    assert_eq!(values.len(), 4);
    assert_eq!(values[0], Value::Num(0));
    assert_eq!(values[1], Value::Num(10));
    assert_eq!(values[2], Value::Str("1.21.1".to_string()));
    assert_eq!(values[3], Value::Num(10));

    let sent = host.commands_sent();
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].name, "mcm.game.choose");
}

#[test]
fn game_choose_scope_persists_until_next_choose() {
    let host = MockHostProvider::with_responses(vec![
        MockHostProvider::ok_response("1", McmArg::Str("1.21.1".to_string())),
        MockHostProvider::ok_response("2", McmArg::Str("1.20.4".to_string())),
    ]);

    let source = "\
mcm.game.choose 1.21.1
mcm.game.choose 1.20.4
";

    let (_values, host) = run_with_host(source, host);
    let sent = host.commands_sent();
    assert_eq!(sent.len(), 2);
    assert_eq!(sent[0].name, "mcm.game.choose");
    assert_eq!(sent[1].name, "mcm.game.choose");
}

#[test]
fn unknown_command_still_errors_without_host() {
    let source = "unknown.cmd\n";
    let output = run_script(source, false);
    assert_eq!(output.values.len(), 1);
    assert_eq!(output.values[0], Value::Num(-1));
}

#[test]
fn mcm_command_without_host_errors() {
    let source = "mcm.game.choose 1.21.1\n";
    let output = run_script(source, false);
    assert_eq!(output.values.len(), 1);
    assert_eq!(output.values[0], Value::Num(-1));
}

#[test]
fn mcm_command_args_evaluation() {
    let host = MockHostProvider::with_responses(vec![MockHostProvider::ok_response(
        "1",
        McmArg::Str("ok".to_string()),
    )]);

    let source = "\
create.num v
set $v, 42
mcm.game.install $v
";

    let (_values, host) = run_with_host(source, host);
    let sent = host.commands_sent();
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].name, "mcm.game.install");
    assert_eq!(sent[0].args, vec![McmArg::Num(42)]);
}

#[test]
fn json_serialization_roundtrip() {
    let host = MockHostProvider::with_responses(vec![MockHostProvider::ok_response(
        "1",
        McmArg::Str("1.21.1".to_string()),
    )]);

    let source = "mcm.game.choose 1.21.1\n";
    let (_values, host) = run_with_host(source, host);

    let sent = host.commands_sent();
    assert_eq!(sent.len(), 1);

    let json = serde_json::to_string(&sent[0]).expect("serialize command");
    assert!(json.contains("\"type\":\"mcm_command\""));
    assert!(json.contains("\"name\":\"mcm.game.choose\""));

    let back: dyyl::McmCommand = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back, sent[0]);
}

#[test]
fn host_timeout_error_code() {
    let host = MockHostProvider::with_responses(vec![MockHostProvider::error_response(
        "1",
        "host_timeout",
        "command timed out after 60s",
    )]);

    let source = "mcm.game.install 1.21.1\n";
    let (values, _host) = run_with_host(source, host);
    assert_eq!(values.len(), 1);
    assert_eq!(values[0], Value::Num(-1));
}

#[test]
fn multiple_mcm_error_resilience() {
    let host = MockHostProvider::with_responses(vec![
        MockHostProvider::ok_response("1", McmArg::Str("ok".to_string())),
        MockHostProvider::error_response("2", "unknown_command", "fail"),
        MockHostProvider::ok_response("3", McmArg::Num(99)),
    ]);

    let source = "\
mcm.game.choose 1.21.1
mcm.unknown.cmd
mcm.game.version
";

    let (values, _host) = run_with_host(source, host);
    assert_eq!(values.len(), 3);
    assert_eq!(values[0], Value::Str("ok".to_string()));
    assert_eq!(values[1], Value::Num(-1));
    assert_eq!(values[2], Value::Num(99));
}
