use super::context::ExecContext;
use super::dispatch::eval_expr;
use super::helpers::resolve_container;
use super::helpers::resolve_var_name;
use crate::i18n;
use crate::parser::types::Call;
use crate::runtime::env::Env;
use crate::runtime::error::RuntimeError;
use crate::runtime::value::Value;

pub(crate) fn handle_list_join(
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
    let list_val = resolve_container(
        call.args
            .first()
            .ok_or_else(|| RuntimeError::new(ctx.line, &call.command, "missing argument"))?,
        env,
        ctx,
    )?;
    let sep_val = eval_expr(
        call.args
            .get(1)
            .ok_or_else(|| RuntimeError::new(ctx.line, &call.command, "missing argument"))?,
        env,
        ctx,
    )?;
    let sep = match sep_val {
        Value::Str(s) => s,
        Value::Num(n) => n.to_string(),
        Value::Expr(e) => e.to_string(),
        _ => {
            return Err(RuntimeError::new(
                ctx.line,
                &call.command,
                i18n::separator_must_be_str_or_num(ctx.lang.get()).to_string(),
            ));
        }
    };
    match list_val {
        Value::List(items) => {
            let parts: Vec<String> = items.iter().map(Value::to_string).collect();
            Ok(Value::Str(parts.join(&sep)))
        }
        _ => Err(RuntimeError::new(
            ctx.line,
            &call.command,
            i18n::first_arg_must_be(ctx.lang.get(), "list"),
        )),
    }
}

pub(crate) fn handle_list_reverse(
    call: &Call,
    env: &mut Env,
    ctx: &ExecContext,
) -> Result<Value, RuntimeError> {
    if call.args.is_empty() {
        return Err(RuntimeError::new(
            ctx.line,
            &call.command,
            i18n::cmd_requires_container(ctx.lang.get(), "list.reverse", "list"),
        ));
    }
    let name = resolve_var_name(
        call.args
            .first()
            .ok_or_else(|| RuntimeError::new(ctx.line, &call.command, "missing argument"))?,
        ctx,
    )?;
    let current = env.get(&name).cloned().ok_or_else(|| {
        RuntimeError::new(
            ctx.line,
            &call.command,
            i18n::undefined_variable(ctx.lang.get(), &name),
        )
    })?;
    match current {
        Value::List(mut items) => {
            items.reverse();
            env.set(&name, Value::List(items));
            Ok(Value::Empty)
        }
        _ => Err(RuntimeError::new(
            ctx.line,
            &call.command,
            i18n::argument_must_be(ctx.lang.get(), "list"),
        )),
    }
}

pub(crate) fn handle_list_sort(
    call: &Call,
    env: &mut Env,
    ctx: &ExecContext,
) -> Result<Value, RuntimeError> {
    if call.args.is_empty() {
        return Err(RuntimeError::new(
            ctx.line,
            &call.command,
            i18n::cmd_requires_container(ctx.lang.get(), "list.sort", "list"),
        ));
    }
    let name = resolve_var_name(
        call.args
            .first()
            .ok_or_else(|| RuntimeError::new(ctx.line, &call.command, "missing argument"))?,
        ctx,
    )?;
    let current = env.get(&name).cloned().ok_or_else(|| {
        RuntimeError::new(
            ctx.line,
            &call.command,
            i18n::undefined_variable(ctx.lang.get(), &name),
        )
    })?;
    match current {
        Value::List(items) => {
            let sorted = sort_value_list(items);
            env.set(&name, Value::List(sorted));
            Ok(Value::Empty)
        }
        _ => Err(RuntimeError::new(
            ctx.line,
            &call.command,
            i18n::argument_must_be(ctx.lang.get(), "list"),
        )),
    }
}

pub(crate) fn handle_list_slice(
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
    let list_val = resolve_container(
        call.args
            .first()
            .ok_or_else(|| RuntimeError::new(ctx.line, &call.command, "missing argument"))?,
        env,
        ctx,
    )?;
    let start_val = eval_expr(
        call.args
            .get(1)
            .ok_or_else(|| RuntimeError::new(ctx.line, &call.command, "missing argument"))?,
        env,
        ctx,
    )?;
    let end_val = eval_expr(
        call.args
            .get(2)
            .ok_or_else(|| RuntimeError::new(ctx.line, &call.command, "missing argument"))?,
        env,
        ctx,
    )?;
    let start = match start_val {
        Value::Num(n) => n,
        _ => {
            return Err(RuntimeError::new(
                ctx.line,
                &call.command,
                i18n::index_must_be_numeric(ctx.lang.get()).to_string(),
            ));
        }
    };
    let end = match end_val {
        Value::Num(n) => n,
        _ => {
            return Err(RuntimeError::new(
                ctx.line,
                &call.command,
                i18n::index_must_be_numeric(ctx.lang.get()).to_string(),
            ));
        }
    };
    match list_val {
        Value::List(items) => {
            let len = items.len() as i64;
            let s = start.max(0) as usize;
            let e = end.max(0).min(len) as usize;
            if s >= e {
                Ok(Value::List(Vec::new()))
            } else {
                Ok(Value::List(items.get(s..e).unwrap_or_default().to_vec()))
            }
        }
        _ => Err(RuntimeError::new(
            ctx.line,
            &call.command,
            i18n::first_arg_must_be(ctx.lang.get(), "list"),
        )),
    }
}

fn sort_value_list(items: Vec<Value>) -> Vec<Value> {
    let mut nums: Vec<(usize, i64)> = Vec::new();
    let mut strs: Vec<(usize, String)> = Vec::new();
    let mut others: Vec<(usize, Value)> = Vec::new();

    for (i, item) in items.iter().enumerate() {
        match item {
            Value::Num(n) => nums.push((i, *n)),
            Value::Str(s) => strs.push((i, s.clone())),
            Value::Expr(e) => {
                nums.push((i, e.to_f64() as i64));
            }
            other => others.push((i, other.clone())),
        }
    }

    nums.sort_by_key(|&(_, n)| n);
    strs.sort_by(|a, b| a.1.cmp(&b.1));

    let mut result: Vec<Value> = Vec::with_capacity(items.len());
    for (_, n) in nums {
        result.push(Value::Num(n));
    }
    for (_, s) in strs {
        result.push(Value::Str(s));
    }
    for (_, v) in others {
        result.push(v);
    }
    result
}
