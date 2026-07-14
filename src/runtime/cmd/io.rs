use super::context::ExecContext;
use super::dispatch::eval_expr;
use crate::i18n;
use crate::parser::types::Call;
use crate::runtime::env::Env;
use crate::runtime::error::RuntimeError;
use crate::runtime::io_provider::IoError;
use crate::runtime::value::Value;

pub(crate) fn handle_io_out(
    call: &Call,
    env: &mut Env,
    ctx: &ExecContext,
) -> Result<Value, RuntimeError> {
    if call.args.is_empty() {
        return Ok(Value::Empty);
    }
    let val = eval_expr(
        call.args
            .first()
            .ok_or_else(|| RuntimeError::new(ctx.line, &call.command, "missing argument"))?,
        env,
        ctx,
    )?;
    println!("{}", val);
    Ok(val)
}

pub(crate) fn handle_io_changeline() -> Result<Value, RuntimeError> {
    println!();
    Ok(Value::Empty)
}

pub(crate) fn handle_io_in(call: &Call, ctx: &ExecContext) -> Result<Value, RuntimeError> {
    if !call.args.is_empty() {
        return Err(RuntimeError::new(
            ctx.line,
            &call.command,
            i18n::io_in_no_args(ctx.lang.get()).to_string(),
        ));
    }
    match ctx.io_provider.read_line("") {
        Ok(line) => Ok(Value::Str(line)),
        Err(IoError::NoInputAvailable) => {
            if ctx.debug {
                eprintln!(
                    "line {}: {}",
                    ctx.line,
                    i18n::warn_io_in_no_input(ctx.lang.get())
                );
            }
            Ok(Value::sentinel_str())
        }
        Err(IoError::Io(e)) => {
            if ctx.debug {
                eprintln!("line {}: io.in — {e}", ctx.line);
            }
            Ok(Value::sentinel_str())
        }
    }
}

pub(crate) fn handle_io_get(call: &Call, ctx: &ExecContext) -> Result<Value, RuntimeError> {
    if !call.args.is_empty() {
        return Err(RuntimeError::new(
            ctx.line,
            &call.command,
            i18n::io_get_no_args(ctx.lang.get()).to_string(),
        ));
    }
    match ctx.io_provider.read_key() {
        Ok(key) => Ok(Value::Str(key)),
        Err(IoError::NoInputAvailable) => {
            if ctx.debug {
                eprintln!(
                    "line {}: {}",
                    ctx.line,
                    i18n::warn_io_get_no_input(ctx.lang.get())
                );
            }
            Ok(Value::sentinel_str())
        }
        Err(IoError::Io(e)) => {
            if ctx.debug {
                eprintln!("line {}: io.get — {e}", ctx.line);
            }
            Ok(Value::sentinel_str())
        }
    }
}

pub(crate) fn handle_io_inpasswd(
    call: &Call,
    env: &mut Env,
    ctx: &ExecContext,
) -> Result<Value, RuntimeError> {
    let mode = if call.args.is_empty() {
        0i64
    } else {
        let val = eval_expr(
            call.args
                .first()
                .ok_or_else(|| RuntimeError::new(ctx.line, &call.command, "missing argument"))?,
            env,
            ctx,
        )?;
        match val {
            Value::Num(n) => n,
            Value::Str(s) => s.parse::<i64>().unwrap_or(0),
            _ => 0,
        }
    };

    let prompt = if mode == 1 { "* " } else { "" };
    match ctx.io_provider.read_password(prompt) {
        Ok(pwd) => Ok(Value::Str(pwd)),
        Err(IoError::NoInputAvailable) => {
            if ctx.debug {
                eprintln!(
                    "line {}: {}",
                    ctx.line,
                    i18n::warn_io_inpasswd_no_input(ctx.lang.get())
                );
            }
            Ok(Value::sentinel_str())
        }
        Err(IoError::Io(e)) => {
            if ctx.debug {
                eprintln!("line {}: io.inpasswd — {e}", ctx.line);
            }
            Ok(Value::sentinel_str())
        }
    }
}
