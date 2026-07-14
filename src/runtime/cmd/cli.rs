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
use super::dispatch::eval_expr;

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

/// `cli.get <idx>` — return arg at 0-based index, or Num(-1) if OOB/negative.
fn handle_cli_get(call: &Call, env: &mut Env, ctx: &ExecContext) -> Result<Value, RuntimeError> {
    if call.args.len() != 1 {
        return Err(RuntimeError::new(
            ctx.line,
            &call.command,
            i18n::requires_n_args(ctx.lang.get(), 1),
        ));
    }
    let val = eval_expr(&call.args[0], env, ctx)?;
    let idx = match val {
        Value::Num(n) => n,
        Value::Str(s) => match s.parse::<i64>() {
            Ok(n) => n,
            Err(_) => {
                return Err(RuntimeError::new(
                    ctx.line,
                    &call.command,
                    i18n::expected_numeric(ctx.lang.get(), &Value::Str(s)),
                ));
            }
        },
        other => {
            return Err(RuntimeError::new(
                ctx.line,
                &call.command,
                i18n::expected_numeric(ctx.lang.get(), &other),
            ));
        }
    };
    if idx < 0 {
        if ctx.debug {
            eprintln!(
                "line {}: {}: negative index {}",
                ctx.line,
                ctx.text,
                idx
            );
        }
        return Ok(Value::Num(-1));
    }
    let args = env.script_args();
    match args.get(idx as usize) {
        Some(s) => Ok(Value::Str(s.clone())),
        None => {
            if ctx.debug {
                eprintln!(
                    "line {}: {}: index {} out of bounds (len {})",
                    ctx.line,
                    ctx.text,
                    idx,
                    args.len()
                );
            }
            Ok(Value::Num(-1))
        }
    }
}

/// `cli.has <flag>` — return 1 if flag present (exact match, or `--flag=...`
/// form counts), else 0. No prefix matching.
fn handle_cli_has(call: &Call, env: &mut Env, ctx: &ExecContext) -> Result<Value, RuntimeError> {
    if call.args.len() != 1 {
        return Err(RuntimeError::new(
            ctx.line,
            &call.command,
            i18n::requires_n_args(ctx.lang.get(), 1),
        ));
    }
    let val = eval_expr(&call.args[0], env, ctx)?;
    let flag = match val {
        Value::Str(s) => s,
        Value::Num(n) => n.to_string(),
        Value::Expr(e) => e.to_string(),
        other => {
            return Err(RuntimeError::new(
                ctx.line,
                &call.command,
                i18n::expected_string(ctx.lang.get(), &other),
            ));
        }
    };
    let eq_prefix = format!("{flag}=");
    let found = env.script_args().iter().any(|arg| {
        // 精确匹配,或 --flag=value 形式(--flag 部分相等)
        arg == &flag || arg.strip_prefix(&eq_prefix).is_some()
    });
    Ok(Value::Num(if found { 1 } else { 0 }))
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
