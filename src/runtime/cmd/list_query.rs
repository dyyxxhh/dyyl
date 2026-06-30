use super::context::ExecContext;
use super::dispatch::eval_expr;
use super::helpers::resolve_container;
use crate::i18n;
use crate::parser::types::Call;
use crate::runtime::env::Env;
use crate::runtime::error::RuntimeError;
use crate::runtime::value::Value;

pub(crate) fn handle_list_contains(
    call: &Call,
    env: &mut Env,
    ctx: &ExecContext,
) -> Result<Value, RuntimeError> {
    if call.args.len() < 2 {
        return Err(RuntimeError::new(
            ctx.line,
            &call.command,
            i18n::requires_n_args(ctx.lang.get(), 2),
        ));
    }
    let list_val = resolve_container(&call.args[0], env, ctx)?;
    let needle = eval_expr(&call.args[1], env, ctx)?;
    match list_val {
        Value::List(items) => {
            let found = items.iter().any(|v| *v == needle);
            Ok(Value::Num(if found { 1 } else { 0 }))
        }
        _ => Err(RuntimeError::new(
            ctx.line,
            &call.command,
            i18n::first_arg_must_be(ctx.lang.get(), "list"),
        )),
    }
}

pub(crate) fn handle_list_index(
    call: &Call,
    env: &mut Env,
    ctx: &ExecContext,
) -> Result<Value, RuntimeError> {
    if call.args.len() < 2 {
        return Err(RuntimeError::new(
            ctx.line,
            &call.command,
            i18n::requires_n_args(ctx.lang.get(), 2),
        ));
    }
    let list_val = resolve_container(&call.args[0], env, ctx)?;
    let needle = eval_expr(&call.args[1], env, ctx)?;
    match list_val {
        Value::List(items) => {
            for (i, v) in items.iter().enumerate() {
                if *v == needle {
                    return Ok(Value::Num(i as i64));
                }
            }
            Ok(Value::Num(-1))
        }
        _ => Err(RuntimeError::new(
            ctx.line,
            &call.command,
            i18n::first_arg_must_be(ctx.lang.get(), "list"),
        )),
    }
}
