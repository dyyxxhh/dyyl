//! ai.* 命令 handler — ai.ask（运行时 HTTP）+ ai.auto.filled（取值）。
//!
//! `ai.ask` 在运行时加载凭证、构造 Provider 并发起一次 HTTP 请求；任何
//! 失败（无配置目录、凭证缺失、网络/鉴权错误）都静默返回 `Value::Num(-1)`
//! 并在 `--debug` 时向 stderr 输出原因。
//!
//! `ai.auto.filled` 运行时不请求 AI，直接返回第二个参数（值），第一个
//! 参数（提示）被忽略——它由 prepass 阶段在源码中回写生成。

use super::context::ExecContext;
use super::dispatch::eval_expr;
use crate::i18n;
use crate::parser::types::Call;
use crate::runtime::env::Env;
use crate::runtime::error::RuntimeError;
use crate::runtime::value::Value;

/// 默认 ai.ask system prompt（单参数或 `_` 跳过 system 时用）。
const DEFAULT_ASK_SYSTEM_PROMPT: &str =
    "You are a helpful assistant. Answer the user's question concisely and accurately.";

/// 路由 ai.* 命令。
pub(crate) fn handle_ai_command(
    call: &Call,
    env: &mut Env,
    ctx: &ExecContext,
) -> Result<Value, RuntimeError> {
    // dispatch_call 已保证 `call.command` 以 "ai." 开头；用 strip_prefix
    // 取子命令名，避免字符串切片（clippy::indexing_slicing = deny）。
    let sub = call.command.strip_prefix("ai.").unwrap_or(&call.command);
    match sub {
        "ask" => handle_ai_ask(call, env, ctx),
        "auto.filled" => handle_ai_auto_filled(call, env, ctx),
        _ => Err(RuntimeError::new(
            ctx.line,
            &call.command,
            i18n::unknown_command(ctx.lang.get(), "ai", sub),
        )),
    }
}

/// ai.ask [system], <prompt>
///
/// 单参数：用内置默认 system。双参数：自定义 system（`_` 跳过 system）。
/// 任何失败都返回 `Value::Num(-1)`，仅在 `--debug` 时输出原因。
fn handle_ai_ask(call: &Call, env: &mut Env, ctx: &ExecContext) -> Result<Value, RuntimeError> {
    if call.args.is_empty() {
        return Err(RuntimeError::new(
            ctx.line,
            &call.command,
            i18n::requires_args(ctx.lang.get(), 1),
        ));
    }
    let (system, user_val) = if call.args.len() == 1 {
        // 单参数：用默认 system，user 为唯一参数。
        let user_expr = call.args.first().ok_or_else(|| {
            RuntimeError::new(
                ctx.line,
                &call.command,
                i18n::requires_args(ctx.lang.get(), 1),
            )
        })?;
        (
            DEFAULT_ASK_SYSTEM_PROMPT.to_string(),
            eval_expr(user_expr, env, ctx)?,
        )
    } else {
        // 双参数（及更多）：第一个为 system（`_` 用默认），第二个为 user。
        let first_expr = call.args.first().ok_or_else(|| {
            RuntimeError::new(
                ctx.line,
                &call.command,
                i18n::requires_args(ctx.lang.get(), 2),
            )
        })?;
        let first = eval_expr(first_expr, env, ctx)?;
        let user_expr = call.args.get(1).ok_or_else(|| {
            RuntimeError::new(
                ctx.line,
                &call.command,
                i18n::requires_args(ctx.lang.get(), 2),
            )
        })?;
        let user_val = eval_expr(user_expr, env, ctx)?;
        let sys_str = match first {
            Value::Empty => DEFAULT_ASK_SYSTEM_PROMPT.to_string(),
            Value::Str(s) => s,
            Value::Num(n) => n.to_string(),
            Value::Expr(e) => e.to_string(),
            _ => {
                return Err(RuntimeError::new(
                    ctx.line,
                    &call.command,
                    i18n::expected_string(ctx.lang.get(), &first),
                ));
            }
        };
        (sys_str, user_val)
    };
    let user_str = match user_val {
        Value::Str(s) => s,
        Value::Num(n) => n.to_string(),
        Value::Expr(e) => e.to_string(),
        _ => {
            return Err(RuntimeError::new(
                ctx.line,
                &call.command,
                i18n::expected_string(ctx.lang.get(), &user_val),
            ));
        }
    };

    // 加载凭证：无配置目录 → 静默 -1。
    let Some(creds_path) = crate::credentials::CredentialsFile::default_path() else {
        if ctx.debug {
            eprintln!(
                "line {}: {}",
                ctx.line,
                i18n::t(
                    ctx.lang.get(),
                    "ai.ask_failed",
                    &[("reason", "no config dir")]
                )
            );
        }
        return Ok(Value::Num(-1));
    };
    let ai_creds = match crate::credentials::ensure_ai(&creds_path, ctx.lang.get()) {
        Ok(c) => c,
        Err(e) => {
            if ctx.debug {
                eprintln!(
                    "line {}: {}",
                    ctx.line,
                    i18n::t(ctx.lang.get(), "ai.ask_failed", &[("reason", &e)])
                );
            }
            return Ok(Value::Num(-1));
        }
    };
    let provider = crate::ai::build_provider(&ai_creds);
    match provider.ask(&system, &user_str) {
        Ok(s) => Ok(Value::Str(s)),
        Err(e) => {
            if ctx.debug {
                eprintln!(
                    "line {}: {}",
                    ctx.line,
                    i18n::t(
                        ctx.lang.get(),
                        "ai.ask_failed",
                        &[("reason", &e.to_string())]
                    )
                );
            }
            Ok(Value::Num(-1))
        }
    }
}

/// ai.auto.filled <提示>, <值>
///
/// 运行时不请求 AI，直接返回值。提示参数忽略其内容。
fn handle_ai_auto_filled(
    call: &Call,
    env: &mut Env,
    ctx: &ExecContext,
) -> Result<Value, RuntimeError> {
    if call.args.len() < 2 {
        return Err(RuntimeError::new(
            ctx.line,
            &call.command,
            i18n::requires_args(ctx.lang.get(), 2),
        ));
    }
    let val_expr = call.args.get(1).ok_or_else(|| {
        RuntimeError::new(
            ctx.line,
            &call.command,
            i18n::requires_args(ctx.lang.get(), 2),
        )
    })?;
    eval_expr(val_expr, env, ctx)
}
