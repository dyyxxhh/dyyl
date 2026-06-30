//! Time command handlers — time.get, time.now, time.year/month/day/hour/
//! minute/second, time.weekday, time.weekday.name, time.format, time.diff,
//! time.add.
//!
//! Uses `chrono` for calendar arithmetic and formatting.

use chrono::{Datelike, Local, Timelike};

use super::context::ExecContext;
use super::dispatch::eval_expr;
use crate::i18n;
use crate::parser::types::Call;
use crate::runtime::env::Env;
use crate::runtime::error::RuntimeError;
use crate::runtime::value::Value;

/// Route a `time.*` call to the appropriate handler.
pub(crate) fn handle_time_command(
    call: &Call,
    env: &mut Env,
    ctx: &ExecContext,
) -> Result<Value, RuntimeError> {
    let sub = &call.command["time.".len()..];
    match sub {
        "get" => handle_time_get(),
        "now" => handle_time_now(),
        "year" => handle_time_year(),
        "month" => handle_time_month(),
        "day" => handle_time_day(),
        "hour" => handle_time_hour(),
        "minute" => handle_time_minute(),
        "second" => handle_time_second(),
        "weekday" => handle_time_weekday(),
        "weekday.name" => handle_time_weekday_name(),
        "format" => handle_time_format(call, env, ctx),
        "diff" => handle_time_diff(call, env, ctx),
        "add" => handle_time_add(call, env, ctx),
        "wait" => handle_time_wait(call, env, ctx),
        _ => Err(RuntimeError::new(
            ctx.line,
            &call.command,
            i18n::unknown_command(ctx.lang.get(), "time", sub),
        )),
    }
}

// ── Helpers ────────────────────────────────────────────────────────

/// Resolve arg at `idx` to an i64 value.
fn resolve_num_arg(
    call: &Call,
    env: &mut Env,
    ctx: &ExecContext,
    idx: usize,
) -> Result<i64, RuntimeError> {
    let expr = call.args.get(idx).ok_or_else(|| {
        RuntimeError::new(
            ctx.line,
            &call.command,
            i18n::requires_args(ctx.lang.get(), idx + 1),
        )
    })?;
    let val = eval_expr(expr, env, ctx)?;
    match val {
        Value::Num(n) => Ok(n),
        Value::Str(s) => s.parse::<i64>().map_err(|_| {
            RuntimeError::new(
                ctx.line,
                &call.command,
                i18n::expected_numeric_str(ctx.lang.get(), &s),
            )
        }),
        Value::Expr(e) => Ok(e.to_f64() as i64),
        _ => Err(RuntimeError::new(
            ctx.line,
            &call.command,
            i18n::expected_numeric(ctx.lang.get(), &val),
        )),
    }
}

/// Resolve arg at `idx` to a string value.
fn resolve_str_arg(
    call: &Call,
    env: &mut Env,
    ctx: &ExecContext,
    idx: usize,
) -> Result<String, RuntimeError> {
    let expr = call.args.get(idx).ok_or_else(|| {
        RuntimeError::new(
            ctx.line,
            &call.command,
            i18n::requires_args(ctx.lang.get(), idx + 1),
        )
    })?;
    let val = eval_expr(expr, env, ctx)?;
    match val {
        Value::Str(s) => Ok(s),
        Value::Num(n) => Ok(n.to_string()),
        Value::Expr(e) => Ok(e.to_string()),
        _ => Err(RuntimeError::new(
            ctx.line,
            &call.command,
            i18n::expected_string(ctx.lang.get(), &val),
        )),
    }
}

// ── Handlers ───────────────────────────────────────────────────────

/// Handle `time.get` — return Unix timestamp (seconds since 1970-01-01).
fn handle_time_get() -> Result<Value, RuntimeError> {
    let now = Local::now().timestamp();
    Ok(Value::Num(now))
}

/// Handle `time.now` — return current datetime as `YYYY-MM-DD HH:mm:ss`.
fn handle_time_now() -> Result<Value, RuntimeError> {
    let now = Local::now();
    Ok(Value::Str(now.format("%Y-%m-%d %H:%M:%S").to_string()))
}

