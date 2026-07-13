//! Plugin CLI subcommands: install, update, remove, autoremove, list.

use crate::i18n::Lang;

/// Plugins unused for this many days are removed by `autoremove`.
const AUTOREMOVE_DAYS: i64 = 30;

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
    let name = match args.first() {
        Some(n) => n.as_str(),
        None => {
            eprintln!("{}", crate::i18n::cli_plugin_usage(lang));
            return 1;
        }
    };

    // Check if already installed with same version.
    if let Some(existing) = crate::runtime::plugin::registry::find_installed(name) {
        let toml_content = match std::fs::read_to_string(&existing.toml_path) {
            Ok(s) => s,
            Err(_) => String::new(),
        };
        if let Ok(t) =
            toml::from_str::<crate::runtime::plugin::manifest::LocalPluginToml>(&toml_content)
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
    use crate::runtime::plugin::{abi::DYRL_API_VERSION, fetch, manifest::*, store};
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

    let bytes =
        fetch::download_and_verify(&entry.url, &entry.sha256).map_err(|e| format!("{e}"))?;

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
    if args.is_empty() {
        // Update all installed plugins.
        let installed = match crate::runtime::plugin::registry::scan_installed() {
            Ok(p) => p,
            Err(e) => {
                eprintln!("{e}");
                return 1;
            }
        };
        let mut updated = 0usize;
        let mut latest = 0usize;
        let mut failed = 0usize;
        for p in &installed {
            match update_single(&p.name, lang) {
                UpdateOutcome::Updated => updated += 1,
                UpdateOutcome::AlreadyLatest => latest += 1,
                UpdateOutcome::Failed(_) => failed += 1,
            }
        }
        println!(
            "{}",
            crate::i18n::plugin_update_all_summary(lang, updated, latest, failed)
        );
        if failed > 0 {
            1
        } else {
            0
        }
    } else {
        let name = match args.first() {
            Some(n) => n.as_str(),
            None => {
                eprintln!("{}", crate::i18n::cli_plugin_usage(lang));
                return 1;
            }
        };
        if crate::runtime::plugin::registry::find_installed(name).is_none() {
            eprintln!("{}", crate::i18n::plugin_not_installed(lang, name));
            return 1;
        }
        match update_single(name, lang) {
            UpdateOutcome::Updated => 0,
            UpdateOutcome::AlreadyLatest => 0,
            UpdateOutcome::Failed(e) => {
                eprintln!("{}", crate::i18n::plugin_update_failed(lang, name, &e));
                1
            }
        }
    }
}

enum UpdateOutcome {
    Updated,
    AlreadyLatest,
    Failed(String),
}

fn update_single(name: &str, lang: Lang) -> UpdateOutcome {
    use crate::runtime::plugin::{fetch, manifest::*, registry, store};
    use std::fs;

    // Get current installed version.
    let current = match registry::find_installed(name) {
        Some(p) => p,
        None => return UpdateOutcome::Failed("not installed".to_string()),
    };
    let toml_content = match fs::read_to_string(&current.toml_path) {
        Ok(s) => s,
        Err(e) => return UpdateOutcome::Failed(e.to_string()),
    };
    let local: LocalPluginToml = match toml::from_str(&toml_content) {
        Ok(t) => t,
        Err(e) => return UpdateOutcome::Failed(e.to_string()),
    };

    // Fetch remote manifest.
    let remote = match fetch::fetch_manifest(name) {
        Ok(m) => m,
        Err(e) => return UpdateOutcome::Failed(e.to_string()),
    };

    if remote.version == local.version {
        return UpdateOutcome::AlreadyLatest;
    }

    // Download + install new version.
    let cur_platform = store::current_platform();
    let entry = match remote.platforms.iter().find(|p| p.platform == cur_platform) {
        Some(e) => e,
        None => {
            return UpdateOutcome::Failed(format!("no build for {cur_platform}"));
        }
    };

    let bytes = match fetch::download_and_verify(&entry.url, &entry.sha256) {
        Ok(b) => b,
        Err(e) => return UpdateOutcome::Failed(e.to_string()),
    };

    let new_lib_path = store::lib_path(name, &remote.version);
    let new_toml_path = store::plugin_toml_path(name, &remote.version);
    let new_version_dir = store::plugin_version_dir(name, &remote.version);

    if let Err(e) = fs::create_dir_all(&new_version_dir) {
        return UpdateOutcome::Failed(e.to_string());
    }
    if let Err(e) = fs::write(&new_lib_path, &bytes) {
        return UpdateOutcome::Failed(e.to_string());
    }

    let new_local = LocalPluginToml {
        name: remote.name.clone(),
        version: remote.version.clone(),
        abi_version: remote.abi_version,
        dyyl_min: remote.dyyl_min.clone(),
        panic_mode: remote.panic_mode.clone(),
        commands: remote.commands.clone(),
        installed: InstalledRecord {
            source_url: entry.url.clone(),
            sha256: entry.sha256.clone(),
            installed_at: chrono::Utc::now().to_rfc3339(),
            dyyl_version: env!("CARGO_PKG_VERSION").to_string(),
        },
    };
    let toml_content = toml::to_string_pretty(&new_local).unwrap_or_default();
    if let Err(e) = fs::write(&new_toml_path, toml_content) {
        return UpdateOutcome::Failed(e.to_string());
    }

    // Remove old version directory.
    let old_version_dir = store::plugin_version_dir(name, &local.version);
    let _ = fs::remove_dir_all(&old_version_dir);

    println!(
        "{}",
        crate::i18n::plugin_updated(lang, name, &local.version, &remote.version)
    );
    UpdateOutcome::Updated
}

