use std::sync::Arc;

use super::*;
use crate::i18n::Lang;
use crate::runtime::cmd::context::ExecContext;
use crate::runtime::cmd::dispatch::dispatch_call;
use crate::runtime::io_provider::StdIoProvider;

fn default_provider() -> Arc<dyn IoProvider> {
    Arc::new(StdIoProvider)
}

fn parse_one(source: &str) -> ParsedCommand {
    let mut commands = crate::parser::parse_source(source).expect("parse should succeed");
    assert_eq!(commands.len(), 1, "expected exactly one command");
    commands.remove(0)
}

fn exec_one(source: &str, env: &mut Env) -> Result<Value, RuntimeError> {
    let cmd = parse_one(source);
    let ctx = ExecContext::from_command(&cmd, false, default_provider(), Lang::default());
    dispatch_call(&cmd.call, env, &ctx)
}

fn exec_script_get_values(source: &str) -> Vec<Value> {
    let commands = crate::parser::parse_source(source).expect("parse should succeed");
    run_commands_with_provider(&commands, false, &default_provider()).values
}

#[test]
fn dict_get_missing_returns_minus_one() {
    let mut env = Env::new();
    env.set("d", Value::Dict(Vec::new()));
    let val = exec_one("dict.get d, nonexistent", &mut env).expect("dict.get should succeed");
    assert_eq!(val, Value::Num(-1), "missing key → -1");
}

#[test]
fn dict_get_present_returns_value() {
    let mut env = Env::new();
    env.set(
        "d",
        Value::Dict(vec![(Value::Str("key".into()), Value::Num(42))]),
    );
    let val = exec_one("dict.get d, key", &mut env).expect("dict.get should succeed");
    assert_eq!(val, Value::Num(42), "present key → value");
}

#[test]
fn dict_get_with_dollar_prefix() {
    let mut env = Env::new();
    env.set("d", Value::Dict(Vec::new()));
    let val = exec_one("dict.get $d, missing", &mut env).expect("dict.get should succeed");
    assert_eq!(val, Value::Num(-1));
}

#[test]
fn list_get_oob_returns_minus_one() {
    let mut env = Env::new();
    env.set("l", Value::List(Vec::new()));
    let val = exec_one("list.get l, 0", &mut env).expect("list.get should succeed");
    assert_eq!(val, Value::Num(-1), "OOB → -1");
}

#[test]
fn list_get_valid_index() {
    let mut env = Env::new();
    env.set("l", Value::List(vec![Value::Num(10), Value::Num(20)]));
    let val = exec_one("list.get l, 1", &mut env).expect("list.get should succeed");
    assert_eq!(val, Value::Num(20), "index 1 → 20");
}

#[test]
fn list_get_negative_index_is_oob() {
    let mut env = Env::new();
    env.set("l", Value::List(vec![Value::Num(5)]));
    let val = exec_one("list.get l, -1", &mut env).expect("list.get should succeed");
    assert_eq!(val, Value::Num(-1), "negative index → -1");
}

#[test]
fn undefined_variable_returns_error() {
    let cmd = parse_one("set $x, $undefined");
    let ctx = ExecContext::from_command(&cmd, false, default_provider(), Lang::default());
    let result = dispatch_call(&cmd.call, &mut Env::new(), &ctx);
    assert!(result.is_err(), "undefined variable should error");
    assert!(result.unwrap_err().reason.contains("undefined"));
}

#[test]
fn unknown_command_returns_error() {
    let cmd = parse_one("completely_unknown 1, 2");
    let ctx = ExecContext::from_command(&cmd, false, default_provider(), Lang::default());
    let result = dispatch_call(&cmd.call, &mut Env::new(), &ctx);
    assert!(result.is_err(), "unknown command should error");
    assert!(result.unwrap_err().reason.contains("unknown"));
}

#[test]
fn set_binds_and_returns_value() {
    let mut env = Env::new();
    let val = exec_one("set $x, 42", &mut env).expect("set should succeed");
    assert_eq!(val, Value::Num(42));
    assert_eq!(env.get("x"), Some(&Value::Num(42)));
}

