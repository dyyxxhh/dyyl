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
    if args.is_empty() {
        eprintln!("{}", crate::i18n::cli_plugin_usage(lang));
        return 1;
    }
    let name = &args[0];

    // Check if already installed with same version.
    if let Some(existing) = crate::runtime::plugin::registry::find_installed(name) {
        let toml_content = match std::fs::read_to_string(&existing.toml_path) {
            Ok(s) => s,
            Err(_) => String::new(),
        };
        if let Ok(t) = toml::from_str::<crate::runtime::plugin::manifest::LocalPluginToml>(&toml_content)
        {
            // Fetch remote manifest to check if same version.
            if let Ok(remote) = crate::runtime::plugin::fetch::fetch_manifest(name) {
                if remote.version == t.version {
                    println!(
                        "{}",
                        crate::i18n::plugin_already_installed(lang, name, &t.version)
                    );
                    return 0;
                }
            }
        }
    }

    // Install via fetch + download + verify + write.
    match install_plugin_by_name(name, lang) {
        Ok(version) => {
            println!(
                "{}",
                crate::i18n::plugin_install_success(lang, name, &version)
            );
            0
        }
        Err(e) => {
            eprintln!("{}", crate::i18n::plugin_install_failed(lang, name, &e));
            1
        }
    }
}

/// Install a plugin: fetch manifest, download, verify, write to XDG dir.
/// Returns the installed version string.
fn install_plugin_by_name(name: &str, lang: Lang) -> Result<String, String> {
    use crate::runtime::plugin::{
        abi::DYRL_API_VERSION, fetch, manifest::*, store,
    };
    use std::fs;

    let manifest = fetch::fetch_manifest(name).map_err(|e| format!("{e}"))?;

    if manifest.abi_version != DYRL_API_VERSION {
        return Err(crate::i18n::plugin_abi_mismatch(
            lang,
            name,
            DYRL_API_VERSION,
            manifest.abi_version,
        ));
    }

    let current = store::current_platform();
    let entry = manifest
        .platforms
        .iter()
        .find(|p| p.platform == current)
        .ok_or_else(|| format!("no build for {current}"))?;

    let bytes = fetch::download_and_verify(&entry.url, &entry.sha256)
        .map_err(|e| format!("{e}"))?;

    let lib_path = store::lib_path(name, &manifest.version);
    let toml_path = store::plugin_toml_path(name, &manifest.version);
    let version_dir = store::plugin_version_dir(name, &manifest.version);

    fs::create_dir_all(&version_dir).map_err(|e| format!("{e}"))?;
    fs::write(&lib_path, &bytes).map_err(|e| format!("{e}"))?;

    // Build local toml.
    let local_toml = LocalPluginToml {
        name: manifest.name.clone(),
        version: manifest.version.clone(),
        abi_version: manifest.abi_version,
        dyyl_min: manifest.dyyl_min.clone(),
        panic_mode: manifest.panic_mode.clone(),
        commands: manifest.commands.clone(),
        installed: InstalledRecord {
            source_url: entry.url.clone(),
            sha256: entry.sha256.clone(),
            installed_at: chrono::Utc::now().to_rfc3339(),
            dyyl_version: env!("CARGO_PKG_VERSION").to_string(),
        },
    };
    let toml_content = toml::to_string_pretty(&local_toml).unwrap_or_default();
    fs::write(&toml_path, toml_content).map_err(|e| format!("{e}"))?;

    Ok(manifest.version)
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
