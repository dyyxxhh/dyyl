//! Variable command handlers — set, create.num, create.str.

use super::context::ExecContext;
use super::dispatch::eval_expr;
use super::helpers::resolve_var_name;
use crate::i18n;
use crate::parser::types::Call;
use crate::runtime::env::Env;
use crate::runtime::error::RuntimeError;
use crate::runtime::value::Value;

/// Handle `set $var, <value>` — bind or rebind a variable.
pub(crate) fn handle_set(
    call: &Call,
    env: &mut Env,
    ctx: &ExecContext,
) -> Result<Value, RuntimeError> {
    if call.args.len() < 2 {
        return Err(RuntimeError::new(
            ctx.line,
            &call.command,
            i18n::set_requires_two_args(ctx.lang.get()).to_string(),
        ));
    }
    let name = resolve_var_name(&call.args[0], ctx)?;
    let val = eval_expr(&call.args[1], env, ctx)?;
    env.set(&name, val.clone());
    Ok(val)
}

/// Handle `create.num <name>` — create a numeric variable (initial 0).
pub(crate) fn handle_create_num(
    call: &Call,
    env: &mut Env,
    ctx: &ExecContext,
) -> Result<Value, RuntimeError> {
    if call.args.is_empty() {
        return Err(RuntimeError::new(
            ctx.line,
            &call.command,
            i18n::cmd_requires_var(ctx.lang.get(), "create.num"),
        ));
    }
    let name = resolve_var_name(&call.args[0], ctx)?;
    env.create_num(&name);
    Ok(Value::Num(0))
}

/// Handle `create.str <name>` — create a string variable (initial "").
pub(crate) fn handle_create_str(
    call: &Call,
    env: &mut Env,
    ctx: &ExecContext,
) -> Result<Value, RuntimeError> {
    if call.args.is_empty() {
        return Err(RuntimeError::new(
            ctx.line,
            &call.command,
            i18n::cmd_requires_var(ctx.lang.get(), "create.str"),
        ));
    }
    let name = resolve_var_name(&call.args[0], ctx)?;
    env.create_str(&name);
    Ok(Value::Str(String::new()))
}
