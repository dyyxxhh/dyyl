//! Plugin CLI subcommands: install, update, remove, autoremove, list.

use crate::i18n::Lang;

/// Dispatch a plugin subcommand. Returns exit code.
pub fn dispatch(sub: &str, args: &[String], lang: Lang) -> i32 {
    match sub {
        "install" => cmd_install(args, lang),
        "update" => cmd_update(args, lang),
        "remove" => cmd_remove(args, lang),
        "autoremove" => cmd_autoremove(args, lang),
        "list" => cmd_list(args, lang),
        _ => {
            eprintln!("{}", crate::i18n::cli_plugin_subcommand_unknown(lang, sub));
            1
        }
    }
}

fn cmd_install(args: &[String], lang: Lang) -> i32 {
    let _ = (args, lang);
    eprintln!("install: not yet implemented");
    1
}

fn cmd_update(args: &[String], lang: Lang) -> i32 {
    let _ = (args, lang);
    eprintln!("update: not yet implemented");
    1
}

fn cmd_remove(args: &[String], lang: Lang) -> i32 {
    let _ = (args, lang);
    eprintln!("remove: not yet implemented");
    1
}

fn cmd_autoremove(args: &[String], lang: Lang) -> i32 {
    let _ = (args, lang);
    eprintln!("autoremove: not yet implemented");
    1
}

fn cmd_list(args: &[String], lang: Lang) -> i32 {
    let _ = (args, lang);
    eprintln!("list: not yet implemented");
    1
}
