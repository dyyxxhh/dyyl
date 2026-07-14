//! math.hash command handler.

use super::context::ExecContext;
use super::dispatch::eval_expr;
use crate::math;
use crate::parser::types::Call;
use crate::runtime::env::Env;
use crate::runtime::error::RuntimeError;
use crate::runtime::value::Value;

pub(crate) fn hash_cmd(
    call: &Call,
    env: &mut Env,
    ctx: &ExecContext,
) -> Result<Value, RuntimeError> {
    if call.args.is_empty() {
        return Err(RuntimeError::new(
            ctx.line,
            &call.command,
            "math.hash requires at least 1 argument",
        ));
    }
    let val = eval_expr(
        call.args
            .first()
            .ok_or_else(|| RuntimeError::new(ctx.line, &call.command, "missing argument"))?,
        env,
        ctx,
    )?;
    let algo = if call.args.len() > 1 {
        let algo_expr = eval_expr(
            call.args
                .get(1)
                .ok_or_else(|| RuntimeError::new(ctx.line, &call.command, "missing argument"))?,
            env,
            ctx,
        )?;
        algo_to_string(&algo_expr)
    } else {
        "sha256".to_string()
    };
    let algo = math::hash::parse_algo(&algo).ok_or_else(|| {
        RuntimeError::new(
            ctx.line,
            &call.command,
            format!("unknown hash algorithm '{algo}'"),
        )
    })?;
    let input = value_to_hash_string(&val);
    let result = math::hash::hash_value(algo, &input);
    Ok(Value::Str(result))
}

fn algo_to_string(val: &Value) -> String {
    match val {
        Value::Str(s) => s.clone(),
        Value::Expr(c) => c.to_string(),
        Value::Num(n) => n.to_string(),
        _ => "sha256".to_string(),
    }
}

fn value_to_hash_string(val: &Value) -> String {
    match val {
        Value::Str(s) => s.clone(),
        Value::Expr(c) => c.to_string(),
        Value::Num(n) => n.to_string(),
        _ => String::new(),
    }
}
