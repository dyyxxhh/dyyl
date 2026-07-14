use super::context::ExecContext;
use crate::i18n;
use crate::parser::types::Expr;
use crate::runtime::env::Env;
use crate::runtime::error::RuntimeError;
use crate::runtime::value::Value;

pub(crate) fn resolve_var_name(expr: &Expr, ctx: &ExecContext) -> Result<String, RuntimeError> {
    match expr {
        Expr::Param(s) => Ok(if let Some(rest) = s.strip_prefix('$') {
            rest.to_string()
        } else {
            s.clone()
        }),
        _ => Err(RuntimeError::new(
            ctx.line,
            ctx.command.as_str(),
            i18n::expected_var_name(ctx.lang.get(), expr),
        )),
    }
}

pub(crate) fn resolve_container(
    expr: &Expr,
    env: &Env,
    ctx: &ExecContext,
) -> Result<Value, RuntimeError> {
    let name = match expr {
        Expr::Param(s) => {
            if let Some(rest) = s.strip_prefix('$') {
                rest.to_string()
            } else {
                s.clone()
            }
        }
        _ => {
            return Err(RuntimeError::new(
                ctx.line,
                ctx.command.as_str(),
                i18n::expected_var_ref(ctx.lang.get(), expr),
            ));
        }
    };
    env.get(&name).cloned().ok_or_else(|| {
        RuntimeError::new(
            ctx.line,
            ctx.command.as_str(),
            i18n::undefined_variable(ctx.lang.get(), &name),
        )
    })
}