/// Handle `time.year` — return current year (1–9999).
fn handle_time_year() -> Result<Value, RuntimeError> {
    let now = Local::now();
    Ok(Value::Num(i64::from(now.year())))
}

/// Handle `time.month` — return current month (1–12).
fn handle_time_month() -> Result<Value, RuntimeError> {
    let now = Local::now();
    Ok(Value::Num(i64::from(now.month())))
}

/// Handle `time.day` — return current day of month (1–31).
fn handle_time_day() -> Result<Value, RuntimeError> {
    let now = Local::now();
    Ok(Value::Num(i64::from(now.day())))
}

/// Handle `time.hour` — return current hour (0–23).
fn handle_time_hour() -> Result<Value, RuntimeError> {
    let now = Local::now();
    Ok(Value::Num(i64::from(now.hour())))
}

/// Handle `time.minute` — return current minute (0–59).
fn handle_time_minute() -> Result<Value, RuntimeError> {
    let now = Local::now();
    Ok(Value::Num(i64::from(now.minute())))
}

/// Handle `time.second` — return current second (0–59).
fn handle_time_second() -> Result<Value, RuntimeError> {
    let now = Local::now();
    Ok(Value::Num(i64::from(now.second())))
}

/// Handle `time.weekday` — return weekday number (1=Monday, 7=Sunday).
fn handle_time_weekday() -> Result<Value, RuntimeError> {
    let now = Local::now();
    // chrono weekday: 0=Monday, 6=Sunday → shift to 1=Monday, 7=Sunday
    let wd = i64::from(now.weekday().num_days_from_monday()) + 1;
    Ok(Value::Num(wd))
}

/// Handle `time.weekday.name` — return weekday name (e.g. "Monday").
fn handle_time_weekday_name() -> Result<Value, RuntimeError> {
    let now = Local::now();
    Ok(Value::Str(now.format("%A").to_string()))
}

/// Handle `time.format <fmt>` — apply a custom format string.
///
/// Supported placeholders: `YYYY` `MM` `DD` `HH` `mm` `ss`.
fn handle_time_format(
    call: &Call,
    env: &mut Env,
    ctx: &ExecContext,
) -> Result<Value, RuntimeError> {
    let fmt = resolve_str_arg(call, env, ctx, 0)?;
    let now = Local::now();
    let result = fmt
        .replace("YYYY", &now.format("%Y").to_string())
        .replace("MM", &now.format("%m").to_string())
        .replace("DD", &now.format("%d").to_string())
        .replace("HH", &now.format("%H").to_string())
        .replace("mm", &now.format("%M").to_string())
        .replace("ss", &now.format("%S").to_string());
    Ok(Value::Str(result))
}

/// Handle `time.diff <ts1>, <ts2>` — return the difference in seconds.
fn handle_time_diff(call: &Call, env: &mut Env, ctx: &ExecContext) -> Result<Value, RuntimeError> {
    let ts1 = resolve_num_arg(call, env, ctx, 0)?;
    let ts2 = resolve_num_arg(call, env, ctx, 1)?;
    Ok(Value::Num(ts2 - ts1))
}

/// Handle `time.add <timestamp>, <seconds>` — return new timestamp.
fn handle_time_add(call: &Call, env: &mut Env, ctx: &ExecContext) -> Result<Value, RuntimeError> {
    let ts = resolve_num_arg(call, env, ctx, 0)?;
    let secs = resolve_num_arg(call, env, ctx, 1)?;
    Ok(Value::Num(ts + secs))
}

/// Handle `time.wait <milliseconds>` — sleep for the given duration,
/// then return the original value.
fn handle_time_wait(call: &Call, env: &mut Env, ctx: &ExecContext) -> Result<Value, RuntimeError> {
    let ms = resolve_num_arg(call, env, ctx, 0)?;
    if ms < 0 {
        return Err(RuntimeError::new(
            ctx.line,
            &call.command,
            i18n::time_wait_nonnegative(ctx.lang.get()).to_string(),
        ));
    }
    std::thread::sleep(std::time::Duration::from_millis(ms as u64));
    Ok(Value::Num(ms))
}
