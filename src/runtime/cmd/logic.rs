//! Logic command dispatch — routes `logic.*` commands to handlers.
//! Handler implementations live in `logic_handlers.rs` for module size.

use super::context::ExecContext;
use super::dispatch::eval_expr;
use crate::i18n;
use crate::parser::types::Call;
use crate::runtime::env::Env;
use crate::runtime::error::RuntimeError;
use crate::runtime::value::Value;

#[path = "logic_handlers.rs"]
mod logic_handlers;
use logic_handlers::*;

pub(crate) fn dispatch_logic(
    call: &Call,
    env: &mut Env,
    ctx: &ExecContext,
    sub: &str,
) -> Result<Value, RuntimeError> {
    match sub {
        "un" => handle_un(call, env, ctx),
        "and" => handle_and(call, env, ctx),
        "or" => handle_or(call, env, ctx),
        "same" => handle_same(call, env, ctx),
        "not.same" => handle_not_same(call, env, ctx),
        "more" => handle_more(call, env, ctx),
        "less" => handle_less(call, env, ctx),
        "more.same" => handle_more_same(call, env, ctx),
        "less.same" => handle_less_same(call, env, ctx),
        "max" => handle_max(call, env, ctx),
        "min" => handle_min(call, env, ctx),
        "between" => handle_between(call, env, ctx),
        "clamp" => handle_clamp(call, env, ctx),
        "is.num" => handle_is_num(call, env, ctx),
        "is.str" => handle_is_str(call, env, ctx),
        "is.empty" => handle_is_empty(call, env, ctx),
        "if" | "else" | "while" => handle_cond(call, env, ctx),
        "for" => handle_for_cond(call, env, ctx),
        _ => Err(RuntimeError::new(
            ctx.line,
            &call.command,
            i18n::unknown_command(
                ctx.lang.get(),
                "logic",
                call.command.strip_prefix("logic.").unwrap_or(&call.command),
            ),
        )),
    }
}

fn handle_cond(call: &Call, env: &mut Env, ctx: &ExecContext) -> Result<Value, RuntimeError> {
    let val = eval_expr(
        call.args
            .first()
            .ok_or_else(|| RuntimeError::new(ctx.line, &call.command, "missing argument"))?,
        env,
        ctx,
    )?;
    Ok(Value::Num(if is_truthy(&val) { 1 } else { 0 }))
}

fn handle_for_cond(call: &Call, env: &mut Env, ctx: &ExecContext) -> Result<Value, RuntimeError> {
    let val = eval_expr(
        call.args
            .first()
            .ok_or_else(|| RuntimeError::new(ctx.line, &call.command, "missing argument"))?,
        env,
        ctx,
    )?;
    Ok(Value::Num(numeric_val(&val, ctx).unwrap_or_default()))
}

// ── Shared helpers (also used by logic_handlers.rs via super::*) ─────

pub(super) fn resolve_one(
    call: &Call,
    env: &mut Env,
    ctx: &ExecContext,
    idx: usize,
) -> Result<Value, RuntimeError> {
    let expr = call.args.get(idx).ok_or_else(|| {
        RuntimeError::new(
            ctx.line,
            &call.command,
            i18n::requires_args(ctx.lang.get(), idx + 1),
        )
    })?;
    eval_expr(expr, env, ctx)
}

pub(super) fn resolve_two(
    call: &Call,
    env: &mut Env,
    ctx: &ExecContext,
) -> Result<(Value, Value), RuntimeError> {
    Ok((
        resolve_one(call, env, ctx, 0)?,
        resolve_one(call, env, ctx, 1)?,
    ))
}

pub(super) fn resolve_three(
    call: &Call,
    env: &mut Env,
    ctx: &ExecContext,
) -> Result<(Value, Value, Value), RuntimeError> {
    Ok((
        resolve_one(call, env, ctx, 0)?,
        resolve_one(call, env, ctx, 1)?,
        resolve_one(call, env, ctx, 2)?,
    ))
}

pub(super) fn is_truthy(v: &Value) -> bool {
    match v {
        Value::Num(n) => *n != 0,
        Value::Expr(e) => !e.is_zero(),
        Value::Str(s) => !s.is_empty(),
        Value::List(items) => !items.is_empty(),
        Value::Dict(pairs) => !pairs.is_empty(),
        Value::Empty => false,
    }
}

pub(super) fn numeric_val(v: &Value, _ctx: &ExecContext) -> Option<i64> {
    match v {
        Value::Num(n) => Some(*n),
        Value::Expr(e) => {
            if e.is_zero() {
                Some(0)
            } else {
                let f = e.to_f64();
                if f.fract() == 0.0 && f.is_finite() {
                    Some(f as i64)
                } else {
                    None
                }
            }
        }
        _ => None,
    }
}

pub(super) fn compare_values(
    a: &Value,
    b: &Value,
    ctx: &ExecContext,
) -> Option<std::cmp::Ordering> {
    match (a, b) {
        (Value::Num(na), Value::Num(nb)) => Some(na.cmp(nb)),
        (Value::Expr(_), Value::Expr(_))
        | (Value::Expr(_), Value::Num(_))
        | (Value::Num(_), Value::Expr(_)) => {
            let (fa, fb) = (numeric_val(a, ctx)?, numeric_val(b, ctx)?);
            Some(fa.cmp(&fb))
        }
        _ => None,
    }
}
