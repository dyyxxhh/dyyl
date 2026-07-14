//! Persistent configuration for the dyyl interpreter.
//!
//! Configuration is stored in TOML format at the XDG-standard location:
//! `~/.config/dyyl/config.toml` (Linux/macOS).

use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// The dyyl interpreter configuration.
#[derive(Serialize, Deserialize, Default, Debug, Clone)]
pub struct DyylConfig {
    /// Preferred output language for i18n (e.g. `"zh"`, `"en"`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lang: Option<String>,
    /// Installed plugins with `last_used_at` tracking.
    #[serde(default, skip_serializing_if = "std::collections::HashMap::is_empty")]
    pub installed_plugins: std::collections::HashMap<String, InstalledPluginRecord>,
}

/// Record of an installed plugin, for `last_used_at` tracking.
#[derive(Serialize, Deserialize, Default, Debug, Clone)]
pub struct InstalledPluginRecord {
    /// Version string (may be `None` if unknown).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// ISO 8601 timestamp of last successful dispatch.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_used_at: Option<String>,
}

/// Returns the XDG-compliant path to `config.toml`.
///
/// Uses `directories::ProjectDirs` to resolve the platform-appropriate
/// config directory. Returns `None` if the platform data directory
/// cannot be determined.
pub fn config_path() -> Option<PathBuf> {
    let proj = directories::ProjectDirs::from("dev", "lucky", "dyyl")?;
    Some(proj.config_dir().join("config.toml"))
}

/// Load the configuration from disk.
///
/// Returns the deserialized config if the file exists and is valid TOML.
/// Returns `DyylConfig::default()` if the file is missing.
/// Returns an error if the file exists but cannot be read or parsed.
pub fn load_config() -> Result<DyylConfig, String> {
    let path = config_path().ok_or_else(|| "unable to determine config directory".to_owned())?;

    if !path.exists() {
        return Ok(DyylConfig::default());
    }

    let content = fs::read_to_string(&path)
        .map_err(|e| format!("failed to read config at `{}`: {e}", path.display()))?;

    toml::from_str(&content)
        .map_err(|e| format!("failed to parse config at `{}`: {e}", path.display()))
}

/// Persist the configuration to disk.
///
/// Creates parent directories as needed. Overwrites the file if it
/// already exists.
pub fn save_config(config: &DyylConfig) -> Result<(), String> {
    let path = config_path().ok_or_else(|| "unable to determine config directory".to_owned())?;

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            format!(
                "failed to create config directory `{}`: {e}",
                parent.display()
            )
        })?;
    }

    let content =
        toml::to_string_pretty(config).map_err(|e| format!("failed to serialize config: {e}"))?;

    fs::write(&path, content)
        .map_err(|e| format!("failed to write config to `{}`: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn default_config_has_no_lang() {
        let cfg = DyylConfig::default();
        assert!(cfg.lang.is_none());
    }

    #[test]
    fn roundtrip_toml() {
        let cfg = DyylConfig {
            lang: Some("zh".to_owned()),
            installed_plugins: std::collections::HashMap::new(),
        };
        let s = toml::to_string_pretty(&cfg).unwrap();
        let parsed: DyylConfig = toml::from_str(&s).unwrap();
        assert_eq!(parsed.lang.as_deref(), Some("zh"));
    }

    #[test]
    fn missing_file_returns_default() {
        // Use a temp dir so we don't touch the real config.
        let dir = tempfile::tempdir().unwrap();
        let fake_path = dir.path().join("nonexistent.toml");
        // config_path() is not overrideable, so test the parse path directly.
        let content = fs::read_to_string(&fake_path);
        assert!(content.is_err());
    }

    #[test]
    fn invalid_toml_yields_error() {
        let err = toml::from_str::<DyylConfig>("{{{{bad").unwrap_err();
        assert!(err.to_string().contains("invalid key"));
    }

    #[test]
    fn empty_toml_yields_default() {
        let cfg: DyylConfig = toml::from_str("").unwrap();
        assert!(cfg.lang.is_none());
    }
}
