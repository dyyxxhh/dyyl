//! Execution context — typed metadata bundle for command dispatch.
//!
//! Collected here so individual handler functions pass ≤3 parameters:
//! `(call, env, ctx)` instead of separate `line`, `text`, `command`, `debug`.
//!
//! Constructors take domain objects so each stays ≤2 params:
//! - `ExecContext::from_command(&ParsedCommand, debug, provider)` — top-level
//! - `ctx.for_call(&Call)` — nested command dispatch

use std::cell::Cell;
use std::fmt;
use std::sync::Arc;

use crate::i18n::Lang;
use crate::parser::types::{Call, ParsedCommand};
use crate::runtime::io_provider::IoProvider;

/// Execution context passed through command dispatch and expression eval.
///
/// `text` and `command` are owned so `for_call` borrows seamlessly from
/// both the parent context and the nested call without lifetime conflicts.
#[derive(Clone)]
pub(crate) struct ExecContext {
    /// 1-based source line number.
    pub line: usize,
    /// Raw command text (as it appears in source).
    pub text: String,
    /// The current command name.
    pub command: String,
    /// Whether debug stderr warnings are enabled.
    pub debug: bool,
    /// IO provider for terminal input commands.
    pub io_provider: Arc<dyn IoProvider>,
    pub lang: Cell<Lang>,
}

impl fmt::Debug for ExecContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ExecContext")
            .field("line", &self.line)
            .field("text", &self.text)
            .field("command", &self.command)
            .field("debug", &self.debug)
            .field("lang", &self.lang.get())
            .finish_non_exhaustive()
    }
}

impl ExecContext {
    /// Create context from a parsed command (top-level execution).
    #[must_use]
    pub(crate) fn from_command(
        cmd: &ParsedCommand,
        debug: bool,
        io_provider: Arc<dyn IoProvider>,
        lang: Lang,
    ) -> Self {
        Self {
            line: cmd.line,
            text: cmd.text.clone(),
            command: cmd.call.command.clone(),
            debug,
            io_provider,
            lang: Cell::new(lang),
        }
    }

    /// Derive context for a nested call, updating the command name while
    /// preserving line, text, debug, io_provider, and lang from the parent.
    #[must_use]
    pub(crate) fn for_call(&self, call: &Call) -> Self {
        Self {
            line: self.line,
            text: self.text.clone(),
            command: call.command.clone(),
            debug: self.debug,
            io_provider: Arc::clone(&self.io_provider),
            lang: Cell::new(self.lang.get()),
        }
    }
}
