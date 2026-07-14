//! Plugin system — dynamic-library loading and dispatch.
//!
//! Plugins are compiled dynamic libraries (.so/.dll/.dylib) loaded via dlopen.
//! The `PluginManager` orchestrates fetch → install → load → dispatch.
//! On first call to `<name>.<sub>`, the manager fetches the manifest from
//! l.dyyapp.com, downloads+verifies the library, dlopens it, and dispatches.

pub mod abi;
pub mod creds_inject;
pub mod fetch;
pub mod loader;
pub mod manifest;
pub mod registry;
pub mod store;
pub mod value_codec;

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

use crate::i18n::Lang;
use crate::runtime::error::RuntimeError;
use crate::runtime::plugin::abi::DYRL_API_VERSION;
use crate::runtime::plugin::fetch::FetchError;
use crate::runtime::plugin::loader::PluginLoader;
use crate::runtime::plugin::manifest::{InstalledRecord, LocalPluginToml, RemoteManifest};
use crate::runtime::plugin::store as plugin_store;
use crate::runtime::plugin::value_codec::{value_from_json, values_to_json_array};
use crate::runtime::value::Value;

/// Loaded plugin instance — holds the loader and manifest.
#[derive(Debug)]
pub struct LoadedPlugin {
    /// The name of the plugin (e.g. "migpt").
    pub name: String,
    /// The dlopen'd library + resolved symbols.
    pub loader: PluginLoader,
    /// The parsed local `plugin.toml`.
    pub manifest: LocalPluginToml,
}

/// Central plugin manager — holds already-loaded plugins.
#[derive(Debug)]
pub struct PluginManager {
    loaded: Mutex<HashMap<String, LoadedPlugin>>,
}

impl PluginManager {
    /// Create a new empty plugin manager.
    #[must_use]
    pub fn new() -> Self {
        Self {
            loaded: Mutex::new(HashMap::new()),
        }
    }

    /// Dispatch a plugin command `<name>.<sub>` (sub may contain dots).
    ///
    /// If the plugin isn't loaded yet, this fetches+installs+loads it first.
    /// Returns the plugin's result Value or a RuntimeError.
    pub fn dispatch(
        &self,
        name: &str,
        sub: &str,
        args: &[Value],
        lang: Lang,
        line: usize,
    ) -> Result<Value, RuntimeError> {
        // 1. Ensure plugin is loaded (lock, insert if missing, drop guard).
        {
            let mut loaded = self.loaded.lock().expect("plugin map mutex poisoned");
            if !loaded.contains_key(name) {
                let lp = self.load_plugin(name, lang, line)?;
                loaded.insert(name.to_string(), lp);
            }
        }

        // 2. Re-lock to find the plugin and dispatch. The guard is held for the
        //    duration of the FFI call; the plugin cannot re-enter the manager
        //    (different language boundary), so no deadlock risk.
        let loaded = self.loaded.lock().expect("plugin map mutex poisoned");
        let lp = loaded.get(name).ok_or_else(|| {
            RuntimeError::new(
                line,
                format!("{name}.{sub}"),
                crate::i18n::plugin_command_failed(lang, name, sub, "load_failed"),
            )
        })?;

        // 3. Verify command exists in manifest.
        if lp.manifest.find_command(sub).is_none() {
            return Err(RuntimeError::new(
                line,
                format!("{name}.{sub}"),
                crate::i18n::plugin_unknown_subcommand(lang, name, sub),
            ));
        }

        // 4. Encode args, call handle_command, decode result.
        let args_json = values_to_json_array(args);
        let result_json = lp.loader.handle_command(sub, &args_json).map_err(|e| {
            RuntimeError::new(
                line,
                format!("{name}.{sub}"),
                crate::i18n::plugin_command_failed(lang, name, sub, &e.to_string()),
            )
        })?;

        value_from_json(&result_json).map_err(|e| {
            RuntimeError::new(
                line,
                format!("{name}.{sub}"),
                crate::i18n::plugin_command_failed(lang, name, sub, &e.to_string()),
            )
        })
    }

    /// Load (or install+load) a plugin by name.
    fn load_plugin(
        &self,
        name: &str,
        lang: Lang,
        line: usize,
    ) -> Result<LoadedPlugin, RuntimeError> {
        // 1. Check if already installed locally.
        let installed = registry::find_installed(name);

        let lib_path = match installed {
            Some(rec) => rec.lib_path,
            None => {
                // 2. Not installed — fetch manifest, download, install.
                self.install_plugin(name, lang, line)?
            }
        };

        // 3. Read local plugin.toml.
        let version = self.read_installed_version(name).map_err(|msg| {
            RuntimeError::new(
                line,
                name,
                crate::i18n::plugin_dlopen_failed(lang, name, &msg),
            )
        })?;
        let toml_path = plugin_store::plugin_toml_path(name, &version);
        let toml_content = fs::read_to_string(&toml_path).map_err(|e| {
            RuntimeError::new(
                line,
                name,
                crate::i18n::plugin_dlopen_failed(lang, name, &e.to_string()),
            )
        })?;
        let manifest: LocalPluginToml = toml::from_str(&toml_content).map_err(|e| {
            RuntimeError::new(
                line,
                name,
                crate::i18n::plugin_dlopen_failed(lang, name, &e.to_string()),
            )
        })?;

        // 4. dlopen + init + on_load.
        let loader = PluginLoader::load(&lib_path, name, None).map_err(|e| {
            RuntimeError::new(
                line,
                name,
                crate::i18n::plugin_dlopen_failed(lang, name, &e.to_string()),
            )
        })?;

        Ok(LoadedPlugin {
            name: name.to_string(),
            loader,
            manifest,
        })
    }