fn cmd_remove(args: &[String], lang: Lang) -> i32 {
    let name = match args.first() {
        Some(n) => n.as_str(),
        None => {
            eprintln!("{}", crate::i18n::cli_plugin_usage(lang));
            return 1;
        }
    };

    // Check if installed.
    if crate::runtime::plugin::registry::find_installed(name).is_none() {
        eprintln!("{}", crate::i18n::plugin_not_installed(lang, name));
        return 1;
    }

    // Remove the entire plugin directory (all versions).
    let plugin_dir = crate::runtime::plugin::store::plugin_dir().join(name);
    if let Err(e) = std::fs::remove_dir_all(&plugin_dir) {
        eprintln!(
            "{}",
            crate::i18n::plugin_remove_failed(lang, name, &e.to_string())
        );
        return 1;
    }

    // Remove from config.
    if let Ok(mut config) = crate::config::load_config() {
        config.installed_plugins.remove(name);
        let _ = crate::config::save_config(&config);
    }

    println!("{}", crate::i18n::plugin_removed(lang, name));
    0
}

fn cmd_autoremove(args: &[String], lang: Lang) -> i32 {
    let _ = args;
    let installed = match crate::runtime::plugin::registry::scan_installed() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("{e}");
            return 1;
        }
    };
    let mut config = crate::config::load_config().unwrap_or_default();
    let now = chrono::Utc::now();
    let mut removed_count = 0usize;

    for p in &installed {
        let last_used = config
            .installed_plugins
            .get(&p.name)
            .and_then(|r| r.last_used_at.as_ref())
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&chrono::Utc));

        let days_ago: i64 = match last_used {
            Some(dt) => (now - dt).num_days(),
            None => i64::MAX, // Never used — remove.
        };

        if days_ago >= AUTOREMOVE_DAYS {
            let plugin_dir = crate::runtime::plugin::store::plugin_dir().join(&p.name);
            if std::fs::remove_dir_all(&plugin_dir).is_ok() {
                config.installed_plugins.remove(&p.name);
                if days_ago == i64::MAX {
                    println!("{}", crate::i18n::plugin_removed(lang, &p.name));
                } else {
                    println!(
                        "{}",
                        crate::i18n::plugin_autoremove_removed(lang, &p.name, days_ago as u64)
                    );
                }
                removed_count += 1;
            }
        }
    }

    if removed_count > 0 {
        let _ = crate::config::save_config(&config);
    }
    println!(
        "{}",
        crate::i18n::plugin_autoremove_summary(lang, removed_count)
    );
    0
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
        let toml: crate::runtime::plugin::manifest::LocalPluginToml =
            match toml::from_str(&toml_content) {
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
