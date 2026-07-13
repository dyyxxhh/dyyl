//! credentials.toml 读写 — AI 凭证 + 插件凭证。
//!
//! 文件位于 ~/.config/dyyl/credentials.toml（与 config.toml 同目录）。
//! 不自动 chmod 修正权限；--debug 时权限过松仅警告。

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

/// AI Provider 类型。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AiProviderKind {
    OpenaiChat,
    OpenaiResponse,
    Anthropic,
}

impl AiProviderKind {
    /// 从用户选择数字解析（1/2/3）。
    #[must_use]
    pub fn from_choice(n: u8) -> Option<Self> {
        match n {
            1 => Some(Self::OpenaiChat),
            2 => Some(Self::OpenaiResponse),
            3 => Some(Self::Anthropic),
            _ => None,
        }
    }
}

/// dyyl 内置 AI 凭证（credentials.toml 的 [ai] 段）。
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AiCredentials {
    pub provider: AiProviderKind,
    pub api_key: String,
    pub model: String,
    #[serde(default)]
    pub base_url: String,
    #[serde(default)]
    pub auto_system_prompt: String,
}

/// credentials.toml 的完整结构。
#[derive(Default, Debug, Clone)]
pub struct CredentialsFile {
    /// dyyl 内置 AI 凭证。None = 未配置或字段不完整。
    pub ai: Option<AiCredentials>,
    /// 插件凭证：plugin_name -> (field_name -> value)。
    pub plugins: HashMap<String, HashMap<String, String>>,
}

/// TOML 反序列化的中间结构（[ai] 段可能字段不全）。
#[derive(Deserialize, Default)]
struct RawCredentials {
    #[serde(default)]
    ai: Option<RawAi>,
    #[serde(default)]
    plugin: HashMap<String, HashMap<String, String>>,
}

#[derive(Deserialize, Default)]
struct RawAi {
    provider: Option<String>,
    api_key: Option<String>,
    model: Option<String>,
    #[serde(default)]
    base_url: String,
    #[serde(default)]
    auto_system_prompt: String,
}

impl CredentialsFile {
    /// 从文件加载。文件不存在返回空默认值。
    pub fn load(path: &Path) -> Result<Self, String> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = fs::read_to_string(path)
            .map_err(|e| format!("failed to read credentials at {}: {e}", path.display()))?;
        let raw: RawCredentials = toml::from_str(&content)
            .map_err(|e| format!("failed to parse credentials at {}: {e}", path.display()))?;
        let ai = raw.ai.and_then(|r| {
            let provider_str = r.provider?;
            let provider = match provider_str.as_str() {
                "openai-chat" => AiProviderKind::OpenaiChat,
                "openai-response" => AiProviderKind::OpenaiResponse,
                "anthropic" => AiProviderKind::Anthropic,
                _ => return None,
            };
            Some(AiCredentials {
                provider,
                api_key: r.api_key?,
                model: r.model?,
                base_url: r.base_url,
                auto_system_prompt: r.auto_system_prompt,
            })
        });
        Ok(Self {
            ai,
            plugins: raw.plugin,
        })
    }

    /// 保存到文件。
    pub fn save(&self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                format!("failed to create credentials dir {}: {e}", parent.display())
            })?;
        }
        let mut content = String::new();
        if let Some(ai) = &self.ai {
            content.push_str("[ai]\n");
            content.push_str(&format!(
                "provider = \"{}\"\n",
                match ai.provider {
                    AiProviderKind::OpenaiChat => "openai-chat",
                    AiProviderKind::OpenaiResponse => "openai-response",
                    AiProviderKind::Anthropic => "anthropic",
                }
            ));
            content.push_str(&format!("api_key = \"{}\"\n", escape_toml(&ai.api_key)));
            content.push_str(&format!("model = \"{}\"\n", escape_toml(&ai.model)));
            if !ai.base_url.is_empty() {
                content.push_str(&format!("base_url = \"{}\"\n", escape_toml(&ai.base_url)));
            }
            if !ai.auto_system_prompt.is_empty() {
                content.push_str(&format!(
                    "auto_system_prompt = \"{}\"\n",
                    escape_toml(&ai.auto_system_prompt)
                ));
            }
            content.push('\n');
        }
        for (plugin_name, fields) in &self.plugins {
            content.push_str(&format!("[plugin.{plugin_name}]\n"));
            for (k, v) in fields {
                content.push_str(&format!("{k} = \"{}\"\n", escape_toml(v)));
            }
            content.push('\n');
        }
        fs::write(path, content)
            .map_err(|e| format!("failed to write credentials to {}: {e}", path.display()))?;
        Ok(())
    }

    /// 返回 credentials.toml 的默认路径（~/.config/dyyl/credentials.toml）。
    #[must_use]
    pub fn default_path() -> Option<std::path::PathBuf> {
        let proj = directories::ProjectDirs::from("dev", "lucky", "dyyl")?;
        Some(proj.config_dir().join("credentials.toml"))
    }
}

/// 转义 TOML 字符串值（处理引号、反斜杠、换行）。
fn escape_toml(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}