    /// Fetch manifest, download library, verify SHA256, install to XDG dir.
    fn install_plugin(&self, name: &str, lang: Lang, line: usize) -> Result<PathBuf, RuntimeError> {
        // 1. Fetch manifest.
        let manifest = fetch::fetch_manifest(name).map_err(|e| {
            let reason = match e {
                FetchError::Http(_, msg) | FetchError::Read(_, msg) | FetchError::Parse(_, msg) => {
                    msg
                }
                FetchError::ChecksumMismatch(_) => "checksum mismatch".to_string(),
            };
            RuntimeError::new(
                line,
                name,
                crate::i18n::plugin_fetch_manifest_failed(lang, name, &reason),
            )
        })?;

        // 2. Validate ABI version.
        if manifest.abi_version != 1 && manifest.abi_version != DYRL_API_VERSION {
            return Err(RuntimeError::new(
                line,
                name,
                crate::i18n::plugin_abi_mismatch(
                    lang,
                    name,
                    DYRL_API_VERSION,
                    manifest.abi_version,
                ),
            ));
        }

        // 3. Find platform entry.
        let current = plugin_store::current_platform();
        let entry = manifest
            .platforms
            .iter()
            .find(|p| p.platform == current)
            .ok_or_else(|| {
                let available: Vec<String> = manifest
                    .platforms
                    .iter()
                    .map(|p| p.platform.clone())
                    .collect();
                RuntimeError::new(
                    line,
                    name,
                    crate::i18n::plugin_platform_unavailable(
                        lang,
                        name,
                        &current,
                        &available.join(", "),
                    ),
                )
            })?;

        // 4. Download + verify SHA256.
        let bytes = fetch::download_and_verify(&entry.url, &entry.sha256).map_err(|e| match e {
            FetchError::ChecksumMismatch(_) => {
                RuntimeError::new(line, name, crate::i18n::plugin_sha256_mismatch(lang, name))
            }
            FetchError::Http(_, msg) | FetchError::Read(_, msg) | FetchError::Parse(_, msg) => {
                RuntimeError::new(
                    line,
                    name,
                    crate::i18n::plugin_download_failed(lang, name, &msg),
                )
            }
        })?;

        // 5. Install: create dir, write lib + plugin.toml.
        let lib_path = plugin_store::lib_path(name, &manifest.version);
        let toml_path = plugin_store::plugin_toml_path(name, &manifest.version);
        let version_dir = plugin_store::plugin_version_dir(name, &manifest.version);

        fs::create_dir_all(&version_dir).map_err(|e| {
            RuntimeError::new(
                line,
                name,
                crate::i18n::plugin_install_failed(lang, name, &e.to_string()),
            )
        })?;

        fs::write(&lib_path, &bytes).map_err(|e| {
            RuntimeError::new(
                line,
                name,
                crate::i18n::plugin_install_failed(lang, name, &e.to_string()),
            )
        })?;

        // 6. Write plugin.toml.
        let local_toml = build_local_toml(&manifest, &entry.url, &entry.sha256);
        let toml_content = toml::to_string_pretty(&local_toml).unwrap_or_default();
        fs::write(&toml_path, toml_content).map_err(|e| {
            RuntimeError::new(
                line,
                name,
                crate::i18n::plugin_install_failed(lang, name, &e.to_string()),
            )
        })?;

        Ok(lib_path)
    }

    /// Read the installed version of a plugin from its `plugin.toml`.
    fn read_installed_version(&self, name: &str) -> Result<String, String> {
        let rec =
            registry::find_installed(name).ok_or_else(|| format!("plugin {name} not installed"))?;
        let content = fs::read_to_string(&rec.toml_path)
            .map_err(|e| format!("read {}: {e}", rec.toml_path.display()))?;
        let toml: LocalPluginToml = toml::from_str(&content)
            .map_err(|e| format!("parse {}: {e}", rec.toml_path.display()))?;
        Ok(toml.version)
    }
}

impl Default for PluginManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Build a `LocalPluginToml` from a `RemoteManifest` + install metadata.
fn build_local_toml(manifest: &RemoteManifest, source_url: &str, sha256: &str) -> LocalPluginToml {
    LocalPluginToml {
        name: manifest.name.clone(),
        version: manifest.version.clone(),
        abi_version: manifest.abi_version,
        dyyl_min: manifest.dyyl_min.clone(),
        panic_mode: manifest.panic_mode.clone(),
        commands: manifest.commands.clone(),
        credentials: manifest.credentials.clone(),
        installed: InstalledRecord {
            source_url: source_url.to_string(),
            sha256: sha256.to_string(),
            installed_at: chrono::Utc::now().to_rfc3339(),
            dyyl_version: env!("CARGO_PKG_VERSION").to_string(),
        },
    }
}
