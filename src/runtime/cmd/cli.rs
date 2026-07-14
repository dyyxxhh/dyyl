//! CLI command handlers — cli.args, cli.count, cli.get, cli.has, cli.value,
//! cli.script_name.
//!
//! Provides read-only access to command-line arguments passed after the
//! script filename. Args are stored in Env (set by main.rs via execute.rs).

use crate::i18n;
use crate::parser::types::Call;
use crate::runtime::env::Env;
use crate::runtime::error::RuntimeError;
use crate::runtime::value::Value;

use super::context::ExecContext;

/// Route a `cli.*` call to the appropriate handler.
pub(crate) fn handle_cli_command(
    call: &Call,
    env: &mut Env,
    ctx: &ExecContext,
) -> Result<Value, RuntimeError> {
    let sub = &call.command["cli.".len()..];
    match sub {
        "args" => handle_cli_args(call, env, ctx),
        "count" => handle_cli_count(call, env, ctx),
        "get" => handle_cli_get(call, env, ctx),
        "has" => handle_cli_has(call, env, ctx),
        "value" => handle_cli_value(call, env, ctx),
        "script_name" => handle_cli_script_name(call, env, ctx),
        _ => Err(RuntimeError::new(
            ctx.line,
            &call.command,
            i18n::unknown_command(ctx.lang.get(), "cli", sub),
        )),
    }
}

// ── Handlers ───────────────────────────────────────────────────────

/// `cli.args` — return all args as a list of strings.
fn handle_cli_args(call: &Call, env: &mut Env, ctx: &ExecContext) -> Result<Value, RuntimeError> {
    if !call.args.is_empty() {
        return Err(RuntimeError::new(
            ctx.line,
            &call.command,
            i18n::requires_n_args(ctx.lang.get(), 0),
        ));
    }
    let items: Vec<Value> = env
        .script_args()
        .iter()
        .map(|s| Value::Str(s.clone()))
        .collect();
    Ok(Value::List(items))
}

/// `cli.count` — return the number of args.
fn handle_cli_count(call: &Call, env: &Env, ctx: &ExecContext) -> Result<Value, RuntimeError> {
    if !call.args.is_empty() {
        return Err(RuntimeError::new(
            ctx.line,
            &call.command,
            i18n::requires_n_args(ctx.lang.get(), 0),
        ));
    }
    Ok(Value::Num(env.script_args().len() as i64))
}

// Placeholder handlers for commands added in Tasks 3-5.
// These will be replaced with real implementations in subsequent tasks.
fn handle_cli_get(_call: &Call, _env: &mut Env, _ctx: &ExecContext) -> Result<Value, RuntimeError> {
    Ok(Value::Num(-1))
}

fn handle_cli_has(_call: &Call, _env: &mut Env, _ctx: &ExecContext) -> Result<Value, RuntimeError> {
    Ok(Value::Num(0))
}

fn handle_cli_value(_call: &Call, _env: &mut Env, _ctx: &ExecContext) -> Result<Value, RuntimeError> {
    Ok(Value::Empty)
}

fn handle_cli_script_name(
    _call: &Call,
    _env: &Env,
    _ctx: &ExecContext,
) -> Result<Value, RuntimeError> {
    Ok(Value::Str(String::new()))
}
