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

use crate::i18n::{self, Lang};

/// 从预读的输入行列表解析 AI 凭证。
///
/// 行顺序：provider 选择（1/2/3）、api_key、model、base_url（空行=默认）。
/// 返回 (凭证, 消耗的行数)。供测试与实际提示流程共用。
pub fn prompt_ai_from_lines(lines: &[String]) -> Result<(AiCredentials, usize), String> {
    if lines.len() < 4 {
        return Err("not enough input lines for AI credential prompt".to_string());
    }
    let provider_raw = lines.first().map(String::as_str).unwrap_or_default();
    let choice: u8 = provider_raw
        .trim()
        .parse()
        .map_err(|_| format!("invalid provider choice: '{provider_raw}'"))?;
    let provider = AiProviderKind::from_choice(choice)
        .ok_or_else(|| format!("invalid provider choice: {choice}"))?;
    let api_key = lines
        .get(1)
        .map(|s| s.trim().to_string())
        .unwrap_or_default();
    if api_key.is_empty() {
        return Err("api_key cannot be empty".to_string());
    }
    let model = lines
        .get(2)
        .map(|s| s.trim().to_string())
        .unwrap_or_default();
    if model.is_empty() {
        return Err("model cannot be empty".to_string());
    }
    let base_url = lines
        .get(3)
        .map(|s| s.trim().to_string())
        .unwrap_or_default();
    Ok((
        AiCredentials {
            provider,
            api_key,
            model,
            base_url,
            auto_system_prompt: String::new(),
        },
        4,
    ))
}

/// 交互式提示用户输入 AI 凭证并保存到 credentials.toml。
///
/// 从 stdin 读取，stderr 输出问题。返回加载后的 AiCredentials。
/// stdin EOF → 返回 Err（调用方中止，退出码 3）。
pub fn ensure_ai_interactive(path: &Path, lang: Lang) -> Result<AiCredentials, String> {
    use std::io::{BufRead, Write};
    eprintln!("{}", i18n::t(lang, "ai.credential_prompt_header", &[]));
    eprintln!("  {}", i18n::t(lang, "ai.credential_prompt_provider", &[]));
    let stdin = std::io::stdin();
    let mut lines = stdin.lock().lines();
    let provider_line = lines
        .next()
        .ok_or_else(|| "credential input aborted".to_string())?
        .map_err(|e| format!("stdin read error: {e}"))?;
    eprint!("  API Key: ");
    let _ = std::io::stderr().flush();
    let api_key_line = lines
        .next()
        .ok_or_else(|| "credential input aborted".to_string())?
        .map_err(|e| format!("stdin read error: {e}"))?;
    eprint!("  Model: ");
    let _ = std::io::stderr().flush();
    let model_line = lines
        .next()
        .ok_or_else(|| "credential input aborted".to_string())?
        .map_err(|e| format!("stdin read error: {e}"))?;
    eprint!("  Base URL (空=官方端点): ");
    let _ = std::io::stderr().flush();
    let base_url_line = lines
        .next()
        .ok_or_else(|| "credential input aborted".to_string())?
        .map_err(|e| format!("stdin read error: {e}"))?;
    let inputs = vec![provider_line, api_key_line, model_line, base_url_line];
    let (creds, _) = prompt_ai_from_lines(&inputs)?;
    let mut file = CredentialsFile::load(path)?;
    file.ai = Some(creds.clone());
    file.save(path)?;
    eprintln!(
        "{}",
        i18n::t(
            lang,
            "ai.credential_saved",
            &[("path", &path.display().to_string())]
        )
    );
    Ok(creds)
}

/// 加载 AI 凭证；若缺失或不完整则交互式提示。
pub fn ensure_ai(path: &Path, lang: Lang) -> Result<AiCredentials, String> {
    let file = CredentialsFile::load(path)?;
    if let Some(ai) = file.ai {
        return Ok(ai);
    }
    ensure_ai_interactive(path, lang)
}

/// Return the per-plugin credentials directory under XDG data:
/// `<xdg_data>/dyyl/credentials.d/<plugin_name>/`.
///
/// Used for `type: "file"` and `type: "directory"` credential fields
/// that may hold large or dynamic blobs (e.g. `OpenPGP` keys).
/// Does NOT create the directory; callers create on demand with 0700.
///
/// # Panics
///
/// Panics if the XDG data directory cannot be determined (e.g. no valid home).
#[must_use]
pub fn credentials_dir_for_plugin(plugin_name: &str) -> std::path::PathBuf {
    let proj = directories::ProjectDirs::from("dev", "lucky", "dyyl")
        .expect("unable to determine XDG data directory");
    proj.data_dir()
        .join("credentials.d")
        .join(plugin_name)
}
