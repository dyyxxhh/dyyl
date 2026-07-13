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
    let _ = args;
    let installed = match crate::runtime::plugin::registry::scan_installed() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("{e}");
            return 1;
        }
    };
    if installed.is_empty() {
        println!("{}", crate::i18n::plugin_list_empty(lang));
        return 0;
    }
    // Header
    println!("{}", crate::i18n::plugin_list_header(lang));
    for p in &installed {
        // Read plugin.toml for installed_at.
        let toml_content = match std::fs::read_to_string(&p.toml_path) {
            Ok(s) => s,
            Err(_) => continue,
        };
        let toml: crate::runtime::plugin::manifest::LocalPluginToml = match toml::from_str(&toml_content) {
            Ok(t) => t,
            Err(_) => continue,
        };
        // Check config for last_used_at.
        let last_used = crate::config::load_config()
            .ok()
            .and_then(|c| {
                c.installed_plugins
                    .get(&p.name)
                    .and_then(|r| r.last_used_at.clone())
            })
            .unwrap_or_else(|| "-".to_string());
        println!(
            "{} {} {} {}",
            p.name, p.version, last_used, toml.installed.installed_at
        );
    }
    0
}
