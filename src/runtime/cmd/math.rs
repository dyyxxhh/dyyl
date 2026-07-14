//! Math command handlers — all `math.*` commands.
//! All production helpers use `MathCtx` bundle to stay ≤3 params.

use super::context::ExecContext;
use super::dispatch::eval_expr;
use super::math_char::{char_code_add, char_code_sub};
use super::math_hash::hash_cmd;
use crate::i18n;
use crate::math;
use crate::math::ops;
use crate::math::{CasNumber, SymConstant};
use crate::parser::types::Call;
use crate::runtime::env::Env;
use crate::runtime::error::RuntimeError;
use crate::runtime::value::Value;

/// Bundled context for math handlers (reference, so functions take ≤3 params).
struct MathCtx<'a> {
    call: &'a Call,
    env: &'a mut Env,
    exec: &'a ExecContext,
}

// ── Op descriptors ───────────────────────────────────────────────────

struct UnaryOp(fn(&CasNumber) -> CasNumber);
struct BinaryOp(fn(&CasNumber, &CasNumber) -> CasNumber);
struct IntBinaryOp(fn(&CasNumber, &CasNumber) -> CasNumber);

pub(crate) fn handle_math_command(
    call: &Call,
    env: &mut Env,
    ctx: &ExecContext,
) -> Result<Value, RuntimeError> {
    let sub = &call.command["math.".len()..];
    let mut mc = MathCtx {
        call,
        env,
        exec: ctx,
    };
    match sub {
        "pi" => Ok(Value::Expr(CasNumber::Const(SymConstant::Pi))),
        "e" => Ok(Value::Expr(CasNumber::Const(SymConstant::E))),
        "tau" => Ok(Value::Expr(CasNumber::Const(SymConstant::Tau))),

        "sqrt" => do_unary(&UnaryOp(ops::sqrt), &mut mc),
        "abs" => do_unary(&UnaryOp(ops::abs), &mut mc),
        "round" => do_unary(&UnaryOp(ops::round), &mut mc),
        "floor" => do_unary(&UnaryOp(ops::floor), &mut mc),
        "ceil" => do_unary(&UnaryOp(ops::ceil), &mut mc),
        "sin" => do_unary(&UnaryOp(math::trig::sin), &mut mc),
        "cos" => do_unary(&UnaryOp(math::trig::cos), &mut mc),
        "tan" => do_unary(&UnaryOp(math::trig::tan), &mut mc),
        "asin" => do_unary(&UnaryOp(math::trig::asin), &mut mc),
        "acos" => do_unary(&UnaryOp(math::trig::acos), &mut mc),
        "atan" => do_unary(&UnaryOp(math::trig::atan), &mut mc),
        "ln" => do_unary(&UnaryOp(ops::ln), &mut mc),
        "lg" => do_unary(
            &UnaryOp(|x| CasNumber::Int(x.to_f64().log10() as i64)),
            &mut mc,
        ),
        "exp" => do_unary(
            &UnaryOp(|x| CasNumber::Int(x.to_f64().exp() as i64)),
            &mut mc,
        ),

        "multi" => do_binary(&BinaryOp(ops::mul), &mut mc),
        "div" => do_div(&mut mc),
        "strike" => do_int_binary(&IntBinaryOp(ops::strike), &mut mc),
        "surplus" => do_int_binary(&IntBinaryOp(ops::surplus), &mut mc),
        "pow" => do_binary(&BinaryOp(ops::pow), &mut mc),
        "log" => do_binary(&BinaryOp(log_with_base), &mut mc),
        "approx" => do_approx(&mut mc),

        "add" => do_mixed(&BinaryOp(ops::add), char_code_add, &mut mc),
        "sub" => do_mixed(&BinaryOp(ops::sub), char_code_sub, &mut mc),

        "hash" => hash_cmd(mc.call, mc.env, mc.exec),

        _ => Err(RuntimeError::new(
            ctx.line,
            &call.command,
            i18n::unknown_command(
                ctx.lang.get(),
                "math",
                call.command.strip_prefix("math.").unwrap_or(&call.command),
            ),
        )),
    }
}

// ── 2-param handlers (op, mc) ────────────────────────────────────────

fn do_unary(op: &UnaryOp, mc: &mut MathCtx) -> Result<Value, RuntimeError> {
    resolve_one(mc, 0).map(|v| Value::Expr((op.0)(&v)))
}

fn do_approx(mc: &mut MathCtx) -> Result<Value, RuntimeError> {
    resolve_one(mc, 0).map(|v| Value::Str(math::approx::format_15_sig_digits(&v)))
}

fn do_binary(op: &BinaryOp, mc: &mut MathCtx) -> Result<Value, RuntimeError> {
    let a = resolve_one(mc, 0)?;
    let b = resolve_one(mc, 1)?;
    Ok(Value::Expr((op.0)(&a, &b)))
}

fn do_div(mc: &mut MathCtx) -> Result<Value, RuntimeError> {
    let a = resolve_one(mc, 0)?;
    let b = resolve_one(mc, 1)?;
    if b.is_zero() {
        return Err(RuntimeError::new(
            mc.exec.line,
            &mc.call.command,
            i18n::division_by_zero(mc.exec.lang.get()).to_string(),
        ));
    }
    Ok(Value::Expr(ops::div(&a, &b)))
}

