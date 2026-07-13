//! CLI subcommand dispatch for plugin management.
//!
//! Entry point for `dyyl install|update|remove|autoremove|list`.

pub mod plugin_cmds;

use crate::i18n::Lang;

/// Outcome of CLI subcommand handling.
pub enum CliResult {
    /// Subcommand handled, exit with this code.
    Handled(i32),
    /// Not a subcommand — continue with normal script execution.
    NotASubcommand,
}

/// Check if args[1] is a known subcommand; if so, handle it.
///
/// Global options like `--lang` must come before the subcommand.
pub fn try_handle_subcommand(args: &[String], lang: &mut Lang) -> CliResult {
    // Find first non-flag arg (the subcommand candidate).
    let mut i = 1;
    while i < args.len() {
        let arg = match args.get(i) {
            Some(a) => a.as_str(),
            None => break,
        };
        match arg {
            "--lang" => {
                i += 1;
                if let Some(val) = args.get(i) {
                    if let Some(l) = Lang::from_name(val) {
                        *lang = l;
                    }
                }
            }
            "--debug" | "--host-json" => { /* ignore for CLI */ }
            "install" | "update" | "remove" | "autoremove" | "list" => {
                let rest = args.get(i + 1..).unwrap_or(&[]);
                let code = plugin_cmds::dispatch(arg, rest, *lang);
                return CliResult::Handled(code);
            }
            _ => return CliResult::NotASubcommand,
        }
        i += 1;
    }
    CliResult::NotASubcommand
}
