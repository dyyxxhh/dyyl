//! Parse plugin manifests (remote JSON + local TOML).

use serde::{Deserialize, Serialize};

/// Remote manifest fetched from `l.dyyapp.com/plugins/<name>/manifest.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteManifest {
    pub name: String,
    pub version: String,
    pub abi_version: u32,
    pub dyyl_min: String,
    #[serde(default = "default_panic_mode")]
    pub panic_mode: String,
    #[serde(default)]
    pub commands: Vec<PluginCommand>,
    pub platforms: Vec<PlatformEntry>,
    /// Optional: indicates plugin ships locales/ directory.
    #[serde(default)]
    pub has_locales: bool,
    /// 可选：插件声明需要的凭证字段。
    #[serde(default)]
    pub credentials: Option<CredentialsSpec>,
}

fn default_panic_mode() -> String {
    "abort".to_string()
}

/// A command exposed by a plugin. `name` may contain dots (e.g. "user.login").
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginCommand {
    pub name: String,
    pub arity: usize,
    #[serde(default)]
    pub brief: String,
}

/// A platform-specific build entry in the manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformEntry {
    pub platform: String,
    pub url: String,
    pub sha256: String,
}

/// Local `plugin.toml` stored alongside the library after installation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalPluginToml {
    pub name: String,
    pub version: String,
    pub abi_version: u32,
    pub dyyl_min: String,
    pub panic_mode: String,
    #[serde(default)]
    pub commands: Vec<PluginCommand>,
    pub installed: InstalledRecord,
    /// 可选：插件声明需要的凭证字段（从远程 manifest 透传到本地）。
    #[serde(default)]
    pub credentials: Option<CredentialsSpec>,
}

/// Installation metadata in `plugin.toml`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledRecord {
    pub source_url: String,
    pub sha256: String,
    pub installed_at: String,
    pub dyyl_version: String,
}

/// 远程 manifest 中的插件凭证声明。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CredentialsSpec {
    #[serde(default)]
    pub fields: Vec<CredentialField>,
}

/// 单个凭证字段声明。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CredentialField {
    pub name: String,
    #[serde(default = "default_field_type")]
    pub r#type: String,
    #[serde(default)]
    pub secret: bool,
    #[serde(default)]
    pub description: String,
}

fn default_field_type() -> String {
    "string".to_string()
}

impl LocalPluginToml {
    /// Find a command by name (exact match, names may contain dots).
    #[must_use]
    pub fn find_command(&self, name: &str) -> Option<&PluginCommand> {
        self.commands.iter().find(|c| c.name == name)
    }
}
