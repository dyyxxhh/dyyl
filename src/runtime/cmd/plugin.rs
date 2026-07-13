//! Plugin command dispatch router.
//!
//! Called from `dispatch.rs` fallback arm when a command starts with an
//! unknown prefix (not `math.`/`str.`/`io.`/etc). Splits `<name>.<rest>`
//! and routes to `PluginManager`.

use super::context::ExecContext;
use super::dispatch::eval_expr;
use crate::i18n;
use crate::parser::types::Call;
use crate::runtime::env::Env;
use crate::runtime::error::RuntimeError;
use crate::runtime::value::Value;

/// Dispatch a `<name>.<sub>[.<sub>...]` command to the plugin manager.
///
/// `full_command` is the complete command string (e.g. `migpt.user.login`).
/// The plugin name is the segment before the first dot; the rest (which may
/// contain further dots) is the sub-command passed to the plugin.
pub(crate) fn dispatch_plugin_command(
    full_command: &str,
    call: &Call,
    env: &mut Env,
    ctx: &ExecContext,
) -> Result<Value, RuntimeError> {
    let dot_pos = full_command.find('.').ok_or_else(|| {
        RuntimeError::new(
            ctx.line,
            full_command,
            i18n::unknown_top_command(ctx.lang.get(), full_command),
        )
    })?;
    let plugin_name = &full_command[..dot_pos];
    let sub = &full_command[dot_pos + 1..];

    let mut args = Vec::with_capacity(call.args.len());
    for expr in &call.args {
        args.push(eval_expr(expr, env, ctx)?);
    }

    env.plugin_manager()
        .dispatch(plugin_name, sub, &args, ctx.lang.get(), ctx.line)
}
