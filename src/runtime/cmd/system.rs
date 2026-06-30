//! System command handlers — system.os, system.arch.
//!
//! Returns the host operating system and CPU architecture as strings.

use crate::i18n;
use crate::parser::types::Call;
use crate::runtime::env::Env;
use crate::runtime::error::RuntimeError;
use crate::runtime::value::Value;

use super::context::ExecContext;

/// Route a `system.*` call to the appropriate handler.
pub(crate) fn handle_system_command(
    call: &Call,
    _env: &mut Env,
    ctx: &ExecContext,
) -> Result<Value, RuntimeError> {
    let sub = &call.command["system.".len()..];
    match sub {
        "os" => handle_system_os(),
        "arch" => handle_system_arch(),
        _ => Err(RuntimeError::new(
            ctx.line,
            &call.command,
            i18n::unknown_command(ctx.lang.get(), "system", sub),
        )),
    }
}

// ── Handlers ───────────────────────────────────────────────────────

/// Handle `system.os` — return the operating system name.
fn handle_system_os() -> Result<Value, RuntimeError> {
    Ok(Value::Str(std::env::consts::OS.to_string()))
}

/// Handle `system.arch` — return the CPU architecture.
fn handle_system_arch() -> Result<Value, RuntimeError> {
    Ok(Value::Str(std::env::consts::ARCH.to_string()))
}