#[test]
fn set_rebinds_existing_var() {
    let mut env = Env::new();
    env.create_num("x");
    let val = exec_one("set $x, 99", &mut env).expect("set should succeed");
    assert_eq!(val, Value::Num(99));
    assert_eq!(env.get("x"), Some(&Value::Num(99)));
}

#[test]
fn create_num_binds_and_returns_zero() {
    let mut env = Env::new();
    let val = exec_one("create.num x", &mut env).expect("create.num should succeed");
    assert_eq!(val, Value::Num(0));
    assert_eq!(
        env.get("x"),
        Some(&Value::Num(0)),
        "create.num must bind in env"
    );
}

#[test]
fn create_str_binds_and_returns_empty() {
    let mut env = Env::new();
    let val = exec_one("create.str s", &mut env).expect("create.str should succeed");
    assert_eq!(val, Value::Str(String::new()));
    assert_eq!(
        env.get("s"),
        Some(&Value::Str(String::new())),
        "create.str must bind in env"
    );
}

#[test]
fn io_out_prints_and_returns_value() {
    let mut env = Env::new();
    let val = exec_one("io.out hello", &mut env).expect("io.out should succeed");
    assert_eq!(val, Value::Str("hello".to_string()));
}

#[test]
fn dict_create_creates_empty_dict() {
    let mut env = Env::new();
    let _ = exec_one("dict.create d", &mut env);
    assert_eq!(env.get("d"), Some(&Value::Dict(Vec::new())));
}

#[test]
fn list_create_creates_empty_list() {
    let mut env = Env::new();
    let _ = exec_one("list.create l", &mut env);
    assert_eq!(env.get("l"), Some(&Value::List(Vec::new())));
}

#[test]
fn script_with_dict_get_missing_produces_minus_one() {
    let values = exec_script_get_values("dict.create d\ndict.get d, nonexistent");
    assert_eq!(values[1], Value::Num(-1), "missing dict.get → -1");
}

#[test]
fn script_with_list_get_oob_produces_minus_one() {
    let values = exec_script_get_values("list.create l\nlist.get l, 0");
    assert_eq!(values[1], Value::Num(-1), "OOB list.get → -1");
}

#[test]
fn script_with_undefined_var_produces_error() {
    let source = "set $x, $undefined";
    let commands = crate::parser::parse_source(source).expect("parse");
    let output = run_commands_with_provider(&commands, true, &default_provider());
    assert_eq!(output.values[0], Value::Num(-1));
}

#[test]
fn script_with_unknown_command_produces_sentinel() {
    let source = "unknown_cmd";
    let commands = crate::parser::parse_source(source).expect("parse");
    let output = run_commands_with_provider(&commands, true, &default_provider());
    assert_eq!(output.values[0], Value::Num(-1));
}

#[test]
fn create_then_set_then_read_num_variable() {
    let values = exec_script_get_values("create.num x\nset $x, 42");
    assert_eq!(values.len(), 2);
    assert_eq!(values[0], Value::Num(0), "create.num x → 0");
    assert_eq!(
        values[1],
        Value::Num(42),
        "set $x, 42 → 42; if create didn't bind, set would fail"
    );
}

#[test]
fn create_then_set_then_read_str_variable() {
    let values = exec_script_get_values("create.str s\nset $s, hello");
    assert_eq!(values.len(), 2);
    assert_eq!(values[0], Value::Str(String::new()), "create.str s → ''");
    assert_eq!(
        values[1],
        Value::Str("hello".to_string()),
        "set $s, hello → 'hello'"
    );
}

#[test]
fn create_then_set_then_use_with_io_out() {
    let values = exec_script_get_values("create.num x\nset $x, 99\nio.out $x");
    assert_eq!(values.len(), 3);
    assert_eq!(values[2], Value::Num(99), "io.out $x → 99 after assign");
}

#[test]
fn io_out_with_undefined_var_produces_sentinel() {
    let source = "io.out $undefined_var";
    let commands = crate::parser::parse_source(source).expect("parse");
    let output = run_commands_with_provider(&commands, false, &default_provider());
    assert_eq!(output.values[0], Value::Num(-1));
}
