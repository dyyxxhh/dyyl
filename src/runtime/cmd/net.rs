//! Net command handlers — net.get and net.download.
//!
//! Uses `ureq` for synchronous HTTPS requests.  A module-level agent is
//! lazily initialised with system trust anchors.  Tests may override it
//! via `configure_agent_for_testing` to trust a self-signed certificate.

use std::fs;
use std::io::Read as _;
use std::path::Path;
use std::sync::{Mutex, OnceLock};

use super::context::ExecContext;
use super::dispatch::eval_expr;
use crate::i18n;
use crate::parser::types::Call;
use crate::runtime::env::Env;
use crate::runtime::error::RuntimeError;
use crate::runtime::value::Value;

/// Module-level HTTPS agent — overridable for testing.
static AGENT: OnceLock<Mutex<Option<ureq::Agent>>> = OnceLock::new();

/// Configure the HTTPS agent for testing.  Only effective once per
/// process (first call wins).
pub fn configure_agent_for_testing(agent: ureq::Agent) {
    let mutex = AGENT.get_or_init(|| Mutex::new(None));
    if let Ok(mut guard) = mutex.lock() {
        *guard = Some(agent);
    }
}

fn get_agent() -> ureq::Agent {
    let mutex = AGENT.get_or_init(|| Mutex::new(None));
    let guard = match mutex.lock() {
        Ok(g) => g,
        Err(_) => return ureq::AgentBuilder::new().build(),
    };
    match guard.as_ref() {
        Some(a) => a.clone(),
        None => ureq::AgentBuilder::new().build(),
    }
}

/// Route a `net.*` call to the appropriate handler.
pub(crate) fn handle_net_command(
    call: &Call,
    env: &mut Env,
    ctx: &ExecContext,
) -> Result<Value, RuntimeError> {
    let sub = &call.command["net.".len()..];
    match sub {
        "get" => handle_net_get(call, env, ctx),
        "download" => handle_net_download(call, env, ctx),
        _ => Err(RuntimeError::new(
            ctx.line,
            &call.command,
            i18n::unknown_command(ctx.lang.get(), "net", sub),
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

/// Handle `net.get <url>` — return HTTPS response body as string.
fn handle_net_get(call: &Call, env: &mut Env, ctx: &ExecContext) -> Result<Value, RuntimeError> {
    let url = resolve_str_arg(call, env, ctx, 0)?;
    let agent = get_agent();
    let mut body = String::new();
    agent
        .get(&url)
        .call()
        .map_err(|e| {
            RuntimeError::new(
                ctx.line,
                &call.command,
                i18n::failed_to_fetch(ctx.lang.get(), &url, &e),
            )
        })?
        .into_reader()
        .read_to_string(&mut body)
        .map_err(|e| {
            RuntimeError::new(
                ctx.line,
                &call.command,
                i18n::failed_to_read_response(ctx.lang.get(), &url, &e),
            )
        })?;
    Ok(Value::Str(body))
}

/// Handle `net.download <url>, <dest>` — download to file, return byte count.
fn handle_net_download(
    call: &Call,
    env: &mut Env,
    ctx: &ExecContext,
) -> Result<Value, RuntimeError> {
    let url = resolve_str_arg(call, env, ctx, 0)?;
    let dest = resolve_str_arg(call, env, ctx, 1)?;
    require_absolute(&dest, ctx, &call.command)?;
    let agent = get_agent();
    let mut body = Vec::new();
    agent
        .get(&url)
        .call()
        .map_err(|e| {
            RuntimeError::new(
                ctx.line,
                &call.command,
                i18n::failed_to_fetch(ctx.lang.get(), &url, &e),
            )
        })?
        .into_reader()
        .read_to_end(&mut body)
        .map_err(|e| {
            RuntimeError::new(
                ctx.line,
                &call.command,
                i18n::failed_to_read_response(ctx.lang.get(), &url, &e),
            )
        })?;
    let byte_count = body.len() as i64;
    fs::write(&dest, &body).map_err(|e| {
        RuntimeError::new(
            ctx.line,
            &call.command,
            i18n::failed_to_write(ctx.lang.get(), &dest, &e),
        )
    })?;
    Ok(Value::Num(byte_count))
}
