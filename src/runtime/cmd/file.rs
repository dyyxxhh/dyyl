//! File command handlers — file.write, file.append, file.read.
//!
//! All file path commands require absolute paths.  Relative paths are
//! rejected with a RuntimeError that maps to the Str("") sentinel.

use std::fs;
use std::path::Path;

use super::context::ExecContext;
use super::dispatch::eval_expr;
use crate::i18n;
use crate::parser::types::Call;
use crate::runtime::env::Env;
use crate::runtime::error::RuntimeError;
use crate::runtime::value::Value;

/// Route a `file.*` call to the appropriate handler.
pub(crate) fn handle_file_command(
    call: &Call,
    env: &mut Env,
    ctx: &ExecContext,
) -> Result<Value, RuntimeError> {
    let sub = &call.command["file.".len()..];
    match sub {
        "write" => handle_file_write(call, env, ctx),
        "append" => handle_file_append(call, env, ctx),
        "read" => handle_file_read(call, env, ctx),
        _ => Err(RuntimeError::new(
            ctx.line,
            &call.command,
            i18n::unknown_command(ctx.lang.get(), "file", sub),
        )),
    }
}

// ── Helpers ────────────────────────────────────────────────────────

/// Resolve arg at `idx` to a string value.
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

fn require_absolute(path: &str, ctx: &ExecContext, cmd: &str) -> Result<(), RuntimeError> {
    if Path::new(path).is_absolute() {
        Ok(())
    } else {
        Err(RuntimeError::new(
            ctx.line,
            cmd,
            i18n::path_must_be_absolute(ctx.lang.get(), path),
        ))
    }
}

// ── Handlers ───────────────────────────────────────────────────────

/// Handle `file.write <path>, <content>` — overwrite file.
fn handle_file_write(call: &Call, env: &mut Env, ctx: &ExecContext) -> Result<Value, RuntimeError> {
    let path = resolve_str_arg(call, env, ctx, 0)?;
    require_absolute(&path, ctx, &call.command)?;
    let content = resolve_str_arg(call, env, ctx, 1)?;
    fs::write(&path, &content).map_err(|e| {
        RuntimeError::new(
            ctx.line,
            &call.command,
            i18n::failed_to_write(ctx.lang.get(), &path, &e),
        )
    })?;
    Ok(Value::Str(content))
}

/// Handle `file.append <path>, <content>` — append to file.
fn handle_file_append(
    call: &Call,
    env: &mut Env,
    ctx: &ExecContext,
) -> Result<Value, RuntimeError> {
    let path = resolve_str_arg(call, env, ctx, 0)?;
    require_absolute(&path, ctx, &call.command)?;
    let content = resolve_str_arg(call, env, ctx, 1)?;
    fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .and_then(|mut f| {
            use std::io::Write;
            f.write_all(content.as_bytes())
        })
        .map_err(|e| {
            RuntimeError::new(
                ctx.line,
                &call.command,
                i18n::failed_to_append(ctx.lang.get(), &path, &e),
            )
        })?;
    Ok(Value::Str(content))
}

/// Handle `file.read <path>` — read file contents as string.
fn handle_file_read(call: &Call, env: &mut Env, ctx: &ExecContext) -> Result<Value, RuntimeError> {
    let path = resolve_str_arg(call, env, ctx, 0)?;
    require_absolute(&path, ctx, &call.command)?;
    let content = fs::read_to_string(&path).map_err(|e| {
        RuntimeError::new(
            ctx.line,
            &call.command,
            i18n::failed_to_read(ctx.lang.get(), &path, &e),
        )
    })?;
    Ok(Value::Str(content))
}
