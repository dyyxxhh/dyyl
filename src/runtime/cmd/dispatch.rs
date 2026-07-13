//! Command router — maps command names to handler functions.
//!
//! `eval_expr` lives here because it recursively calls `dispatch_call`
//! for nested command expressions (circular dependency).

use super::ai;
use super::containers;
use super::context::ExecContext;
use super::file;
use super::io;
use super::logic;
use super::math;
use super::mcm;
use super::net;
use super::str;
use super::system;
use super::time_cmd;
use super::user;
use super::vars;
use crate::i18n::{self, Lang};
use crate::math::CasNumber;
use crate::parser::types::{Call, Expr};
use crate::runtime::env::Env;
use crate::runtime::error::RuntimeError;
use crate::runtime::value::Value;

/// Route a `Call` to the appropriate command-family handler.
pub(crate) fn dispatch_call(
    call: &Call,
    env: &mut Env,
    ctx: &ExecContext,
) -> Result<Value, RuntimeError> {
    match call.command.as_str() {
        "language" => handle_language(call, env, ctx),

        // math.* commands — route through math dispatcher
        cmd if cmd.starts_with("math.") => math::handle_math_command(call, env, ctx),

        cmd if cmd.starts_with("logic.") => {
            let sub = &cmd["logic.".len()..];
            logic::dispatch_logic(call, env, ctx, sub)
        }

        cmd if cmd.starts_with("ai.") => ai::handle_ai_command(call, env, ctx),

        cmd if cmd.starts_with("str.") => str::handle_str_command(call, env, ctx),

        "io.out" => io::handle_io_out(call, env, ctx),
        "io.changeline" => io::handle_io_changeline(),
        "io.in" => io::handle_io_in(call, ctx),
        "io.get" => io::handle_io_get(call, ctx),
        "io.inpasswd" => io::handle_io_inpasswd(call, env, ctx),

        cmd if cmd.starts_with("dict.") => containers::handle_dict_command(call, env, ctx),
        cmd if cmd.starts_with("list.") => containers::handle_list_command(call, env, ctx),

        cmd if cmd.starts_with("file.") => file::handle_file_command(call, env, ctx),
        cmd if cmd.starts_with("net.") => net::handle_net_command(call, env, ctx),

        cmd if cmd.starts_with("user.") => user::handle_user_command(call, env, ctx),
        cmd if cmd.starts_with("system.") => system::handle_system_command(call, env, ctx),
        cmd if cmd.starts_with("time.") => time_cmd::handle_time_command(call, env, ctx),

        cmd if cmd.starts_with("mcm.") => mcm::handle_mcm_command(call, env, ctx),

        "set" => vars::handle_set(call, env, ctx),
        "create.num" => vars::handle_create_num(call, env, ctx),
        "create.str" => vars::handle_create_str(call, env, ctx),

        _ => {
            // Fallback: if command contains a dot and prefix isn't a known
            // family, treat as a plugin command (`<name>.<sub>[.<sub>...]`).
            if call.command.contains('.') {
                super::plugin::dispatch_plugin_command(&call.command, call, env, ctx)
            } else {
                Err(RuntimeError::new(
                    ctx.line,
                    &call.command,
                    i18n::unknown_top_command(ctx.lang.get(), &call.command),
                ))
            }
        }
    }
}

fn handle_language(call: &Call, env: &mut Env, ctx: &ExecContext) -> Result<Value, RuntimeError> {
    match call.args.first() {
        None => Ok(Value::Str(ctx.lang.get().name().to_string())),
        Some(expr) => {
            let val = eval_expr(expr, env, ctx)?;
            let name = match val {
                Value::Str(s) => s,
                Value::Num(n) => n.to_string(),
                Value::Expr(e) => e.to_string(),
                _ => {
                    return Err(RuntimeError::new(
                        ctx.line,
                        &call.command,
                        i18n::expected_string(ctx.lang.get(), &val),
                    ));
                }
            };
            match Lang::from_name(&name) {
                Some(lang) => {
                    env.set_lang(lang);
                    ctx.lang.set(lang);
                    // Persist language preference to config file
                    if let Ok(mut config) = crate::config::load_config() {
                        config.lang = Some(lang.name().to_string());
                        if let Err(e) = crate::config::save_config(&config) {
                            eprintln!("warning: failed to save language preference: {e}");
                        }
                    }
                    Ok(Value::Str(lang.name().to_string()))
                }
                None => Err(RuntimeError::new(
                    ctx.line,
                    &call.command,
                    format!("unknown language '{name}'"),
                )),
            }
        }
    }
}

/// Evaluate an `Expr` to a runtime `Value`.
pub(crate) fn eval_expr(
    expr: &Expr,
    env: &mut Env,
    ctx: &ExecContext,
) -> Result<Value, RuntimeError> {
    match expr {
        Expr::Num(n) => Ok(Value::Num(*n)),
        Expr::Fraction(a, b) => {
            if *b == 0 {
                return Err(RuntimeError::new(
                    ctx.line,
                    "",
                    i18n::division_by_zero(ctx.lang.get()).to_string(),
                ));
            }
            Ok(Value::Expr(CasNumber::reduce(*a, *b)))
        }
        Expr::Pi => Ok(Value::Expr(CasNumber::Const(crate::math::SymConstant::Pi))),
        Expr::Sqrt(radicand) => {
            let rad_val = parse_radicand(radicand, ctx.line, ctx.lang.get())?;
            Ok(Value::Expr(CasNumber::Sqrt(Box::new(rad_val))))
        }
        Expr::Param(s) => {
            if s.starts_with('$') {
                let name = &s[1..];
                env.get(name).cloned().ok_or_else(|| {
                    RuntimeError::new(ctx.line, "", i18n::undefined_variable(ctx.lang.get(), s))
                })
            } else {
                Ok(Value::Str(s.clone()))
            }
        }
        Expr::Call(call) => dispatch_call(call, env, &ctx.for_call(call)),
        Expr::Empty => Ok(Value::Empty),
    }
}

/// Parse a radicand string (from `√<radicand>` literal) into a `CasNumber`.
fn parse_radicand(s: &str, line: usize, lang: Lang) -> Result<CasNumber, RuntimeError> {
    let trimmed = s.trim();
    if let Some(slash_pos) = trimmed.find('/') {
        let num_str = trimmed[..slash_pos].trim();
        let den_str = trimmed[slash_pos + 1..].trim();
        let num: i64 = num_str
            .parse()
            .map_err(|_| RuntimeError::new(line, "", i18n::invalid_sqrt_num(lang, num_str)))?;
        let den: i64 = den_str
            .parse()
            .map_err(|_| RuntimeError::new(line, "", i18n::invalid_sqrt_den(lang, den_str)))?;
        if den == 0 {
            return Err(RuntimeError::new(
                line,
                "",
                i18n::sqrt_den_zero(lang).to_string(),
            ));
        }
        Ok(CasNumber::reduce(num, den))
    } else {
        let n: i64 = trimmed
            .parse()
            .map_err(|_| RuntimeError::new(line, "", i18n::invalid_sqrt(lang, trimmed)))?;
        Ok(CasNumber::Int(n))
    }
}
