//! String command router — all `str.*` commands.
//!
//! Shared helpers and StrCtx bundle live here. Per-family handlers are
//! in `str_basic`, `str_modify`, `str_regex`, `str_convert`, `str_split_join`.

use super::context::ExecContext;
use super::dispatch::eval_expr;
use super::str_basic;
use super::str_convert;
use super::str_modify;
use super::str_regex;
use super::str_split_join;
use crate::i18n;
use crate::parser::types::Call;
use crate::runtime::env::Env;
use crate::runtime::error::RuntimeError;
use crate::runtime::value::Value;

/// Bundled context for str handlers (≤3 params).
pub(super) struct StrCtx<'a> {
    pub call: &'a Call,
    pub env: &'a mut Env,
    pub exec: &'a ExecContext,
}

pub(crate) fn handle_str_command(
    call: &Call,
    env: &mut Env,
    ctx: &ExecContext,
) -> Result<Value, RuntimeError> {
    let sub = &call.command["str.".len()..];
    let mut sc = StrCtx {
        call,
        env,
        exec: ctx,
    };
    match sub {
        // basic
        "len" | "get" | "slice" | "find" | "rfind" | "count" => {
            str_basic::dispatch_basic(sub, &mut sc)
        }
        "upper" | "lower" | "capital" | "reverse" | "repeat" => {
            str_basic::dispatch_basic(sub, &mut sc)
        }
        "start" | "end" | "contains" | "index" => str_basic::dispatch_basic(sub, &mut sc),

        // modify
        "replace" | "replace.all" | "insert" | "remove" => {
            str_modify::dispatch_modify(sub, &mut sc)
        }
        "pad.left" | "pad.right" | "trim" | "trim.left" | "trim.right" => {
            str_modify::dispatch_modify(sub, &mut sc)
        }

        // split/join
        "split" | "join" => str_split_join::dispatch_split_join(sub, &mut sc),

        // regex
        "match" | "extract" | "replace.regex" | "escape" | "unescape" => {
            str_regex::dispatch_regex(sub, &mut sc)
        }

        // convert
        "encode" | "decode" | "format" | "to.num" | "from.num" => {
            str_convert::dispatch_convert(sub, &mut sc)
        }

        _ => Err(RuntimeError::new(
            ctx.line,
            &call.command,
            i18n::unknown_command(
                ctx.lang.get(),
                "str",
                call.command.strip_prefix("str.").unwrap_or(&call.command),
            ),
        )),
    }
}

// ── Shared helpers ────────────────────────────────────────────────────

/// Resolve arg at `idx` to a string value.
pub(super) fn resolve_str_arg(sc: &mut StrCtx, idx: usize) -> Result<String, RuntimeError> {
    let expr = sc.call.args.get(idx).ok_or_else(|| {
        RuntimeError::new(
            sc.exec.line,
            &sc.call.command,
            i18n::requires_args(sc.exec.lang.get(), idx + 1),
        )
    })?;
    let val = eval_expr(expr, sc.env, sc.exec)?;
    match val {
        Value::Str(s) => Ok(s),
        Value::Num(n) => Ok(n.to_string()),
        Value::Expr(e) => Ok(e.to_string()),
        _ => Err(RuntimeError::new(
            sc.exec.line,
            &sc.call.command,
            i18n::expected_string(sc.exec.lang.get(), &val),
        )),
    }
}

/// Resolve arg at `idx` to a usize index (from numeric value).
pub(super) fn resolve_index_arg(sc: &mut StrCtx, idx: usize) -> Result<usize, RuntimeError> {
    let expr = sc.call.args.get(idx).ok_or_else(|| {
        RuntimeError::new(
            sc.exec.line,
            &sc.call.command,
            i18n::requires_args(sc.exec.lang.get(), idx + 1),
        )
    })?;
    let val = eval_expr(expr, sc.env, sc.exec)?;
    match val {
        Value::Num(n) => {
            if n < 0 {
                Err(RuntimeError::new(
                    sc.exec.line,
                    &sc.call.command,
                    i18n::index_must_be_nonnegative(sc.exec.lang.get(), n),
                ))
            } else {
                Ok(n as usize)
            }
        }
        _ => Err(RuntimeError::new(
            sc.exec.line,
            &sc.call.command,
            i18n::expected_numeric_index(sc.exec.lang.get(), &val),
        )),
    }
}

/// Resolve arg at `idx` to a Value.
pub(super) fn resolve_val(sc: &mut StrCtx, idx: usize) -> Result<Value, RuntimeError> {
    let expr = sc.call.args.get(idx).ok_or_else(|| {
        RuntimeError::new(
            sc.exec.line,
            &sc.call.command,
            i18n::requires_args(sc.exec.lang.get(), idx + 1),
        )
    })?;
    eval_expr(expr, sc.env, sc.exec)
}
