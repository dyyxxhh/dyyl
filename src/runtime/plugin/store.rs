//! Plugin storage path management (XDG data).
//!
//! Plugins are stored under `<xdg_data>/dyyl/plugins/<name>/<version>/`.
//! The library file is `lib<name>.so` (linux), `lib<name>.dylib` (macos),
//! or `<name>.dll` (windows). A `plugin.toml` sits alongside.

use std::path::PathBuf;

/// Return the base plugins directory: `<xdg_data>/dyyl/plugins/`.
///
/// Returns `None` if the XDG data directory cannot be determined.
#[must_use]
pub fn plugin_dir() -> PathBuf {
    let proj = directories::ProjectDirs::from("dev", "lucky", "dyyl")
        .expect("unable to determine XDG data directory");
    proj.data_dir().join("plugins")
}

/// Return the directory for a specific plugin version:
/// `<plugin_dir>/<name>/<version>/`.
#[must_use]
pub fn plugin_version_dir(name: &str, version: &str) -> PathBuf {
    plugin_dir().join(name).join(version)
}

/// Return the path to the plugin library file.
///
/// Filename: `lib<name>.so` (linux), `lib<name>.dylib` (macos), `<name>.dll` (windows).
#[must_use]
pub fn lib_path(name: &str, version: &str) -> PathBuf {
    let filename = lib_filename(name);
    plugin_version_dir(name, version).join(filename)
}

/// Return the path to the plugin's `plugin.toml` metadata file.
#[must_use]
pub fn plugin_toml_path(name: &str, version: &str) -> PathBuf {
    plugin_version_dir(name, version).join("plugin.toml")
}

/// Return the locales directory for a plugin.
#[must_use]
pub fn plugin_locales_dir(name: &str, version: &str) -> PathBuf {
    plugin_version_dir(name, version).join("locales")
}

/// Compute the platform-appropriate library filename for a plugin name.
#[must_use]
fn lib_filename(name: &str) -> String {
    if cfg!(target_os = "windows") {
        format!("{name}.dll")
    } else if cfg!(target_os = "macos") {
        format!("lib{name}.dylib")
    } else {
        format!("lib{name}.so")
    }
}

/// Return the current platform identifier used in manifests.
#[must_use]
pub fn current_platform() -> String {
    let os = if cfg!(target_os = "linux") {
        "linux"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else {
        "unknown"
    };
    let arch = if cfg!(target_arch = "x86_64") {
        "x86_64"
    } else if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else {
        "unknown"
    };
    format!("{os}-{arch}")
}
