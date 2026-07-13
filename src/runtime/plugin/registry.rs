//! Scan installed plugins directory.
//!
//! Walks `<xdg_data>/dyyl/plugins/<name>/<version>/` and collects
//! `InstalledPlugin` records (one per version directory that has a
//! `plugin.toml`).

use std::fs;
use std::path::PathBuf;

use crate::runtime::plugin::store;

/// A scanned installed plugin (one per version directory).
#[derive(Debug, Clone)]
pub struct InstalledPlugin {
    pub name: String,
    pub version: String,
    pub toml_path: PathBuf,
    pub lib_path: PathBuf,
}

/// Scan the plugins directory and return all installed plugins.
///
/// Returns an error only if the plugins directory exists but cannot be read.
/// If the directory doesn't exist, returns an empty Vec.
pub fn scan_installed() -> Result<Vec<InstalledPlugin>, String> {
    let plugins_dir = store::plugin_dir();
    if !plugins_dir.exists() {
        return Ok(Vec::new());
    }
    let mut result = Vec::new();
    let name_entries = fs::read_dir(&plugins_dir)
        .map_err(|e| format!("failed to read {}: {e}", plugins_dir.display()))?;
    for name_entry in name_entries.flatten() {
        let name_path = name_entry.path();
        if !name_path.is_dir() {
            continue;
        }
        let name = match name_entry.file_name().to_str() {
            Some(n) => n.to_string(),
            None => continue,
        };
        let version_entries = match fs::read_dir(&name_path) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for version_entry in version_entries.flatten() {
            let version_path = version_entry.path();
            if !version_path.is_dir() {
                continue;
            }
            let version = match version_entry.file_name().to_str() {
                Some(v) => v.to_string(),
                None => continue,
            };
            let toml_path = store::plugin_toml_path(&name, &version);
            let lib_path = store::lib_path(&name, &version);
            if toml_path.exists() && lib_path.exists() {
                result.push(InstalledPlugin {
                    name: name.clone(),
                    version: version.clone(),
                    toml_path,
                    lib_path,
                });
            }
        }
    }
    Ok(result)
}

/// Find the installed version of a plugin by name.
///
/// Returns the most recent (lexicographically last) version if multiple
/// are installed, or `None` if not installed.
#[must_use]
pub fn find_installed(name: &str) -> Option<InstalledPlugin> {
    let plugins = scan_installed().ok()?;
    plugins
        .into_iter()
        .filter(|p| p.name == name)
        .max_by_key(|p| p.version.clone())
}
