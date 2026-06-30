//! Logic command handler implementations.
//! Helpers (resolve_one, is_truthy, etc.) are in parent logic.rs.

use super::{compare_values, is_truthy, numeric_val, resolve_one, resolve_three, resolve_two};
use crate::i18n;
use crate::parser::types::Call;
use crate::runtime::cmd::context::ExecContext;
use crate::runtime::env::Env;
use crate::runtime::error::RuntimeError;
use crate::runtime::value::Value;

pub(super) fn handle_un(
    call: &Call,
    env: &mut Env,
    ctx: &ExecContext,
) -> Result<Value, RuntimeError> {
    Ok(Value::Num(if is_truthy(&resolve_one(call, env, ctx, 0)?) {
        0
    } else {
        1
    }))
}

pub(super) fn handle_and(
    call: &Call,
    env: &mut Env,
    ctx: &ExecContext,
) -> Result<Value, RuntimeError> {
    let (a, b) = resolve_two(call, env, ctx)?;
    Ok(Value::Num(if is_truthy(&a) && is_truthy(&b) {
        1
    } else {
        0
    }))
}

pub(super) fn handle_or(
    call: &Call,
    env: &mut Env,
    ctx: &ExecContext,
) -> Result<Value, RuntimeError> {
    let (a, b) = resolve_two(call, env, ctx)?;
    Ok(Value::Num(if is_truthy(&a) || is_truthy(&b) {
        1
    } else {
        0
    }))
}

pub(super) fn handle_same(
    call: &Call,
    env: &mut Env,
    ctx: &ExecContext,
) -> Result<Value, RuntimeError> {
    let (a, b) = resolve_two(call, env, ctx)?;
    Ok(Value::Num(if a == b { 1 } else { 0 }))
}

pub(super) fn handle_not_same(
    call: &Call,
    env: &mut Env,
    ctx: &ExecContext,
) -> Result<Value, RuntimeError> {
    let (a, b) = resolve_two(call, env, ctx)?;
    Ok(Value::Num(if a != b { 1 } else { 0 }))
}

pub(super) fn handle_more(
    call: &Call,
    env: &mut Env,
    ctx: &ExecContext,
) -> Result<Value, RuntimeError> {
    let (a, b) = resolve_two(call, env, ctx)?;
    match compare_values(&a, &b, ctx) {
        Some(std::cmp::Ordering::Greater) => Ok(Value::Num(1)),
        _ => Ok(Value::Num(0)),
    }
}

pub(super) fn handle_less(
    call: &Call,
    env: &mut Env,
    ctx: &ExecContext,
) -> Result<Value, RuntimeError> {
    let (a, b) = resolve_two(call, env, ctx)?;
    match compare_values(&a, &b, ctx) {
        Some(std::cmp::Ordering::Less) => Ok(Value::Num(1)),
        _ => Ok(Value::Num(0)),
    }
}

pub(super) fn handle_more_same(
    call: &Call,
    env: &mut Env,
    ctx: &ExecContext,
) -> Result<Value, RuntimeError> {
    let (a, b) = resolve_two(call, env, ctx)?;
    match compare_values(&a, &b, ctx) {
        Some(std::cmp::Ordering::Less) => Ok(Value::Num(0)),
        _ => Ok(Value::Num(1)),
    }
}

pub(super) fn handle_less_same(
    call: &Call,
    env: &mut Env,
    ctx: &ExecContext,
) -> Result<Value, RuntimeError> {
    let (a, b) = resolve_two(call, env, ctx)?;
    match compare_values(&a, &b, ctx) {
        Some(std::cmp::Ordering::Greater) => Ok(Value::Num(0)),
        _ => Ok(Value::Num(1)),
    }
}

pub(super) fn handle_max(
    call: &Call,
    env: &mut Env,
    ctx: &ExecContext,
) -> Result<Value, RuntimeError> {
    let (a, b) = resolve_two(call, env, ctx)?;
    match compare_values(&a, &b, ctx) {
        Some(std::cmp::Ordering::Less) => Ok(b),
        _ => Ok(a),
    }
}

pub(super) fn handle_min(
    call: &Call,
    env: &mut Env,
    ctx: &ExecContext,
) -> Result<Value, RuntimeError> {
    let (a, b) = resolve_two(call, env, ctx)?;
    match compare_values(&a, &b, ctx) {
        Some(std::cmp::Ordering::Greater) => Ok(b),
        _ => Ok(a),
    }
}

pub(super) fn handle_between(
    call: &Call,
    env: &mut Env,
    ctx: &ExecContext,
) -> Result<Value, RuntimeError> {
    let (x, lo, hi) = resolve_three(call, env, ctx)?;
    let (xi, loi, hii) = (
        numeric_val(&x, ctx),
        numeric_val(&lo, ctx),
        numeric_val(&hi, ctx),
    );
    match (xi, loi, hii) {
        (Some(xv), Some(lv), Some(hv)) => Ok(Value::Num(if lv <= xv && xv <= hv { 1 } else { 0 })),
        _ => Ok(Value::Num(0)),
    }
}

pub(super) fn handle_clamp(
    call: &Call,
    env: &mut Env,
    ctx: &ExecContext,
) -> Result<Value, RuntimeError> {
    let (x, lo, hi) = resolve_three(call, env, ctx)?;
    let (xi, loi, hii) = (
        numeric_val(&x, ctx),
        numeric_val(&lo, ctx),
        numeric_val(&hi, ctx),
    );
    match (xi, loi, hii) {
        (Some(xv), Some(lv), Some(hv)) => {
            if xv < lv {
                Ok(Value::Num(lv))
            } else if xv > hv {
                Ok(Value::Num(hv))
            } else {
                Ok(Value::Num(xv))
            }
        }
        _ => Err(RuntimeError::new(
            ctx.line,
            &call.command,
            i18n::clamp_requires_three(ctx.lang.get()).to_string(),
        )),
    }
}

pub(super) fn handle_is_num(
    call: &Call,
    env: &mut Env,
    ctx: &ExecContext,
) -> Result<Value, RuntimeError> {
    match resolve_one(call, env, ctx, 0)? {
        Value::Num(_) | Value::Expr(_) => Ok(Value::Num(1)),
        _ => Ok(Value::Num(0)),
    }
}

pub(super) fn handle_is_str(
    call: &Call,
    env: &mut Env,
    ctx: &ExecContext,
) -> Result<Value, RuntimeError> {
    match resolve_one(call, env, ctx, 0)? {
        Value::Str(_) => Ok(Value::Num(1)),
        _ => Ok(Value::Num(0)),
    }
}

pub(super) fn handle_is_empty(
    call: &Call,
    env: &mut Env,
    ctx: &ExecContext,
) -> Result<Value, RuntimeError> {
    let v = resolve_one(call, env, ctx, 0)?;
    match v {
        Value::Num(0) => Ok(Value::Num(1)),
        Value::Expr(ref e) if e.is_zero() => Ok(Value::Num(1)),
        Value::Str(ref s) if s.is_empty() => Ok(Value::Num(1)),
        Value::List(ref items) if items.is_empty() => Ok(Value::Num(1)),
        Value::Dict(ref pairs) if pairs.is_empty() => Ok(Value::Num(1)),
        Value::Empty => Ok(Value::Num(1)),
        _ => Ok(Value::Num(0)),
    }
}
