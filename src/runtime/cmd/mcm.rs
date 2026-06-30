//! MCM command dispatch — routes `mcm.*` commands through the host provider.
//!
//! When a `HostProvider` is available, `mcm.*` commands are serialised as
//! `McmCommand` events, sent to the host, and the `McmResponse` is mapped
//! back to a dyyl `Value`. Without a host provider, the command produces
//! a `RuntimeError`.

use super::context::ExecContext;
use crate::i18n;
use crate::parser::types::Call;
use crate::runtime::env::Env;
use crate::runtime::error::RuntimeError;
use crate::runtime::host_provider::{
    McmArg, McmCommand, McmResponse,
};
use crate::runtime::value::Value;

/// Handle a `mcm.*` command by forwarding it to the host provider.
///
/// Returns `Err` if no host provider is attached to the environment.
pub(crate) fn handle_mcm_command(
    call: &Call,
    env: &mut Env,
    ctx: &ExecContext,
) -> Result<Value, RuntimeError> {
    let provider = env
        .host_provider()
        .cloned()
        .ok_or_else(|| {
            RuntimeError::new(
                ctx.line,
                &call.command,
                i18n::mcm_no_host_provider(ctx.lang.get()),
            )
        })?;

    let mcm_args: Vec<McmArg> = call
        .args
        .iter()
        .map(|expr| {
            let val = super::dispatch::eval_expr(expr, env, ctx)?;
            Ok(McmArg::from_value(&val))
        })
        .collect::<Result<Vec<McmArg>, RuntimeError>>()?;

    let id = env.mcm_next_id();
    let cmd = McmCommand {
        kind: "mcm_command".to_string(),
        id,
        name: call.command.clone(),
        args: mcm_args,
        source_line: ctx.text.clone(),
    };

    let resp = provider.send_command(&cmd).map_err(|e| {
        crate::runtime::host_provider::host_error_to_runtime(&e, ctx.line, &call.command)
    })?;

    mcm_response_to_value(&resp, ctx, env)
}

/// Map an `McmResponse` to a dyyl `Value`.
fn mcm_response_to_value(
    resp: &McmResponse,
    ctx: &ExecContext,
    env: &mut Env,
) -> Result<Value, RuntimeError> {
    if resp.ok {
        // Handle game.choose scope tracking.
        if ctx.command == "mcm.game.choose" {
            if let Some(McmArg::Str(version)) = &resp.value {
                env.game_scope_mut().select(version.clone());
            }
        }
        Ok(match &resp.value {
            Some(arg) => arg.to_value(),
            None => Value::Empty,
        })
    } else {
        Err(crate::runtime::host_provider::mcm_response_error_to_runtime(
            resp,
            ctx.line,
            &ctx.command,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::host_provider::{GameChooseScope, MockHostProvider};
    use std::sync::Arc;

    #[test]
    fn mcm_command_dispatches_to_host() {
        let mock = Arc::new(MockHostProvider::with_responses(vec![
            MockHostProvider::ok_response("1", McmArg::Str("1.21.1".to_string())),
        ]));
        let mut env = Env::new();
        env.set_host_provider(mock);

        let ctx = ExecContext {
            line: 1,
            text: "mcm.game.choose 1.21.1".to_string(),
            command: "mcm.game.choose".to_string(),
            debug: false,
            io_provider: Arc::new(crate::runtime::io_provider::StdIoProvider),
            lang: std::cell::Cell::new(crate::i18n::Lang::En),
        };

        let call = Call {
            command: "mcm.game.choose".to_string(),
            args: vec![crate::parser::types::Expr::Param("1.21.1".to_string())],
        };

        let result = handle_mcm_command(&call, &mut env, &ctx);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Value::Str("1.21.1".to_string()));
    }

    #[test]
    fn mcm_command_error_propagation() {
        let mock = Arc::new(MockHostProvider::with_responses(vec![
            MockHostProvider::error_response("1", "host_timeout", "command timed out"),
        ]));
        let mut env = Env::new();
        env.set_host_provider(mock);

        let ctx = ExecContext {
            line: 3,
            text: "mcm.game.install 1.21.1".to_string(),
            command: "mcm.game.install".to_string(),
            debug: false,
            io_provider: Arc::new(crate::runtime::io_provider::StdIoProvider),
            lang: std::cell::Cell::new(crate::i18n::Lang::En),
        };

        let call = Call {
            command: "mcm.game.install".to_string(),
            args: vec![crate::parser::types::Expr::Param("1.21.1".to_string())],
        };

        let result = handle_mcm_command(&call, &mut env, &ctx);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.line, 3);
        assert!(err.reason.contains("host_timeout"));
    }

    #[test]
    fn mcm_command_no_host_provider_returns_error() {
        let mut env = Env::new();
        let ctx = ExecContext {
            line: 1,
            text: "mcm.game.choose 1.21.1".to_string(),
            command: "mcm.game.choose".to_string(),
            debug: false,
            io_provider: Arc::new(crate::runtime::io_provider::StdIoProvider),
            lang: std::cell::Cell::new(crate::i18n::Lang::En),
        };

        let call = Call {
            command: "mcm.game.choose".to_string(),
            args: vec![crate::parser::types::Expr::Param("1.21.1".to_string())],
        };

        let result = handle_mcm_command(&call, &mut env, &ctx);
        assert!(result.is_err());
    }

    #[test]
    fn mcm_game_choose_updates_scope() {
        let mock = Arc::new(MockHostProvider::with_responses(vec![
            MockHostProvider::ok_response("1", McmArg::Str("1.21.1".to_string())),
        ]));
        let mut env = Env::new();
        env.set_host_provider(mock);
        assert!(env.game_scope().current().is_none());

        let ctx = ExecContext {
            line: 1,
            text: "mcm.game.choose 1.21.1".to_string(),
            command: "mcm.game.choose".to_string(),
            debug: false,
            io_provider: Arc::new(crate::runtime::io_provider::StdIoProvider),
            lang: std::cell::Cell::new(crate::i18n::Lang::En),
        };

        let call = Call {
            command: "mcm.game.choose".to_string(),
            args: vec![crate::parser::types::Expr::Param("1.21.1".to_string())],
        };

        let _ = handle_mcm_command(&call, &mut env, &ctx);
        assert_eq!(env.game_scope().current(), Some("1.21.1"));
    }

    #[test]
    fn game_choose_scope_tracks_selection() {
        let mut scope = GameChooseScope::default();
        assert!(scope.current().is_none());

        scope.select("1.21.1".to_string());
        assert_eq!(scope.current(), Some("1.21.1"));

        scope.select("1.20.4".to_string());
        assert_eq!(scope.current(), Some("1.20.4"));

        scope.reset();
        assert!(scope.current().is_none());
    }
}
