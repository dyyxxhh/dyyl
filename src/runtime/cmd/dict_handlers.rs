use super::context::ExecContext;
use super::dispatch::eval_expr;
use super::helpers::{resolve_container, resolve_var_name};
use crate::i18n;
use crate::parser::types::Call;
use crate::runtime::env::Env;
use crate::runtime::error::RuntimeError;
use crate::runtime::value::Value;

pub(crate) fn handle_dict_create(
    call: &Call,
    env: &mut Env,
    ctx: &ExecContext,
) -> Result<Value, RuntimeError> {
    if call.args.is_empty() {
        return Err(RuntimeError::new(
            ctx.line,
            &call.command,
            i18n::cmd_requires_var(ctx.lang.get(), "dict.create"),
        ));
    }
    let name = resolve_var_name(&call.args[0], ctx)?;
    env.set(&name, Value::Dict(Vec::new()));
    Ok(Value::Empty)
}

pub(crate) fn handle_dict_set(
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
    let key = eval_expr(&call.args[1], env, ctx)?;
    let val = eval_expr(&call.args[2], env, ctx)?;
    let mut pairs = match env.get(&name) {
        Some(Value::Dict(p)) => p.clone(),
        _ => {
            return Err(RuntimeError::new(
                ctx.line,
                &call.command,
                i18n::first_arg_must_be(ctx.lang.get(), "dict"),
            ));
        }
    };
    pairs.retain(|(k, _)| *k != key);
    pairs.push((key, val));
    env.set(&name, Value::Dict(pairs));
    Ok(Value::Empty)
}

pub(crate) fn handle_dict_get(
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
    let dict_val = resolve_container(&call.args[0], env, ctx)?;
    let key_val = eval_expr(&call.args[1], env, ctx)?;
    match dict_val {
        Value::Dict(pairs) => {
            for (k, v) in &pairs {
                if *k == key_val {
                    return Ok(v.clone());
                }
            }
            if ctx.debug {
                eprintln!("line {}: {}", ctx.line, ctx.text);
                eprintln!(
                    "{}{}",
                    i18n::reason_prefix(ctx.lang.get()),
                    i18n::warn_dict_get_missing_key(ctx.lang.get())
                );
            }
            Ok(Value::Num(-1))
        }
        _ => Err(RuntimeError::new(
            ctx.line,
            &call.command,
            i18n::first_arg_must_be(ctx.lang.get(), "dict"),
        )),
    }
}

pub(crate) fn handle_dict_has(
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
    let dict_val = resolve_container(&call.args[0], env, ctx)?;
    let key_val = eval_expr(&call.args[1], env, ctx)?;
    match dict_val {
        Value::Dict(pairs) => {
            let found = pairs.iter().any(|(k, _)| *k == key_val);
            Ok(Value::Num(if found { 1 } else { 0 }))
        }
        _ => Err(RuntimeError::new(
            ctx.line,
            &call.command,
            i18n::first_arg_must_be(ctx.lang.get(), "dict"),
        )),
    }
}

pub(crate) fn handle_dict_del(
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
    let key = eval_expr(&call.args[1], env, ctx)?;
    match env.get(&name) {
        Some(Value::Dict(pairs)) => {
            let mut new_pairs = pairs.clone();
            new_pairs.retain(|(k, _)| *k != key);
            env.set(&name, Value::Dict(new_pairs));
            Ok(Value::Empty)
        }
        _ => Err(RuntimeError::new(
            ctx.line,
            &call.command,
            i18n::first_arg_must_be(ctx.lang.get(), "dict"),
        )),
    }
}

pub(crate) fn handle_dict_keys(
    call: &Call,
    env: &mut Env,
    ctx: &ExecContext,
) -> Result<Value, RuntimeError> {
    if call.args.is_empty() {
        return Err(RuntimeError::new(
            ctx.line,
            &call.command,
            i18n::cmd_requires_container(ctx.lang.get(), "dict.keys", "dict"),
        ));
    }
    let dict_val = resolve_container(&call.args[0], env, ctx)?;
    match dict_val {
        Value::Dict(pairs) => Ok(Value::List(pairs.iter().map(|(k, _)| k.clone()).collect())),
        _ => Err(RuntimeError::new(
            ctx.line,
            &call.command,
            i18n::argument_must_be(ctx.lang.get(), "dict"),
        )),
    }
}

pub(crate) fn handle_dict_vals(
    call: &Call,
    env: &mut Env,
    ctx: &ExecContext,
) -> Result<Value, RuntimeError> {
    if call.args.is_empty() {
        return Err(RuntimeError::new(
            ctx.line,
            &call.command,
            i18n::cmd_requires_container(ctx.lang.get(), "dict.vals", "dict"),
        ));
    }
    let dict_val = resolve_container(&call.args[0], env, ctx)?;
    match dict_val {
        Value::Dict(pairs) => Ok(Value::List(pairs.iter().map(|(_, v)| v.clone()).collect())),
        _ => Err(RuntimeError::new(
            ctx.line,
            &call.command,
            i18n::argument_must_be(ctx.lang.get(), "dict"),
        )),
    }
}

pub(crate) fn handle_dict_len(
    call: &Call,
    env: &mut Env,
    ctx: &ExecContext,
) -> Result<Value, RuntimeError> {
    if call.args.is_empty() {
        return Err(RuntimeError::new(
            ctx.line,
            &call.command,
            i18n::cmd_requires_container(ctx.lang.get(), "dict.len", "dict"),
        ));
    }
    let dict_val = resolve_container(&call.args[0], env, ctx)?;
    match dict_val {
        Value::Dict(pairs) => Ok(Value::Num(pairs.len() as i64)),
        _ => Err(RuntimeError::new(
            ctx.line,
            &call.command,
            i18n::argument_must_be(ctx.lang.get(), "dict"),
        )),
    }
}
