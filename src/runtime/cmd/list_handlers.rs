use super::context::ExecContext;
use super::dispatch::eval_expr;
use super::helpers::{resolve_container, resolve_var_name};
use crate::i18n;
use crate::parser::types::Call;
use crate::runtime::env::Env;
use crate::runtime::error::RuntimeError;
use crate::runtime::value::Value;

pub(crate) fn handle_list_create(
    call: &Call,
    env: &mut Env,
    ctx: &ExecContext,
) -> Result<Value, RuntimeError> {
    if call.args.is_empty() {
        return Err(RuntimeError::new(
            ctx.line,
            &call.command,
            i18n::cmd_requires_var(ctx.lang.get(), "list.create"),
        ));
    }
    let name = resolve_var_name(&call.args[0], ctx)?;
    env.set(&name, Value::List(Vec::new()));
    Ok(Value::Empty)
}

pub(crate) fn handle_list_get(
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
    let index_val = eval_expr(&call.args[1], env, ctx)?;
    match (list_val, index_val) {
        (Value::List(items), Value::Num(idx)) => {
            if idx < 0 || (idx as usize) >= items.len() {
                if ctx.debug {
                    eprintln!("line {}: {}", ctx.line, ctx.text);
                    eprintln!(
                        "{}{}",
                        i18n::reason_prefix(ctx.lang.get()),
                        i18n::warn_list_get_oob(ctx.lang.get(), idx, items.len())
                    );
                }
                Ok(Value::Num(-1))
            } else {
                Ok(items[idx as usize].clone())
            }
        }
        _ => Err(RuntimeError::new(
            ctx.line,
            &call.command,
            "list.get expects a list and a numeric index".to_string(),
        )),
    }
}

pub(crate) fn handle_list_len(
    call: &Call,
    env: &mut Env,
    ctx: &ExecContext,
) -> Result<Value, RuntimeError> {
    if call.args.is_empty() {
        return Err(RuntimeError::new(
            ctx.line,
            &call.command,
            i18n::cmd_requires_container(ctx.lang.get(), "list.len", "list"),
        ));
    }
    let list_val = resolve_container(&call.args[0], env, ctx)?;
    match list_val {
        Value::List(items) => Ok(Value::Num(items.len() as i64)),
        _ => Err(RuntimeError::new(
            ctx.line,
            &call.command,
            i18n::argument_must_be(ctx.lang.get(), "list"),
        )),
    }
}

pub(crate) fn handle_list_append(
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
    let name = resolve_var_name(&call.args[0], ctx)?;
    let val = eval_expr(&call.args[1], env, ctx)?;
    let current = env.get(&name).cloned().ok_or_else(|| {
        RuntimeError::new(
            ctx.line,
            &call.command,
            i18n::undefined_variable(ctx.lang.get(), &name),
        )
    })?;
    match current {
        Value::List(mut items) => {
            let len = items.len() as i64;
            items.push(val);
            env.set(&name, Value::List(items));
            Ok(Value::Num(len))
        }
        _ => Err(RuntimeError::new(
            ctx.line,
            &call.command,
            i18n::first_arg_must_be(ctx.lang.get(), "list"),
        )),
    }
}

pub(crate) fn handle_list_insert(
    call: &Call,
    env: &mut Env,
    ctx: &ExecContext,
) -> Result<Value, RuntimeError> {
    if call.args.len() < 3 {
        return Err(RuntimeError::new(
            ctx.line,
            &call.command,
            i18n::requires_n_args(ctx.lang.get(), 3),
        ));
    }
    let name = resolve_var_name(&call.args[0], ctx)?;
    let idx_val = eval_expr(&call.args[1], env, ctx)?;
    let val = eval_expr(&call.args[2], env, ctx)?;
    let idx = match idx_val {
        Value::Num(n) => n,
        _ => {
            return Err(RuntimeError::new(
                ctx.line,
                &call.command,
                i18n::index_must_be_numeric(ctx.lang.get()).to_string(),
            ));
        }
    };
    let current = env.get(&name).cloned().ok_or_else(|| {
        RuntimeError::new(
            ctx.line,
            &call.command,
            i18n::undefined_variable(ctx.lang.get(), &name),
        )
    })?;
    match current {
        Value::List(mut items) => {
            let pos = if idx < 0 {
                0
            } else if (idx as usize) > items.len() {
                items.len()
            } else {
                idx as usize
            };
            items.insert(pos, val);
            env.set(&name, Value::List(items));
            Ok(Value::Empty)
        }
        _ => Err(RuntimeError::new(
            ctx.line,
            &call.command,
            i18n::first_arg_must_be(ctx.lang.get(), "list"),
        )),
    }
}

pub(crate) fn handle_list_remove(
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
    let name = resolve_var_name(&call.args[0], ctx)?;
    let idx_val = eval_expr(&call.args[1], env, ctx)?;
    let idx = match idx_val {
        Value::Num(n) => n,
        _ => {
            return Err(RuntimeError::new(
                ctx.line,
                &call.command,
                i18n::index_must_be_numeric(ctx.lang.get()).to_string(),
            ));
        }
    };
    let current = env.get(&name).cloned().ok_or_else(|| {
        RuntimeError::new(
            ctx.line,
            &call.command,
            i18n::undefined_variable(ctx.lang.get(), &name),
        )
    })?;
    match current {
        Value::List(items) => {
            if idx < 0 || (idx as usize) >= items.len() {
                if ctx.debug {
                    eprintln!("line {}: {}", ctx.line, ctx.text);
                    eprintln!(
                        "{}{}",
                        i18n::reason_prefix(ctx.lang.get()),
                        i18n::warn_list_remove_oob(ctx.lang.get(), idx, items.len())
                    );
                }
                Ok(Value::Num(-1))
            } else {
                let mut new_items = items;
                let removed = new_items.remove(idx as usize);
                env.set(&name, Value::List(new_items));
                Ok(removed)
            }
        }
        _ => Err(RuntimeError::new(
            ctx.line,
            &call.command,
            i18n::first_arg_must_be(ctx.lang.get(), "list"),
        )),
    }
}