fn do_int_binary(op: &IntBinaryOp, mc: &mut MathCtx) -> Result<Value, RuntimeError> {
    let a = resolve_one(mc, 0)?;
    let b = resolve_one(mc, 1)?;
    if b.is_zero() {
        return Err(RuntimeError::new(
            mc.exec.line,
            &mc.call.command,
            i18n::division_by_zero(mc.exec.lang.get()).to_string(),
        ));
    }
    let ai = CasNumber::Int(a.to_f64() as i64);
    let bi = CasNumber::Int(b.to_f64() as i64);
    Ok(Value::Expr((op.0)(&ai, &bi)))
}

fn do_mixed(
    op: &BinaryOp,
    str_f: fn(&str, &CasNumber) -> Option<String>,
    mc: &mut MathCtx,
) -> Result<Value, RuntimeError> {
    if mc.call.args.len() < 2 {
        return Err(RuntimeError::new(
            mc.exec.line,
            &mc.call.command,
            i18n::requires_n_args(mc.exec.lang.get(), 2),
        ));
    }
    let a = eval_expr(
        mc.call
            .args
            .first()
            .ok_or_else(|| RuntimeError::new(mc.exec.line, &mc.call.command, "missing argument"))?,
        mc.env,
        mc.exec,
    )?;
    let b = eval_expr(
        mc.call
            .args
            .get(1)
            .ok_or_else(|| RuntimeError::new(mc.exec.line, &mc.call.command, "missing argument"))?,
        mc.env,
        mc.exec,
    )?;
    match (&a, &b) {
        (Value::Expr(ca), Value::Expr(cb)) => Ok(Value::Expr((op.0)(ca, cb))),
        (Value::Expr(ca), Value::Num(nb)) => Ok(Value::Expr((op.0)(ca, &CasNumber::Int(*nb)))),
        (Value::Num(na), Value::Expr(cb)) => Ok(Value::Expr((op.0)(&CasNumber::Int(*na), cb))),
        (Value::Num(na), Value::Num(nb)) => Ok(Value::Expr((op.0)(
            &CasNumber::Int(*na),
            &CasNumber::Int(*nb),
        ))),
        (Value::Str(sa), Value::Str(sb)) => {
            let mut r = sa.clone();
            r.push_str(sb);
            Ok(Value::Str(r))
        }
        (Value::Str(s), Value::Num(n)) => {
            if let Some(r) = str_f(s, &CasNumber::Int(*n)) {
                Ok(Value::Str(r))
            } else {
                debug_warn(mc, i18n::warn_invalid_char_offset(mc.exec.lang.get()));
                Ok(Value::sentinel_str())
            }
        }
        (Value::Num(n), Value::Str(s)) => {
            if let Some(r) = str_f(s, &CasNumber::Int(*n)) {
                Ok(Value::Str(r))
            } else {
                debug_warn(mc, i18n::warn_invalid_char_offset(mc.exec.lang.get()));
                Ok(Value::sentinel_str())
            }
        }
        _ => {
            debug_warn(mc, i18n::warn_mixed_types(mc.exec.lang.get()));
            Ok(Value::sentinel_num())
        }
    }
}

// ── Low-level helpers (≤3 params) ─────────────────────────────────────

fn resolve_one(mc: &mut MathCtx, idx: usize) -> Result<CasNumber, RuntimeError> {
    let expr = mc.call.args.get(idx).ok_or_else(|| {
        RuntimeError::new(
            mc.exec.line,
            &mc.call.command,
            i18n::requires_args(mc.exec.lang.get(), idx + 1),
        )
    })?;
    let val = eval_expr(expr, mc.env, mc.exec)?;
    cas_from_val(&val, mc.exec)
}

fn cas_from_val(val: &Value, ctx: &ExecContext) -> Result<CasNumber, RuntimeError> {
    match val {
        Value::Expr(c) => Ok(c.clone()),
        Value::Num(n) => Ok(CasNumber::Int(*n)),
        _ => Err(RuntimeError::new(
            ctx.line,
            &ctx.command,
            i18n::expected_numeric_any(ctx.lang.get(), val),
        )),
    }
}

fn log_with_base(a: &CasNumber, base: &CasNumber) -> CasNumber {
    if a.is_zero() || neg(a) || base.is_zero() || base.is_one() || neg(base) {
        return CasNumber::Int(-1);
    }
    CasNumber::Int((a.to_f64().ln() / base.to_f64().ln()) as i64)
}

fn neg(val: &CasNumber) -> bool {
    matches!(val, CasNumber::Int(n) if *n < 0) || matches!(val, CasNumber::Rat(n, _) if *n < 0)
}

fn debug_warn(mc: &mut MathCtx, msg: &str) {
    if mc.exec.debug {
        eprintln!("line {}: {}", mc.exec.line, mc.exec.text);
        eprintln!("{}{msg}", i18n::reason_prefix(mc.exec.lang.get()));
    }
}
