use std::process::Command;

use super::context::ExecContext;
use super::dispatch::eval_expr;
use crate::i18n;
use crate::parser::types::Call;
use crate::runtime::env::Env;
use crate::runtime::error::RuntimeError;
use crate::runtime::value::Value;

pub(crate) fn handle_user_command(
    call: &Call,
    env: &mut Env,
    ctx: &ExecContext,
) -> Result<Value, RuntimeError> {
    let sub = &call.command["user.".len()..];
    match sub {
        "id" => handle_user_id(ctx),
        "name" => handle_user_name(ctx),
        "bash" => handle_user_bash(call, env, ctx),
        _ => Err(RuntimeError::new(
            ctx.line,
            &call.command,
            i18n::unknown_command(ctx.lang.get(), "user", sub),
        )),
    }
}

fn resolve_str_arg(
    call: &Call,
    env: &mut Env,
    ctx: &ExecContext,
    idx: usize,
) -> Result<String, RuntimeError> {
    let expr = call.args.get(idx).ok_or_else(|| {
        RuntimeError::new(
            ctx.line,
            &call.command,
            i18n::requires_args(ctx.lang.get(), idx + 1),
        )
    })?;
    let val = eval_expr(expr, env, ctx)?;
    match val {
        Value::Str(s) => Ok(s),
        Value::Num(n) => Ok(n.to_string()),
        Value::Expr(e) => Ok(e.to_string()),
        _ => Err(RuntimeError::new(
            ctx.line,
            &call.command,
            i18n::expected_string(ctx.lang.get(), &val),
        )),
    }
}

fn handle_user_id(ctx: &ExecContext) -> Result<Value, RuntimeError> {
    if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
        for line in status.lines() {
            if let Some(rest) = line.strip_prefix("Uid:") {
                let uid_str = rest.split_whitespace().next().unwrap_or_default();
                if !uid_str.is_empty() {
                    return Ok(Value::Str(uid_str.to_string()));
                }
            }
        }
    }
    match Command::new("id").arg("-u").output() {
        Ok(output) if output.status.success() => {
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            Ok(Value::Str(stdout))
        }
        _ => {
            if ctx.debug {
                eprintln!("line {}: {}", ctx.line, i18n::warn_user_id(ctx.lang.get()));
            }
            Ok(Value::sentinel_num())
        }
    }
}

fn handle_user_name(ctx: &ExecContext) -> Result<Value, RuntimeError> {
    if let Ok(name) = std::env::var("USER") {
        if !name.is_empty() {
            return Ok(Value::Str(name));
        }
    }
    match Command::new("whoami").output() {
        Ok(output) if output.status.success() => {
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            Ok(Value::Str(stdout))
        }
        _ => {
            if ctx.debug {
                eprintln!(
                    "line {}: {}",
                    ctx.line,
                    i18n::warn_user_name(ctx.lang.get())
                );
            }
            Ok(Value::sentinel_num())
        }
    }
}

fn handle_user_bash(call: &Call, env: &mut Env, ctx: &ExecContext) -> Result<Value, RuntimeError> {
    let cmd_str = resolve_str_arg(call, env, ctx, 0)?;
    let output = Command::new("sh").arg("-c").arg(&cmd_str).output();
    match output {
        Ok(out) => {
            if out.status.success() {
                let stdout = String::from_utf8_lossy(&out.stdout).to_string();
                Ok(Value::Str(stdout))
            } else {
                if ctx.debug {
                    eprintln!(
                        "line {}: {}",
                        ctx.line,
                        i18n::warn_user_bash_status(ctx.lang.get(), &cmd_str, &out.status)
                    );
                }
                Ok(Value::Num(-1))
            }
        }
        Err(e) => {
            if ctx.debug {
                eprintln!(
                    "line {}: {}",
                    ctx.line,
                    i18n::warn_user_bash_exec(ctx.lang.get(), &cmd_str, &e)
                );
            }
            Ok(Value::Num(-1))
        }
    }
}
