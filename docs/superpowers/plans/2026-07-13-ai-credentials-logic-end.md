# AI 集成、凭证系统与 logic.end 实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在 dyyl 中加入 `ai.ask`/`ai.auto` 命令、统一凭证系统（AI + 插件）、`logic.end` 开放块语法，以及 ABI v2 的 `set_credentials` 注入。

**Architecture:** 预扫描 + 运行时分离方案。执行前预扫描检测未填的 `ai.auto` 占位符，批量请求 AI 填值并回写源码；运行时 `ai.auto.filled` 仅取值、`ai.ask` 同步 HTTP 请求。凭证存 `~/.config/dyyl/credentials.toml` 单文件多段，插件通过 manifest `credentials` 段声明字段，dyyl 在 `on_load` 前预检并经新 ABI 函数 `set_credentials` 注入。`logic.end` 栈式关闭 `_` 开放块。

**Tech Stack:** Rust 2021；`ureq`（同步 HTTP，已有依赖）；`serde`/`serde_json`/`toml`（已有）；`libloading`（已有，插件 dlopen）；`tokio`（dev-dep，mock HTTP server 测试）。

**Spec:** [docs/superpowers/specs/2026-07-13-ai-credentials-logic-end-design.md](file:///workspace/docs/superpowers/specs/2026-07-13-ai-credentials-logic-end-design.md)

**关键约束（来自 spec）：**
- HTTP 超时 1800 秒，重试 3 次（指数退避 1s/2s/4s），仅重试网络错误/5xx/429
- `ai.ask` 失败返回 `-1`（特例，非空字符串哨兵）
- 预扫描失败中止脚本（退出码 2），凭证提示中止退出码 3，未关闭块退出码 4
- 凭证文件**不自动 chmod 修正**，仅 `--debug` 警告
- ABI 从 v1 升到 v2，但**仍兼容 v1 插件**（无 `set_credentials` 符号时跳过注入）
- `ai.auto` 值类型由 AI 推断（JSON `type` 字段区分 string/number）

**项目 lint 严格（[Cargo.toml](file:///workspace/Cargo.toml) L42-L57）：** `unwrap_used`/`panic`/`todo`/`indexing_slicing` 全 deny。用 `?`、`get()`、`unwrap_or`/`unwrap_or_default`/`unwrap_or_else` 替代。

---

## 文件结构

**新增文件：**
- `src/credentials.rs` — credentials.toml 读写 + 交互式提示
- `src/ai/mod.rs` — AiProvider trait + AiError + 工厂
- `src/ai/client.rs` — HTTP 客户端（ureq + 重试 + 超时）
- `src/ai/provider_openai_chat.rs` — OpenAI Chat Completions
- `src/ai/provider_openai_response.rs` — OpenAI Responses API
- `src/ai/provider_anthropic.rs` — Anthropic Messages API
- `src/ai/prompt.rs` — ai.auto 批量 prompt 构造 + 响应解析
- `src/prepass.rs` — 预扫描：占位符扫描 + 回写 + reset_filled + run/build_only
- `src/runtime/cmd/ai.rs` — ai.ask / ai.auto.filled handler
- `tests/ai_tests.rs` — AI 模块单元 + 集成测试
- `tests/prepass_tests.rs` — 预扫描测试
- `tests/credentials_tests.rs` — 凭证测试
- `tests/logic_end_tests.rs` — logic.end 测试
- `tests/plugin_credentials_tests.rs` — 插件凭证集成测试
- `tests/fixtures/mock_ai_server.rs` — mock HTTP server 辅助
- `tests/ai_integration_tests.rs` — 端到端测试

**修改文件：**
- [src/lib.rs](file:///workspace/src/lib.rs) — 注册 `ai`/`prepass`/`credentials` 模块
- [src/main.rs](file:///workspace/src/main.rs) — `build` 子命令 + 预扫描集成
- [src/cli/mod.rs](file:///workspace/src/cli/mod.rs) — `build` 子命令分发
- [src/runtime/cmd/mod.rs](file:///workspace/src/runtime/cmd/mod.rs) — 注册 `ai` 模块
- [src/runtime/cmd/dispatch.rs](file:///workspace/src/runtime/cmd/dispatch.rs) — `ai.*` 路由
- [src/runtime/exec_block.rs](file:///workspace/src/runtime/exec_block.rs) — `_` 开放块 + `logic.end`
- [src/runtime/execute.rs](file:///workspace/src/runtime/execute.rs) — 块边界预扫描
- [src/runtime/plugin/abi.rs](file:///workspace/src/runtime/plugin/abi.rs) — ABI v2 + `set_credentials`
- [src/runtime/plugin/manifest.rs](file:///workspace/src/runtime/plugin/manifest.rs) — `credentials` 段
- [src/runtime/plugin/loader.rs](file:///workspace/src/runtime/plugin/loader.rs) — 凭证预检 + 注入
- [locales/en.json](file:///workspace/locales/en.json) + [locales/zh.json](file:///workspace/locales/zh.json) — 新 i18n 键

**实现顺序总览（共 18 个 Task）：**

1. credentials.rs 读写（Task 1）
2. credentials.rs 交互式提示（Task 2）
3. ai/mod.rs trait（Task 3）
4. ai/client.rs HTTP 客户端（Task 4）
5. ai/provider_openai_chat.rs（Task 5）
6. ai/provider_openai_response.rs（Task 6）
7. ai/provider_anthropic.rs（Task 7）
8. ai/prompt.rs 批量 prompt（Task 8）
9. runtime/cmd/ai.rs handler（Task 9）
10. prepass.rs 占位符扫描（Task 10）
11. prepass.rs 回写 + reset_filled（Task 11）
12. prepass.rs run + build_only（Task 12）
13. main.rs + cli build 子命令（Task 13）
14. logic.end 预扫描（Task 14）
15. logic.end 执行集成（Task 15）
16. 端到端 ai.auto 集成测试（Task 16）
17. manifest.rs credentials 段（Task 17）
18. abi.rs ABI v2 + loader.rs 凭证注入（Task 18）

---

## Task 1: credentials.rs — 凭证文件读写

**Files:**
- Create: `src/credentials.rs`
- Modify: `src/lib.rs`
- Test: `tests/credentials_tests.rs`

- [ ] **Step 1: 注册模块**

Edit [src/lib.rs](file:///workspace/src/lib.rs)，在 `pub mod config;` 后加：

```rust
pub mod ai;
pub mod credentials;
pub mod prepass;
```

- [ ] **Step 2: 写失败测试**

Create `tests/credentials_tests.rs`:

```rust
use dyyl::credentials::{AiCredentials, AiProviderKind, CredentialsFile};
use tempfile::tempdir;

#[test]
fn load_missing_file_returns_default() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("credentials.toml");
    let creds = CredentialsFile::load(&path).expect("load missing");
    assert!(creds.ai.is_none());
    assert!(creds.plugins.is_empty());
}

#[test]
fn roundtrip_ai_credentials() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("credentials.toml");
    let mut creds = CredentialsFile::default();
    creds.ai = Some(AiCredentials {
        provider: AiProviderKind::OpenaiChat,
        api_key: "sk-test".to_string(),
        model: "gpt-4o-mini".to_string(),
        base_url: String::new(),
        auto_system_prompt: String::new(),
    });
    creds.save(&path).expect("save");
    let loaded = CredentialsFile::load(&path).expect("load");
    assert_eq!(loaded.ai.as_ref().unwrap().api_key, "sk-test");
    assert_eq!(loaded.ai.as_ref().unwrap().provider, AiProviderKind::OpenaiChat);
}

#[test]
fn load_partial_ai_segment_returns_none() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("credentials.toml");
    std::fs::write(&path, "[ai]\nprovider = \"openai-chat\"\n").expect("write");
    let creds = CredentialsFile::load(&path).expect("load");
    assert!(creds.ai.is_none(), "missing api_key should yield None");
}

#[test]
fn plugin_credentials_roundtrip() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("credentials.toml");
    let mut creds = CredentialsFile::default();
    let mut plugin = std::collections::HashMap::new();
    plugin.insert("token".to_string(), "ghp_x".to_string());
    plugin.insert("repo".to_string(), "foo/bar".to_string());
    creds.plugins.insert("migpt".to_string(), plugin);
    creds.save(&path).expect("save");
    let loaded = CredentialsFile::load(&path).expect("load");
    assert_eq!(loaded.plugins.get("migpt").unwrap().get("token"), Some(&"ghp_x".to_string()));
}
```

- [ ] **Step 3: 运行测试确认失败**

Run: `cargo test --test credentials_tests`
Expected: FAIL — `unresolved module credentials`

- [ ] **Step 4: 实现 credentials.rs**

Create `src/credentials.rs`:

```rust
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
    s.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n")
}
```

- [ ] **Step 5: 创建 ai + prepass 模块占位（避免 lib.rs 编译失败）**

Create `src/ai/mod.rs`:

```rust
//! AI Provider 模块 — Task 3 实现。
```

Create `src/prepass.rs`:

```rust
//! 预扫描模块 — Task 10 实现。
```

- [ ] **Step 6: 运行测试确认通过**

Run: `cargo test --test credentials_tests`
Expected: PASS（4 个测试）

- [ ] **Step 7: 跑 clippy + fmt**

Run: `cargo clippy --all-targets --all-features 2>&1 | head -30`
Run: `cargo fmt`
Expected: 无 deny 级警告

- [ ] **Step 8: 提交**

```bash
git add src/credentials.rs src/ai/mod.rs src/prepass.rs src/lib.rs tests/credentials_tests.rs
git commit -m "feat(credentials): add credentials.toml read/write with AI + plugin segments"
```

---

## Task 2: credentials.rs — 交互式 AI 凭证提示

**Files:**
- Modify: `src/credentials.rs`
- Test: `tests/credentials_tests.rs`

- [ ] **Step 1: 确认 i18n::t 存在**

Run: `grep -n "pub fn t\b" src/i18n.rs`
Expected: 应已存在（项目已用 `i18n::t` 模式）。若不存在，先在 [src/i18n.rs](file:///workspace/src/i18n.rs) 加：

```rust
/// 按语言取消息并插值。args 是 (key, value) 元组列表。
#[must_use]
pub fn t(lang: Lang, key: &str, args: &[(&str, &str)]) -> String {
    let template = lookup(lang, key).unwrap_or_else(|| key.to_string());
    let mut result = template.to_string();
    for (k, v) in args {
        result = result.replace(&format!("{{{k}}}"), v);
    }
    result
}

fn lookup(lang: Lang, key: &str) -> Option<String> {
    store().lookup(lang, key)
}
```

并在 `MessageStore` impl 加 `lookup`：

```rust
pub fn lookup(&self, lang: Lang, key: &str) -> Option<String> {
    let map = match lang {
        Lang::En => &self.en,
        Lang::Zh => &self.zh,
    };
    map.get(key).cloned()
}
```

- [ ] **Step 2: 添加 i18n 键**

Edit [locales/en.json](file:///workspace/locales/en.json)，在末尾 `}` 前加（注意前一行加逗号）：

```json
  "ai.credential_prompt_header": "[dyyl] AI credentials not configured. Please enter:",
  "ai.credential_prompt_provider": "Provider (1=openai-chat, 2=openai-response, 3=anthropic):",
  "ai.credential_saved": "[dyyl] Credentials saved to {path}",
  "ai.ask_failed": "ai.ask failed: {reason}",
  "ai.prepass_failed": "prepass failed: {reason}",
  "ai.credential_aborted": "credential input aborted",
  "logic.end_without_open": "logic.end without open block",
  "logic.unclosed_block": "unclosed block at line {line}"
```

Edit [locales/zh.json](file:///workspace/locales/zh.json)，对应位置加：

```json
  "ai.credential_prompt_header": "[dyyl] AI 凭证未配置，请按提示输入：",
  "ai.credential_prompt_provider": "Provider (1=openai-chat, 2=openai-response, 3=anthropic):",
  "ai.credential_saved": "[dyyl] 凭证已保存到 {path}",
  "ai.ask_failed": "ai.ask 失败: {reason}",
  "ai.prepass_failed": "预扫描失败: {reason}",
  "ai.credential_aborted": "凭证输入中止",
  "logic.end_without_open": "logic.end 无开放块",
  "logic.unclosed_block": "第 {line} 行块未关闭"
```

- [ ] **Step 3: 写失败测试**

Append to `tests/credentials_tests.rs`:

```rust
#[test]
fn prompt_ai_reads_all_fields_from_lines() {
    use dyyl::credentials::prompt_ai_from_lines;
    let lines = vec![
        "1".to_string(),
        "sk-abc".to_string(),
        "gpt-4o".to_string(),
        "".to_string(),
    ];
    let (creds, consumed) = prompt_ai_from_lines(&lines).expect("prompt");
    assert_eq!(consumed, 4);
    assert_eq!(creds.provider, dyyl::credentials::AiProviderKind::OpenaiChat);
    assert_eq!(creds.api_key, "sk-abc");
    assert_eq!(creds.model, "gpt-4o");
    assert!(creds.base_url.is_empty());
}

#[test]
fn prompt_ai_invalid_choice_returns_error() {
    use dyyl::credentials::prompt_ai_from_lines;
    let lines = vec!["9".to_string()];
    let result = prompt_ai_from_lines(&lines);
    assert!(result.is_err());
}

#[test]
fn prompt_ai_empty_api_key_returns_error() {
    use dyyl::credentials::prompt_ai_from_lines;
    let lines = vec!["1".to_string(), "".to_string(), "model".to_string(), "".to_string()];
    assert!(prompt_ai_from_lines(&lines).is_err());
}
```

- [ ] **Step 4: 运行测试确认失败**

Run: `cargo test --test credentials_tests prompt_ai`
Expected: FAIL — `unresolved function prompt_ai_from_lines`

- [ ] **Step 5: 实现 prompt_ai_from_lines + ensure_ai**

Append to `src/credentials.rs`:

```rust
use crate::i18n::{self, Lang};

/// 从预读的输入行列表解析 AI 凭证。
///
/// 行顺序：provider 选择（1/2/3）、api_key、model、base_url（空行=默认）。
/// 返回 (凭证, 消耗的行数)。供测试与实际提示流程共用。
pub fn prompt_ai_from_lines(lines: &[String]) -> Result<(AiCredentials, usize), String> {
    if lines.len() < 4 {
        return Err("not enough input lines for AI credential prompt".to_string());
    }
    let choice: u8 = lines[0]
        .trim()
        .parse()
        .map_err(|_| format!("invalid provider choice: '{}'", lines[0]))?;
    let provider = AiProviderKind::from_choice(choice)
        .ok_or_else(|| format!("invalid provider choice: {choice}"))?;
    let api_key = lines[1].trim().to_string();
    if api_key.is_empty() {
        return Err("api_key cannot be empty".to_string());
    }
    let model = lines[2].trim().to_string();
    if model.is_empty() {
        return Err("model cannot be empty".to_string());
    }
    let base_url = lines[3].trim().to_string();
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
    eprintln!(
        "{}",
        i18n::t(lang, "ai.credential_prompt_header", &[])
    );
    eprintln!(
        "  {}",
        i18n::t(lang, "ai.credential_prompt_provider", &[])
    );
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
```

- [ ] **Step 6: 运行测试确认通过**

Run: `cargo test --test credentials_tests`
Expected: PASS

- [ ] **Step 7: 提交**

```bash
git add src/credentials.rs src/i18n.rs locales/en.json locales/zh.json tests/credentials_tests.rs
git commit -m "feat(credentials): add interactive AI credential prompt"
```

---

## Task 3: ai/mod.rs — AiProvider trait + 错误类型

**Files:**
- Modify: `src/ai/mod.rs`
- Test: `tests/ai_tests.rs`

- [ ] **Step 1: 写失败测试**

Create `tests/ai_tests.rs`:

```rust
use dyyl::ai::{AiError, AiErrorKind, AiProviderKind};

#[test]
fn provider_kind_from_choice() {
    assert_eq!(AiProviderKind::from_choice(1), Some(AiProviderKind::OpenaiChat));
    assert_eq!(AiProviderKind::from_choice(2), Some(AiProviderKind::OpenaiResponse));
    assert_eq!(AiProviderKind::from_choice(3), Some(AiProviderKind::Anthropic));
    assert_eq!(AiProviderKind::from_choice(4), None);
}

#[test]
fn ai_error_display() {
    let err = AiError::new(AiErrorKind::Auth, "invalid api key".to_string(), Some(401));
    assert!(err.to_string().contains("Auth"));
    assert!(err.to_string().contains("invalid api key"));
    assert_eq!(err.status, Some(401));
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test --test ai_tests`
Expected: FAIL — `unresolved import dyyl::ai`

- [ ] **Step 3: 实现 ai/mod.rs**

Replace `src/ai/mod.rs`:

```rust
//! AI Provider 模块 — 统一 trait + 三种 Provider 实现 + HTTP 客户端。

pub mod client;
pub mod prompt;
pub mod provider_anthropic;
pub mod provider_openai_chat;
pub mod provider_openai_response;

use crate::credentials::AiCredentials;

/// AI 错误类型。
#[derive(Debug, Clone)]
pub struct AiError {
    pub kind: AiErrorKind,
    pub message: String,
    pub status: Option<u16>,
}

impl AiError {
    #[must_use]
    pub fn new(kind: AiErrorKind, message: String, status: Option<u16>) -> Self {
        Self { kind, message, status }
    }
}

impl std::fmt::Display for AiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}: {}", self.kind, self.message)
    }
}

impl std::error::Error for AiError {}

/// AI 错误分类。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiErrorKind {
    Network,
    Auth,
    RateLimit,
    ServerError,
    Parse,
    Other,
}

/// AI Provider trait — 发送一次请求，返回响应文本。
pub trait AiProvider: Send + Sync {
    /// 发送一次 AI 请求。
    ///
    /// `system` 可能为空（用 provider 默认行为），`user` 是用户 prompt。
    fn ask(&self, system: &str, user: &str) -> Result<String, AiError>;
}

/// 根据凭证构造 Provider 实例。
#[must_use]
pub fn build_provider(creds: &AiCredentials) -> Box<dyn AiProvider> {
    match creds.provider {
        crate::credentials::AiProviderKind::OpenaiChat => {
            Box::new(provider_openai_chat::OpenaiChatProvider::new(
                creds.api_key.clone(),
                creds.model.clone(),
                creds.base_url.clone(),
            ))
        }
        crate::credentials::AiProviderKind::OpenaiResponse => {
            Box::new(provider_openai_response::OpenaiResponseProvider::new(
                creds.api_key.clone(),
                creds.model.clone(),
                creds.base_url.clone(),
            ))
        }
        crate::credentials::AiProviderKind::Anthropic => {
            Box::new(provider_anthropic::AnthropicProvider::new(
                creds.api_key.clone(),
                creds.model.clone(),
                creds.base_url.clone(),
            ))
        }
    }
}
```

- [ ] **Step 4: 创建子模块占位**

Create `src/ai/client.rs`:
```rust
//! HTTP 客户端 — Task 4 实现。
```

Create `src/ai/provider_openai_chat.rs`:
```rust
//! OpenAI Chat Completions — Task 5 实现。
```

Create `src/ai/provider_openai_response.rs`:
```rust
//! OpenAI Responses API — Task 6 实现。
```

Create `src/ai/provider_anthropic.rs`:
```rust
//! Anthropic Messages API — Task 7 实现。
```

Create `src/ai/prompt.rs`:
```rust
//! ai.auto 批量 prompt 构造 + 响应解析 — Task 8 实现。
```

- [ ] **Step 5: 运行测试确认通过**

Run: `cargo test --test ai_tests`
Expected: PASS

- [ ] **Step 6: 提交**

```bash
git add src/ai/ tests/ai_tests.rs
git commit -m "feat(ai): add AiProvider trait, AiError, and provider factory"
```

---

## Task 4: ai/client.rs — HTTP 客户端（重试 3 次 + 超时 1800s）

**Files:**
- Modify: `src/ai/client.rs`
- Test: `tests/ai_tests.rs`

- [ ] **Step 1: 写失败测试**

Append to `tests/ai_tests.rs`:

```rust
use dyyl::ai::client::{HttpRequest, HttpResponse, http_request_with_retry};
use std::time::Duration;

#[test]
fn retry_succeeds_on_second_attempt() {
    let mut attempts = 0usize;
    let result = http_request_with_retry(
        HttpRequest {
            url: "http://example.invalid".to_string(),
            method: "POST".to_string(),
            headers: vec![],
            body: String::new(),
        },
        Duration::from_millis(1),
        3,
        Duration::from_millis(1),
        Box::new(|_req| {
            attempts += 1;
            if attempts == 1 {
                Err(dyyl::ai::AiError::new(
                    dyyl::ai::AiErrorKind::ServerError,
                    "500".to_string(),
                    Some(500),
                ))
            } else {
                Ok(HttpResponse {
                    status: 200,
                    body: "ok".to_string(),
                })
            }
        }),
    );
    assert!(result.is_ok());
    assert_eq!(result.unwrap().body, "ok");
    assert_eq!(attempts, 2);
}

#[test]
fn retry_exhausted_returns_error() {
    let result = http_request_with_retry(
        HttpRequest {
            url: "http://example.invalid".to_string(),
            method: "POST".to_string(),
            headers: vec![],
            body: String::new(),
        },
        Duration::from_millis(1),
        2,
        Duration::from_millis(1),
        Box::new(|_req| {
            Err(dyyl::ai::AiError::new(
                dyyl::ai::AiErrorKind::ServerError,
                "always fails".to_string(),
                Some(500),
            ))
        }),
    );
    assert!(result.is_err());
}

#[test]
fn no_retry_on_4xx_auth() {
    let mut attempts = 0usize;
    let result = http_request_with_retry(
        HttpRequest {
            url: "http://example.invalid".to_string(),
            method: "POST".to_string(),
            headers: vec![],
            body: String::new(),
        },
        Duration::from_millis(1),
        3,
        Duration::from_millis(1),
        Box::new(|_req| {
            attempts += 1;
            Err(dyyl::ai::AiError::new(
                dyyl::ai::AiErrorKind::Auth,
                "401".to_string(),
                Some(401),
            ))
        }),
    );
    assert!(result.is_err());
    assert_eq!(attempts, 1, "4xx auth errors should not retry");
}

#[test]
fn retry_on_rate_limit_429() {
    let mut attempts = 0usize;
    let result = http_request_with_retry(
        HttpRequest {
            url: "http://example.invalid".to_string(),
            method: "POST".to_string(),
            headers: vec![],
            body: String::new(),
        },
        Duration::from_millis(1),
        2,
        Duration::from_millis(1),
        Box::new(|_req| {
            attempts += 1;
            if attempts < 3 {
                Err(dyyl::ai::AiError::new(
                    dyyl::ai::AiErrorKind::RateLimit,
                    "429".to_string(),
                    Some(429),
                ))
            } else {
                Ok(HttpResponse { status: 200, body: "ok".to_string() })
            }
        }),
    );
    assert!(result.is_ok());
    assert_eq!(attempts, 3);
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test --test ai_tests client`
Expected: FAIL — `unresolved module client`

- [ ] **Step 3: 实现 client.rs**

Replace `src/ai/client.rs`:

```rust
//! HTTP 客户端 — ureq + 重试 + 超时。
//!
//! 超时 1800 秒（适配长推理模型）。重试 3 次，指数退避 1s/2s/4s。
//! 仅重试网络错误、5xx、429。4xx（除 429）不重试。

use std::io::Read as _;
use std::time::Duration;

use super::{AiError, AiErrorKind};

/// HTTP 请求描述（provider 构造，client 执行）。
#[derive(Clone, Debug)]
pub struct HttpRequest {
    pub url: String,
    pub method: String,
    pub headers: Vec<(String, String)>,
    pub body: String,
}

/// HTTP 响应。
#[derive(Clone, Debug)]
pub struct HttpResponse {
    pub status: u16,
    pub body: String,
}

/// 可注入的请求执行器（测试用 mock）。
pub type RequestExecutor = Box<dyn Fn(&HttpRequest) -> Result<HttpResponse, AiError> + Send + Sync>;

/// 默认执行器：用 ureq 发真实 HTTP 请求。
fn default_executor(timeout: Duration) -> RequestExecutor {
    Box::new(move |req: &HttpRequest| -> Result<HttpResponse, AiError> {
        let agent = ureq::AgentBuilder::new()
            .timeout(timeout)
            .build();
        let mut request = match req.method.as_str() {
            "GET" => agent.get(&req.url),
            _ => agent.post(&req.url),
        };
        for (k, v) in &req.headers {
            request = request.set(k, v);
        }
        match if req.body.is_empty() {
            request.call()
        } else {
            request.send_string(&req.body)
        } {
            Ok(resp) => {
                let status = resp.status();
                let mut body = String::new();
                resp.into_reader()
                    .read_to_string(&mut body)
                    .map_err(|e| AiError::new(AiErrorKind::Network, format!("read body: {e}"), None))?;
                Ok(HttpResponse { status: status as u16, body })
            }
            Err(ureq::Error::Status(code, resp)) => {
                let mut body = String::new();
                let _ = resp.into_reader().read_to_string(&mut body);
                let kind = if code == 401 || code == 403 {
                    AiErrorKind::Auth
                } else if code == 429 {
                    AiErrorKind::RateLimit
                } else if code >= 500 {
                    AiErrorKind::ServerError
                } else {
                    AiErrorKind::Other
                };
                Err(AiError::new(kind, format!("HTTP {code}: {body}"), Some(code as u16)))
            }
            Err(ureq::Error::Transport(e)) => {
                Err(AiError::new(AiErrorKind::Network, format!("transport: {e}"), None))
            }
        }
    })
}

/// 带重试的 HTTP 请求。
///
/// - `timeout`: 单次请求超时（仅用于默认执行器，mock 执行器忽略）。
/// - `max_retries`: 最大重试次数（总请求数 = 1 + max_retries）。
/// - `backoff`: 首次重试前等待时长（后续指数翻倍）。
/// - `executor`: 请求执行器。
pub fn http_request_with_retry(
    request: HttpRequest,
    timeout: Duration,
    max_retries: u32,
    backoff: Duration,
    executor: RequestExecutor,
) -> Result<HttpResponse, AiError> {
    let _ = timeout; // 已封装在 default_executor 内
    let mut last_err: Option<AiError> = None;
    let mut current_backoff = backoff;
    for attempt in 0..=max_retries {
        if attempt > 0 {
            std::thread::sleep(current_backoff);
            current_backoff = current_backoff.saturating_mul(2);
        }
        match executor(&request) {
            Ok(resp) => return Ok(resp),
            Err(e) => {
                let retryable = matches!(
                    e.kind,
                    AiErrorKind::Network | AiErrorKind::ServerError | AiErrorKind::RateLimit
                );
                if !retryable {
                    return Err(e);
                }
                last_err = Some(e);
            }
        }
    }
    Err(last_err.unwrap_or_else(|| {
        AiError::new(AiErrorKind::Other, "no attempts made".to_string(), None)
    }))
}

/// 公开入口：用默认 ureq 执行器发带重试的请求。
pub fn request_with_retry(request: HttpRequest) -> Result<HttpResponse, AiError> {
    // 超时 1800 秒，重试 3 次，退避 1 秒起。
    http_request_with_retry(
        request,
        Duration::from_secs(1800),
        3,
        Duration::from_secs(1),
        default_executor(Duration::from_secs(1800)),
    )
}
```

- [ ] **Step 4: 运行测试确认通过**

Run: `cargo test --test ai_tests client`
Expected: PASS（4 个测试）

- [ ] **Step 5: 提交**

```bash
git add src/ai/client.rs tests/ai_tests.rs
git commit -m "feat(ai): add HTTP client with 3x retry and 1800s timeout"
```

---

## Task 5: ai/provider_openai_chat.rs — OpenAI Chat Completions

**Files:**
- Modify: `src/ai/provider_openai_chat.rs`
- Test: `tests/ai_tests.rs`

- [ ] **Step 1: 写失败测试**

Append to `tests/ai_tests.rs`:

```rust
use dyyl::ai::provider_openai_chat::OpenaiChatProvider;
use dyyl::ai::AiProvider;

#[test]
fn openai_chat_builds_correct_request_body() {
    let provider = OpenaiChatProvider::new(
        "sk-test".to_string(),
        "gpt-4o-mini".to_string(),
        String::new(),
    );
    let req = provider.build_request("You are helpful", "What is 2+2?");
    assert_eq!(req.method, "POST");
    assert!(req.url.ends_with("/chat/completions"));
    assert!(req.headers.iter().any(|(k, v)| k == "Authorization" && v == "Bearer sk-test"));
    let body: serde_json::Value = serde_json::from_str(&req.body).expect("valid json");
    assert_eq!(body["model"], "gpt-4o-mini");
    assert_eq!(body["messages"][0]["role"], "system");
    assert_eq!(body["messages"][0]["content"], "You are helpful");
    assert_eq!(body["messages"][1]["role"], "user");
    assert_eq!(body["messages"][1]["content"], "What is 2+2?");
}

#[test]
fn openai_chat_parses_response() {
    let provider = OpenaiChatProvider::new(
        "sk-test".to_string(),
        "gpt-4o-mini".to_string(),
        String::new(),
    );
    let resp_body = r#"{"choices":[{"message":{"content":"4"}}]}"#;
    let result = provider.parse_response(resp_body);
    assert_eq!(result, Ok("4".to_string()));
}

#[test]
fn openai_chat_default_base_url() {
    let provider = OpenaiChatProvider::new(
        "sk-test".to_string(),
        "gpt-4o-mini".to_string(),
        String::new(),
    );
    let req = provider.build_request("sys", "usr");
    assert_eq!(req.url, "https://api.openai.com/v1/chat/completions");
}

#[test]
fn openai_chat_custom_base_url() {
    let provider = OpenaiChatProvider::new(
        "sk-test".to_string(),
        "gpt-4o-mini".to_string(),
        "http://localhost:8080".to_string(),
    );
    let req = provider.build_request("sys", "usr");
    assert_eq!(req.url, "http://localhost:8080/chat/completions");
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test --test ai_tests openai_chat`
Expected: FAIL

- [ ] **Step 3: 实现 provider_openai_chat.rs**

Replace `src/ai/provider_openai_chat.rs`:

```rust
//! OpenAI Chat Completions API provider.
//!
//! 端点：{base_url}/chat/completions（base_url 空 = https://api.openai.com/v1）。

use super::client::{HttpRequest, request_with_retry};
use super::{AiError, AiErrorKind, AiProvider};
use serde_json::Value;

const DEFAULT_BASE_URL: &str = "https://api.openai.com/v1";

pub struct OpenaiChatProvider {
    api_key: String,
    model: String,
    base_url: String,
}

impl OpenaiChatProvider {
    #[must_use]
    pub fn new(api_key: String, model: String, base_url: String) -> Self {
        Self { api_key, model, base_url }
    }

    /// 构造 HTTP 请求（不发送）— 供测试验证请求体。
    #[must_use]
    pub fn build_request(&self, system: &str, user: &str) -> HttpRequest {
        let base = if self.base_url.is_empty() {
            DEFAULT_BASE_URL
        } else {
            &self.base_url
        };
        let body = serde_json::json!({
            "model": self.model,
            "messages": [
                {"role": "system", "content": system},
                {"role": "user", "content": user},
            ],
        });
        HttpRequest {
            url: format!("{base}/chat/completions"),
            method: "POST".to_string(),
            headers: vec![
                ("Authorization".to_string(), format!("Bearer {}", self.api_key)),
                ("Content-Type".to_string(), "application/json".to_string()),
            ],
            body: body.to_string(),
        }
    }

    /// 解析响应体，提取 choices[0].message.content。
    pub fn parse_response(&self, body: &str) -> Result<String, AiError> {
        let v: Value = serde_json::from_str(body).map_err(|e| {
            AiError::new(AiErrorKind::Parse, format!("invalid JSON: {e}"), None)
        })?;
        let content = v
            .get("choices")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("message"))
            .and_then(|m| m.get("content"))
            .and_then(|c| c.as_str());
        match content {
            Some(s) => Ok(s.to_string()),
            None => Err(AiError::new(
                AiErrorKind::Parse,
                "missing choices[0].message.content".to_string(),
                None,
            )),
        }
    }
}

impl AiProvider for OpenaiChatProvider {
    fn ask(&self, system: &str, user: &str) -> Result<String, AiError> {
        let req = self.build_request(system, user);
        let resp = request_with_retry(req)?;
        self.parse_response(&resp.body)
    }
}
```

- [ ] **Step 4: 运行测试确认通过**

Run: `cargo test --test ai_tests openai_chat`
Expected: PASS（4 个测试）

- [ ] **Step 5: 提交**

```bash
git add src/ai/provider_openai_chat.rs tests/ai_tests.rs
git commit -m "feat(ai): add OpenAI Chat Completions provider"
```

---

## Task 6: ai/provider_openai_response.rs — OpenAI Responses API

**Files:**
- Modify: `src/ai/provider_openai_response.rs`
- Test: `tests/ai_tests.rs`

- [ ] **Step 1: 写失败测试**

Append to `tests/ai_tests.rs`:

```rust
use dyyl::ai::provider_openai_response::OpenaiResponseProvider;
use dyyl::ai::AiProvider;

#[test]
fn openai_response_builds_correct_request_body() {
    let provider = OpenaiResponseProvider::new(
        "sk-test".to_string(),
        "gpt-4o".to_string(),
        String::new(),
    );
    let req = provider.build_request("Be concise", "Hi");
    assert!(req.url.ends_with("/responses"));
    let body: serde_json::Value = serde_json::from_str(&req.body).expect("json");
    assert_eq!(body["model"], "gpt-4o");
    assert_eq!(body["instructions"], "Be concise");
    assert_eq!(body["input"], "Hi");
}

#[test]
fn openai_response_parses_output_text() {
    let provider = OpenaiResponseProvider::new(
        "sk-test".to_string(),
        "gpt-4o".to_string(),
        String::new(),
    );
    let body = r#"{"output":[{"content":[{"type":"output_text","text":"Hello"}]}]}"#;
    assert_eq!(provider.parse_response(body), Ok("Hello".to_string()));
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test --test ai_tests openai_response`
Expected: FAIL

- [ ] **Step 3: 实现 provider_openai_response.rs**

Replace `src/ai/provider_openai_response.rs`:

```rust
//! OpenAI Responses API provider.
//!
//! 端点：{base_url}/responses（base_url 空 = https://api.openai.com/v1）。

use super::client::{HttpRequest, request_with_retry};
use super::{AiError, AiErrorKind, AiProvider};
use serde_json::Value;

const DEFAULT_BASE_URL: &str = "https://api.openai.com/v1";

pub struct OpenaiResponseProvider {
    api_key: String,
    model: String,
    base_url: String,
}

impl OpenaiResponseProvider {
    #[must_use]
    pub fn new(api_key: String, model: String, base_url: String) -> Self {
        Self { api_key, model, base_url }
    }

    #[must_use]
    pub fn build_request(&self, system: &str, user: &str) -> HttpRequest {
        let base = if self.base_url.is_empty() {
            DEFAULT_BASE_URL
        } else {
            &self.base_url
        };
        let body = serde_json::json!({
            "model": self.model,
            "instructions": system,
            "input": user,
        });
        HttpRequest {
            url: format!("{base}/responses"),
            method: "POST".to_string(),
            headers: vec![
                ("Authorization".to_string(), format!("Bearer {}", self.api_key)),
                ("Content-Type".to_string(), "application/json".to_string()),
            ],
            body: body.to_string(),
        }
    }

    /// 解析响应：output[*].content[*].text（找第一个有 text 的）。
    pub fn parse_response(&self, body: &str) -> Result<String, AiError> {
        let v: Value = serde_json::from_str(body).map_err(|e| {
            AiError::new(AiErrorKind::Parse, format!("invalid JSON: {e}"), None)
        })?;
        let output = v.get("output").and_then(|o| o.as_array());
        if let Some(arr) = output {
            for item in arr {
                if let Some(content) = item.get("content").and_then(|c| c.as_array()) {
                    for c in content {
                        if let Some(text) = c.get("text").and_then(|t| t.as_str()) {
                            return Ok(text.to_string());
                        }
                    }
                }
            }
        }
        Err(AiError::new(
            AiErrorKind::Parse,
            "missing output[*].content[*].text".to_string(),
            None,
        ))
    }
}

impl AiProvider for OpenaiResponseProvider {
    fn ask(&self, system: &str, user: &str) -> Result<String, AiError> {
        let req = self.build_request(system, user);
        let resp = request_with_retry(req)?;
        self.parse_response(&resp.body)
    }
}
```

- [ ] **Step 4: 运行测试确认通过**

Run: `cargo test --test ai_tests openai_response`
Expected: PASS

- [ ] **Step 5: 提交**

```bash
git add src/ai/provider_openai_response.rs tests/ai_tests.rs
git commit -m "feat(ai): add OpenAI Responses API provider"
```

---

## Task 7: ai/provider_anthropic.rs — Anthropic Messages API

**Files:**
- Modify: `src/ai/provider_anthropic.rs`
- Test: `tests/ai_tests.rs`

- [ ] **Step 1: 写失败测试**

Append to `tests/ai_tests.rs`:

```rust
use dyyl::ai::provider_anthropic::AnthropicProvider;
use dyyl::ai::AiProvider;

#[test]
fn anthropic_builds_correct_request_body() {
    let provider = AnthropicProvider::new(
        "sk-ant".to_string(),
        "claude-3-5-sonnet-20241022".to_string(),
        String::new(),
    );
    let req = provider.build_request("You are helpful", "Hi");
    assert_eq!(req.url, "https://api.anthropic.com/v1/messages");
    assert!(req.headers.iter().any(|(k, v)| k == "x-api-key" && v == "sk-ant"));
    assert!(req.headers.iter().any(|(k, v)| k == "anthropic-version" && v == "2023-06-01"));
    let body: serde_json::Value = serde_json::from_str(&req.body).expect("json");
    assert_eq!(body["model"], "claude-3-5-sonnet-20241022");
    assert_eq!(body["max_tokens"], 4096);
    assert_eq!(body["system"], "You are helpful");
    assert_eq!(body["messages"][0]["role"], "user");
    assert_eq!(body["messages"][0]["content"], "Hi");
}

#[test]
fn anthropic_parses_content_text() {
    let provider = AnthropicProvider::new(
        "sk-ant".to_string(),
        "claude-3-5-sonnet-20241022".to_string(),
        String::new(),
    );
    let body = r#"{"content":[{"type":"text","text":"Hello"}]}"#;
    assert_eq!(provider.parse_response(body), Ok("Hello".to_string()));
}

#[test]
fn anthropic_omits_system_when_empty() {
    let provider = AnthropicProvider::new(
        "sk-ant".to_string(),
        "claude".to_string(),
        String::new(),
    );
    let req = provider.build_request("", "Hi");
    let body: serde_json::Value = serde_json::from_str(&req.body).expect("json");
    assert!(body.get("system").is_none() || body["system"].as_str() == Some(""));
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test --test ai_tests anthropic`
Expected: FAIL

- [ ] **Step 3: 实现 provider_anthropic.rs**

Replace `src/ai/provider_anthropic.rs`:

```rust
//! Anthropic Messages API provider.
//!
//! 端点：{base_url}/v1/messages（base_url 空 = https://api.anthropic.com）。
//! 请求头：x-api-key + anthropic-version: 2023-06-01。

use super::client::{HttpRequest, request_with_retry};
use super::{AiError, AiErrorKind, AiProvider};
use serde_json::Value;

const DEFAULT_BASE_URL: &str = "https://api.anthropic.com";
const ANTHROPIC_VERSION: &str = "2023-06-01";
const MAX_TOKENS: u32 = 4096;

pub struct AnthropicProvider {
    api_key: String,
    model: String,
    base_url: String,
}

impl AnthropicProvider {
    #[must_use]
    pub fn new(api_key: String, model: String, base_url: String) -> Self {
        Self { api_key, model, base_url }
    }

    #[must_use]
    pub fn build_request(&self, system: &str, user: &str) -> HttpRequest {
        let base = if self.base_url.is_empty() {
            DEFAULT_BASE_URL
        } else {
            &self.base_url
        };
        let mut body = serde_json::json!({
            "model": self.model,
            "max_tokens": MAX_TOKENS,
            "messages": [
                {"role": "user", "content": user},
            ],
        });
        if !system.is_empty() {
            body["system"] = Value::String(system.to_string());
        }
        HttpRequest {
            url: format!("{base}/v1/messages"),
            method: "POST".to_string(),
            headers: vec![
                ("x-api-key".to_string(), self.api_key.clone()),
                ("anthropic-version".to_string(), ANTHROPIC_VERSION.to_string()),
                ("Content-Type".to_string(), "application/json".to_string()),
            ],
            body: body.to_string(),
        }
    }

    /// 解析响应：content[*].text（找第一个 type=text 的）。
    pub fn parse_response(&self, body: &str) -> Result<String, AiError> {
        let v: Value = serde_json::from_str(body).map_err(|e| {
            AiError::new(AiErrorKind::Parse, format!("invalid JSON: {e}"), None)
        })?;
        let content = v.get("content").and_then(|c| c.as_array());
        if let Some(arr) = content {
            for item in arr {
                if item.get("type").and_then(|t| t.as_str()) == Some("text") {
                    if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
                        return Ok(text.to_string());
                    }
                }
            }
        }
        Err(AiError::new(
            AiErrorKind::Parse,
            "missing content[*].text".to_string(),
            None,
        ))
    }
}

impl AiProvider for AnthropicProvider {
    fn ask(&self, system: &str, user: &str) -> Result<String, AiError> {
        let req = self.build_request(system, user);
        let resp = request_with_retry(req)?;
        self.parse_response(&resp.body)
    }
}
```

- [ ] **Step 4: 运行测试确认通过**

Run: `cargo test --test ai_tests anthropic`
Expected: PASS（3 个测试）

- [ ] **Step 5: 提交**

```bash
git add src/ai/provider_anthropic.rs tests/ai_tests.rs
git commit -m "feat(ai): add Anthropic Messages API provider"
```

---

## Task 8: ai/prompt.rs — 批量 prompt 构造 + 响应解析

**Files:**
- Modify: `src/ai/prompt.rs`
- Test: `tests/ai_tests.rs`

- [ ] **Step 1: 写失败测试**

Append to `tests/ai_tests.rs`:

```rust
use dyyl::ai::prompt::{build_batch, parse_response, Placeholder};

#[test]
fn build_batch_marks_placeholders_with_ids() {
    let content = "set $port, ai.auto \"端口常用25565\"\nset $name, ai.auto\n";
    let placeholders = vec![
        Placeholder { id: 1, line: 1, hint: Some("端口常用25565".to_string()), original_text: "ai.auto \"端口常用25565\"".to_string() },
        Placeholder { id: 2, line: 2, hint: None, original_text: "ai.auto".to_string() },
    ];
    let (system, user) = build_batch(content, &placeholders);
    assert!(system.contains("filling placeholder values"));
    assert!(user.contains("<<<AUTO_1: 端口常用25565>>>"));
    assert!(user.contains("<<<AUTO_2: (no hint, infer from position)>>>"));
    assert!(user.contains("set $port, <<<AUTO_1"));
}

#[test]
fn parse_response_extracts_typed_values() {
    let body = r#"{"1":{"type":"string","value":"Steve"},"2":{"type":"number","value":25565}}"#;
    let values = parse_response(body).expect("parse");
    assert_eq!(values.len(), 2);
    assert_eq!(values.get("1").unwrap().value, "Steve");
    assert_eq!(values.get("1").unwrap().is_number, false);
    assert_eq!(values.get("2").unwrap().value, "25565");
    assert_eq!(values.get("2").unwrap().is_number, true);
}

#[test]
fn parse_response_strips_markdown_code_fence() {
    let body = "```json\n{\"1\":{\"type\":\"string\",\"value\":\"x\"}}\n```";
    let values = parse_response(body).expect("parse");
    assert_eq!(values.get("1").unwrap().value, "x");
}

#[test]
fn parse_response_extracts_json_from_surrounding_text() {
    let body = "Here are the values:\n{\"1\":{\"type\":\"number\",\"value\":42}}\nDone.";
    let values = parse_response(body).expect("parse");
    assert_eq!(values.get("1").unwrap().value, "42");
}

#[test]
fn parse_response_empty_json_object() {
    let body = "{}";
    let values = parse_response(body).expect("parse");
    assert!(values.is_empty());
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test --test ai_tests prompt`
Expected: FAIL

- [ ] **Step 3: 实现 prompt.rs**

Replace `src/ai/prompt.rs`:

```rust
//! ai.auto 批量 prompt 构造 + 响应解析。
//!
//! 占位符在源码里被替换为 <<<AUTO_<id>: <hint or "(no hint, infer from position)">>>。
//! AI 返回 JSON {"1":{"type":"string|number","value":...}, ...}。

use std::collections::HashMap;

/// 一个 ai.auto 占位符的描述。
#[derive(Clone, Debug)]
pub struct Placeholder {
    pub id: u32,
    pub line: usize,
    /// None = 无提示（AI 纯靠位置推断）。
    pub hint: Option<String>,
    /// 源码中 `ai.auto ...` 的原始文本（用于回写时替换）。
    pub original_text: String,
}

/// 解析后的值。
#[derive(Clone, Debug)]
pub struct FilledValue {
    /// 值的字符串形式（number 也转为字符串便于统一处理）。
    pub value: String,
    /// true = number 类型；false = string 类型。
    pub is_number: bool,
}

/// 内置 ai.auto 的 system prompt。
pub const DEFAULT_AUTO_SYSTEM_PROMPT: &str = "\
You are filling placeholder values in a dyyl script. The user will give you
the full script content with placeholders marked. For each placeholder,
infer the appropriate value from context and the placeholder's hint.
Return ONLY a JSON object mapping placeholder IDs to {type, value}.
type is \"string\" or \"number\". Do not include any explanation.";

/// 构造批量请求的 (system, user) prompt。
#[must_use]
pub fn build_batch(content: &str, placeholders: &[Placeholder]) -> (String, String) {
    let system = DEFAULT_AUTO_SYSTEM_PROMPT.to_string();
    let mut marked = content.to_string();
    for p in placeholders {
        let hint_str = match &p.hint {
            Some(h) => h.clone(),
            None => "(no hint, infer from position)".to_string(),
        };
        let marker = format!("<<<AUTO_{}: {}>>>", p.id, hint_str);
        marked = marked.replacen(&p.original_text, &marker, 1);
    }
    let n = placeholders.len();
    let user = format!(
        "Below is a dyyl script with {n} placeholders marked as <<<AUTO_<id>: <hint>>>.\n\
         Replace each placeholder. Return JSON: {{\"1\":{{\"type\":\"string\",\"value\":\"...\"}}, \"2\":{{\"type\":\"number\",\"value\":42}}, ...}}\n\n\
         --- SCRIPT START ---\n{marked}\n--- SCRIPT END ---"
    );
    (system, user)
}

/// 解析 AI 响应。
///
/// 容错：剥离 ```json ... ``` 代码块；提取首个 `{` 到末个 `}` 之间的子串。
pub fn parse_response(body: &str) -> Result<HashMap<String, FilledValue>, String> {
    let trimmed = body.trim();
    // 剥离 markdown 代码块。
    let stripped = if trimmed.starts_with("```") {
        let after_fence = trimmed
            .trim_start_matches("```json")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim();
        after_fence
    } else {
        trimmed
    };
    // 提取首个 { 到末个 } 之间的子串。
    let json_str = match (stripped.find('{'), stripped.rfind('}')) {
        (Some(start), Some(end)) if end > start => &stripped[start..=end],
        _ => {
            let preview = &stripped[..stripped.len().min(200)];
            return Err(format!("no JSON object found in response: {preview}"));
        }
    };
    let v: serde_json::Value = serde_json::from_str(json_str)
        .map_err(|e| format!("invalid JSON: {e}"))?;
    let obj = v.as_object().ok_or_else(|| "response is not a JSON object".to_string())?;
    let mut map = HashMap::new();
    for (id, entry) in obj {
        let entry_obj = match entry.as_object() {
            Some(o) => o,
            None => continue,
        };
        let type_str = entry_obj
            .get("type")
            .and_then(|t| t.as_str())
            .unwrap_or("string");
        let is_number = type_str == "number";
        let value = entry_obj
            .get("value")
            .map(|v| match v {
                serde_json::Value::String(s) => s.clone(),
                serde_json::Value::Number(n) => n.to_string(),
                _ => v.to_string(),
            })
            .unwrap_or_default();
        map.insert(id.clone(), FilledValue { value, is_number });
    }
    Ok(map)
}
```

- [ ] **Step 4: 运行测试确认通过**

Run: `cargo test --test ai_tests prompt`
Expected: PASS（5 个测试）

- [ ] **Step 5: 提交**

```bash
git add src/ai/prompt.rs tests/ai_tests.rs
git commit -m "feat(ai): add batch prompt construction and response parsing"
```

---

## Task 9: runtime/cmd/ai.rs — ai.ask + ai.auto.filled handler

**Files:**
- Create: `src/runtime/cmd/ai.rs`
- Modify: `src/runtime/cmd/mod.rs`, `src/runtime/cmd/dispatch.rs`
- Test: `tests/ai_tests.rs`

- [ ] **Step 1: 注册 ai 模块 + 路由**

Edit [src/runtime/cmd/mod.rs](file:///workspace/src/runtime/cmd/mod.rs)，在模块声明区加：

```rust
pub(crate) mod ai;
```

Edit [src/runtime/cmd/dispatch.rs](file:///workspace/src/runtime/cmd/dispatch.rs)，在 `cmd if cmd.starts_with("logic.")` 分支后加：

```rust
        cmd if cmd.starts_with("ai.") => ai::handle_ai_command(call, env, ctx),
```

- [ ] **Step 2: 写失败测试（ai.auto.filled）**

Append to `tests/ai_tests.rs`:

```rust
use dyyl::runtime::execute::run_script;
use dyyl::runtime::Value;

#[test]
fn ai_auto_filled_returns_string_value() {
    let v = run_script("set $x, ai.auto.filled _, \"hello\"\nio.out $x", false).values;
    assert_eq!(v[1], Value::Str("hello".to_string()));
}

#[test]
fn ai_auto_filled_number_returns_num() {
    let v = run_script("set $x, ai.auto.filled _, 42\nio.out $x", false).values;
    assert_eq!(v[1], Value::Num(42));
}

#[test]
fn ai_auto_filled_with_hint_ignores_hint() {
    let v = run_script(
        "set $x, ai.auto.filled \"some hint\", \"value\"\nio.out $x",
        false,
    ).values;
    assert_eq!(v[1], Value::Str("value".to_string()));
}
```

- [ ] **Step 3: 运行测试确认失败**

Run: `cargo test --test ai_tests ai_auto_filled`
Expected: FAIL — `unknown command ai.auto.filled`

- [ ] **Step 4: 实现 ai.rs**

Create `src/runtime/cmd/ai.rs`:

```rust
//! ai.* 命令 handler — ai.ask（运行时 HTTP）+ ai.auto.filled（取值）。

use super::context::ExecContext;
use super::dispatch::eval_expr;
use crate::i18n;
use crate::parser::types::Call;
use crate::runtime::env::Env;
use crate::runtime::error::RuntimeError;
use crate::runtime::value::Value;

/// 默认 ai.ask system prompt（单参数时用）。
const DEFAULT_ASK_SYSTEM_PROMPT: &str =
    "You are a helpful assistant. Answer the user's question concisely and accurately.";

/// 路由 ai.* 命令。
pub(crate) fn handle_ai_command(
    call: &Call,
    env: &mut Env,
    ctx: &ExecContext,
) -> Result<Value, RuntimeError> {
    let sub = &call.command["ai.".len()..];
    match sub {
        "ask" => handle_ai_ask(call, env, ctx),
        "auto.filled" => handle_ai_auto_filled(call, env, ctx),
        _ => Err(RuntimeError::new(
            ctx.line,
            &call.command,
            i18n::unknown_command(ctx.lang.get(), "ai", sub),
        )),
    }
}

/// ai.ask [system], <prompt>
///
/// 单参数：用内置默认 system。双参数：自定义 system。`_` 跳过 system。
fn handle_ai_ask(call: &Call, env: &mut Env, ctx: &ExecContext) -> Result<Value, RuntimeError> {
    if call.args.is_empty() {
        return Err(RuntimeError::new(
            ctx.line,
            &call.command,
            i18n::requires_args(ctx.lang.get(), 1),
        ));
    }
    let (system, user_val) = match call.args.len() {
        1 => (
            DEFAULT_ASK_SYSTEM_PROMPT.to_string(),
            eval_expr(&call.args[0], env, ctx)?,
        ),
        _ => {
            let first = eval_expr(&call.args[0], env, ctx)?;
            let user_expr = call.args.get(1).ok_or_else(|| {
                RuntimeError::new(
                    ctx.line,
                    &call.command,
                    i18n::requires_args(ctx.lang.get(), 2),
                )
            })?;
            let user_val = eval_expr(user_expr, env, ctx)?;
            let sys_str = match first {
                Value::Empty => DEFAULT_ASK_SYSTEM_PROMPT.to_string(),
                Value::Str(s) => s,
                Value::Num(n) => n.to_string(),
                Value::Expr(e) => e.to_string(),
                _ => {
                    return Err(RuntimeError::new(
                        ctx.line,
                        &call.command,
                        i18n::expected_string(ctx.lang.get(), &first),
                    ));
                }
            };
            (sys_str, user_val)
        }
    };
    let user_str = match user_val {
        Value::Str(s) => s,
        Value::Num(n) => n.to_string(),
        Value::Expr(e) => e.to_string(),
        _ => {
            return Err(RuntimeError::new(
                ctx.line,
                &call.command,
                i18n::expected_string(ctx.lang.get(), &user_val),
            ));
        }
    };
    // 加载凭证。
    let creds_path = match crate::credentials::CredentialsFile::default_path() {
        Some(p) => p,
        None => {
            if ctx.debug {
                eprintln!(
                    "line {}: {}",
                    ctx.line,
                    i18n::t(ctx.lang.get(), "ai.ask_failed", &[("reason", "no config dir")])
                );
            }
            return Ok(Value::Num(-1));
        }
    };
    let ai_creds = match crate::credentials::ensure_ai(&creds_path, ctx.lang.get()) {
        Ok(c) => c,
        Err(e) => {
            if ctx.debug {
                eprintln!(
                    "line {}: {}",
                    ctx.line,
                    i18n::t(ctx.lang.get(), "ai.ask_failed", &[("reason", &e)])
                );
            }
            return Ok(Value::Num(-1));
        }
    };
    let provider = crate::ai::build_provider(&ai_creds);
    match provider.ask(&system, &user_str) {
        Ok(s) => Ok(Value::Str(s)),
        Err(e) => {
            if ctx.debug {
                eprintln!(
                    "line {}: {}",
                    ctx.line,
                    i18n::t(ctx.lang.get(), "ai.ask_failed", &[("reason", &e.to_string())])
                );
            }
            Ok(Value::Num(-1))
        }
    }
}

/// ai.auto.filled <提示>, <值>
///
/// 运行时不请求 AI，直接返回值。提示参数忽略其内容。
fn handle_ai_auto_filled(
    call: &Call,
    env: &mut Env,
    ctx: &ExecContext,
) -> Result<Value, RuntimeError> {
    if call.args.len() < 2 {
        return Err(RuntimeError::new(
            ctx.line,
            &call.command,
            i18n::requires_args(ctx.lang.get(), 2),
        ));
    }
    let val_expr = call.args.get(1).ok_or_else(|| {
        RuntimeError::new(
            ctx.line,
            &call.command,
            i18n::requires_args(ctx.lang.get(), 2),
        )
    })?;
    eval_expr(val_expr, env, ctx)
}
```

- [ ] **Step 5: 运行测试确认通过**

Run: `cargo test --test ai_tests ai_auto_filled`
Expected: PASS（3 个测试）

- [ ] **Step 6: 提交**

```bash
git add src/runtime/cmd/ai.rs src/runtime/cmd/mod.rs src/runtime/cmd/dispatch.rs tests/ai_tests.rs
git commit -m "feat(ai): add ai.ask and ai.auto.filled command handlers"
```

---

## Task 10: prepass.rs — 占位符扫描

**Files:**
- Modify: `src/prepass.rs`
- Test: `tests/prepass_tests.rs`

- [ ] **Step 1: 写失败测试**

Create `tests/prepass_tests.rs`:

```rust
use dyyl::prepass::scan_placeholders;

#[test]
fn scan_finds_ai_auto_without_hint() {
    let content = "set $x, ai.auto\nio.out $x\n";
    let phs = scan_placeholders(content);
    assert_eq!(phs.len(), 1);
    assert_eq!(phs[0].line, 1);
    assert!(phs[0].hint.is_none());
    assert_eq!(phs[0].original_text, "ai.auto");
}

#[test]
fn scan_finds_ai_auto_with_quoted_hint() {
    let content = "set $port, ai.auto \"端口常用25565\"\n";
    let phs = scan_placeholders(content);
    assert_eq!(phs.len(), 1);
    assert_eq!(phs[0].hint.as_deref(), Some("端口常用25565"));
}

#[test]
fn scan_finds_ai_auto_with_bareword_hint() {
    let content = "set $x, ai.auto some_hint\n";
    let phs = scan_placeholders(content);
    assert_eq!(phs.len(), 1);
    assert_eq!(phs[0].hint.as_deref(), Some("some_hint"));
}

#[test]
fn scan_finds_multiple_placeholders() {
    let content = "set $a, ai.auto \"first\"\nset $b, ai.auto\nset $c, ai.auto \"third\"\n";
    let phs = scan_placeholders(content);
    assert_eq!(phs.len(), 3);
    assert_eq!(phs[0].id, 1);
    assert_eq!(phs[1].id, 2);
    assert_eq!(phs[2].id, 3);
}

#[test]
fn scan_ignores_ai_auto_filled() {
    let content = "set $x, ai.auto.filled _, \"value\"\nset $y, ai.auto\n";
    let phs = scan_placeholders(content);
    assert_eq!(phs.len(), 1, "ai.auto.filled should not be scanned");
    assert_eq!(phs[0].line, 2);
}

#[test]
fn scan_finds_inline_placeholder() {
    let content = "file.write ai.auto \"路径\", \"content\"\n";
    let phs = scan_placeholders(content);
    assert_eq!(phs.len(), 1);
    assert_eq!(phs[0].hint.as_deref(), Some("路径"));
}

#[test]
fn scan_returns_empty_when_no_placeholders() {
    let content = "io.out hello\nset $x, 42\n";
    let phs = scan_placeholders(content);
    assert!(phs.is_empty());
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test --test prepass_tests`
Expected: FAIL — `unresolved function scan_placeholders`

- [ ] **Step 3: 实现 scan_placeholders**

Replace `src/prepass.rs`:

```rust
//! 预扫描：检测 ai.auto 占位符 → 批量请求 AI → 回写源码。
//!
//! 仅做逐行文本扫描找 `ai.auto` 模式，不做完整 parser 解析。

use crate::ai::prompt::Placeholder;

/// 逐行扫描源码，找出所有未填的 `ai.auto` / `ai.auto <提示>` 占位符。
///
/// 不识别 `ai.auto.filled`（已填的）。
#[must_use]
pub fn scan_placeholders(content: &str) -> Vec<Placeholder> {
    let mut result = Vec::new();
    let mut id_counter = 1u32;
    for (idx, line) in content.lines().enumerate() {
        let line_no = idx + 1;
        if let Some(ph) = scan_line(line, line_no, id_counter) {
            id_counter = ph.id + 1;
            result.push(ph);
        }
    }
    result
}

/// 扫描单行，找 `ai.auto`（后不跟 `.filled`）。
fn scan_line(line: &str, line_no: usize, next_id: u32) -> Option<Placeholder> {
    let mut search_from = 0usize;
    while let Some(pos) = line[search_from..].find("ai.auto") {
        let abs_pos = search_from + pos;
        let after = &line[abs_pos + "ai.auto".len()..];
        // 跳过 ai.auto.filled。
        if after.starts_with(".filled") {
            search_from = abs_pos + "ai.auto.filled".len();
            continue;
        }
        // 前一字符若是字母/下划线/点 → 跳过（避免匹配 xxxai.auto）。
        if abs_pos > 0 {
            if let Some(c) = line.as_bytes().get(abs_pos - 1).copied() {
                if c.is_ascii_alphabetic() || c == b'_' || c == b'.' {
                    search_from = abs_pos + 1;
                    continue;
                }
            }
        }
        let rest = after.trim_start();
        let (hint, consumed_after) = parse_hint(rest);
        let original_text = format!("ai.auto{}", &after[..consumed_after]);
        let hint_opt = hint.map(|s| s.to_string());
        return Some(Placeholder {
            id: next_id,
            line: line_no,
            hint: hint_opt,
            original_text,
        });
    }
    None
}

/// 解析 `ai.auto` 之后的提示参数。
///
/// 返回 (Option<hint>, consumed_chars_after_ai_auto)。
fn parse_hint(rest: &str) -> (Option<&str>, usize) {
    if rest.is_empty() {
        return (None, 0);
    }
    let first_byte = rest.as_bytes().get(0).copied();
    if first_byte == Some(b'"') || first_byte == Some(b'\'') {
        let quote = first_byte.unwrap() as char;
        if let Some(end) = rest[1..].find(quote) {
            let hint = &rest[1..1 + end];
            let consumed = 1 + end + 1;
            return (Some(hint), consumed);
        }
        return (None, 0);
    }
    // 裸词提示：读到逗号或行尾。
    let end = rest.find(',').unwrap_or(rest.len());
    let bareword = rest[..end].trim_end();
    if bareword.is_empty() {
        return (None, 0);
    }
    (Some(bareword), end)
}
```

- [ ] **Step 4: 运行测试确认通过**

Run: `cargo test --test prepass_tests`
Expected: PASS（7 个测试）

- [ ] **Step 5: 提交**

```bash
git add src/prepass.rs tests/prepass_tests.rs
git commit -m "feat(prepass): add ai.auto placeholder scanner"
```

---

## Task 11: prepass.rs — 回写 + reset_filled

**Files:**
- Modify: `src/prepass.rs`
- Test: `tests/prepass_tests.rs`

- [ ] **Step 1: 写失败测试**

Append to `tests/prepass_tests.rs`:

```rust
use dyyl::ai::prompt::FilledValue;
use std::collections::HashMap;
use dyyl::prepass::{rewrite_placeholders, reset_filled};

#[test]
fn rewrite_empty_hint_string_value() {
    let content = "set $x, ai.auto\n";
    let phs = dyyl::prepass::scan_placeholders(content);
    let mut values = HashMap::new();
    values.insert("1".to_string(), FilledValue {
        value: "Steve".to_string(),
        is_number: false,
    });
    let result = rewrite_placeholders(content, &phs, &values);
    assert_eq!(result, "set $x, ai.auto.filled _, \"Steve\"\n");
}

#[test]
fn rewrite_empty_hint_number_value() {
    let content = "set $x, ai.auto\n";
    let phs = dyyl::prepass::scan_placeholders(content);
    let mut values = HashMap::new();
    values.insert("1".to_string(), FilledValue {
        value: "42".to_string(),
        is_number: true,
    });
    let result = rewrite_placeholders(content, &phs, &values);
    assert_eq!(result, "set $x, ai.auto.filled _, 42\n");
}

#[test]
fn rewrite_hint_number_value() {
    let content = "set $port, ai.auto \"端口\"\n";
    let phs = dyyl::prepass::scan_placeholders(content);
    let mut values = HashMap::new();
    values.insert("1".to_string(), FilledValue {
        value: "25565".to_string(),
        is_number: true,
    });
    let result = rewrite_placeholders(content, &phs, &values);
    assert_eq!(result, "set $port, ai.auto.filled \"端口\", 25565\n");
}

#[test]
fn rewrite_escapes_special_chars_in_string() {
    let content = "set $x, ai.auto\n";
    let phs = dyyl::prepass::scan_placeholders(content);
    let mut values = HashMap::new();
    values.insert("1".to_string(), FilledValue {
        value: "hello \"world\"\n".to_string(),
        is_number: false,
    });
    let result = rewrite_placeholders(content, &phs, &values);
    assert_eq!(result, "set $x, ai.auto.filled _, \"hello \\\"world\\\"\\n\"\n");
}

#[test]
fn rewrite_missing_value_keeps_original() {
    let content = "set $x, ai.auto \"hint\"\n";
    let phs = dyyl::prepass::scan_placeholders(content);
    let values = HashMap::new();
    let result = rewrite_placeholders(content, &phs, &values);
    assert_eq!(result, content, "missing value should keep original");
}

#[test]
fn reset_filled_strips_value_and_keeps_hint() {
    let content = "set $x, ai.auto.filled \"hint\", \"value\"\n";
    let result = reset_filled(content);
    assert_eq!(result, "set $x, ai.auto \"hint\"\n");
}

#[test]
fn reset_filled_empty_hint() {
    let content = "set $x, ai.auto.filled _, \"value\"\n";
    let result = reset_filled(content);
    assert_eq!(result, "set $x, ai.auto\n");
}

#[test]
fn reset_filled_number_value() {
    let content = "set $x, ai.auto.filled \"hint\", 42\n";
    let result = reset_filled(content);
    assert_eq!(result, "set $x, ai.auto \"hint\"\n");
}

#[test]
fn reset_filled_no_change_if_no_filled() {
    let content = "set $x, ai.auto \"hint\"\nio.out $x\n";
    let result = reset_filled(content);
    assert_eq!(result, content);
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test --test prepass_tests rewrite reset`
Expected: FAIL — `unresolved function rewrite_placeholders`

- [ ] **Step 3: 实现 rewrite_placeholders + reset_filled**

Append to `src/prepass.rs`:

```rust
use crate::ai::prompt::FilledValue;
use std::collections::HashMap;

/// 把占位符替换为 `ai.auto.filled <提示>, <值>` 并返回新源码。
///
/// 若某占位符在 `values` 中缺失，保持原 `ai.auto ...` 不变。
pub fn rewrite_placeholders(
    content: &str,
    placeholders: &[Placeholder],
    values: &HashMap<String, FilledValue>,
) -> String {
    let mut result = content.to_string();
    for p in placeholders {
        let filled = match values.get(&p.id.to_string()) {
            Some(v) => v,
            None => continue,
        };
        let value_literal = if filled.is_number {
            filled.value.clone()
        } else {
            escape_dyyl_string(&filled.value)
        };
        let hint_literal = match &p.hint {
            Some(h) => escape_dyyl_string(h),
            None => "_".to_string(),
        };
        let replacement = format!("ai.auto.filled {hint_literal}, {value_literal}");
        result = result.replacen(&p.original_text, &replacement, 1);
    }
    result
}

/// 把 `ai.auto.filled <提示>, <值>` 替换回 `ai.auto <提示>`。
pub fn reset_filled(content: &str) -> String {
    let mut result = content.to_string();
    loop {
        let pos = match result.find("ai.auto.filled") {
            Some(p) => p,
            None => break,
        };
        let after = &result[pos + "ai.auto.filled".len()..];
        let (hint_literal, consumed) = parse_filled_args(after);
        let hint_str = match hint_literal.trim() {
            "_" | "" => String::new(),
            s => format!(" {s}"),
        };
        let replacement = format!("ai.auto{hint_str}");
        let full_segment = format!("ai.auto.filled{}", &after[..consumed]);
        result = result.replacen(&full_segment, &replacement, 1);
    }
    result
}

/// 解析 `ai.auto.filled` 之后的参数段。
///
/// 返回 (提示字面量字符串, consumed_chars_from_after)。
/// consumed 到行尾（含值部分，便于 replacen 完整匹配）。
fn parse_filled_args(rest: &str) -> (String, usize) {
    let line_end = rest.find('\n').unwrap_or(rest.len());
    let segment = &rest[..line_end];
    // 找顶层逗号分隔提示和值。
    let comma_pos = match find_top_level_comma(segment) {
        Some(p) => p,
        None => return (segment.trim().to_string(), line_end),
    };
    let hint_part = segment[..comma_pos].trim().to_string();
    (hint_part, line_end)
}

/// 找顶层逗号（不在引号内）。
fn find_top_level_comma(s: &str) -> Option<usize> {
    let mut in_quote: Option<char> = None;
    for (i, c) in s.char_indices() {
        match in_quote {
            Some(q) => {
                if c == q {
                    in_quote = None;
                }
            }
            None => {
                if c == '"' || c == '\'' {
                    in_quote = Some(c);
                } else if c == ',' {
                    return Some(i);
                }
            }
        }
    }
    None
}

/// 转义 dyyl 字符串字面量：双引号包裹，转义 `"` `\` 换行。
fn escape_dyyl_string(s: &str) -> String {
    let escaped = s
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n");
    format!("\"{escaped}\"")
}
```

- [ ] **Step 4: 运行测试确认通过**

Run: `cargo test --test prepass_tests`
Expected: PASS

- [ ] **Step 5: 提交**

```bash
git add src/prepass.rs tests/prepass_tests.rs
git commit -m "feat(prepass): add placeholder rewrite and reset_filled"
```

---

## Task 12: prepass.rs — run() + build_only()

**Files:**
- Modify: `src/prepass.rs`
- Test: `tests/prepass_tests.rs`

- [ ] **Step 1: 写失败测试**

Append to `tests/prepass_tests.rs`:

```rust
use dyyl::prepass::{run, build_only};
use std::fs;
use tempfile::tempdir;

#[test]
fn run_skips_when_no_placeholders() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("script.dyyl");
    fs::write(&path, "io.out hello\n").unwrap();
    run(&path, dyyl::i18n::Lang::En).expect("ok");
    assert_eq!(fs::read_to_string(&path).unwrap(), "io.out hello\n");
}

#[test]
fn build_only_no_change_when_no_ai_auto() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("script.dyyl");
    fs::write(&path, "io.out hello\n").unwrap();
    build_only(&path, dyyl::i18n::Lang::En).expect("ok");
    assert_eq!(fs::read_to_string(&path).unwrap(), "io.out hello\n");
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test --test prepass_tests run build_only`
Expected: FAIL — `unresolved function run`

- [ ] **Step 3: 实现 run + build_only**

Append to `src/prepass.rs`:

```rust
use crate::ai::prompt::{build_batch, parse_response};
use crate::credentials;
use crate::i18n::Lang;
use std::path::Path;

/// 预扫描错误。
#[derive(Debug)]
pub enum PrepassError {
    Io(String),
    AiFailed(String),
    ParseFailed(String),
    CredentialAborted,
}

impl std::fmt::Display for PrepassError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(s) => write!(f, "io error: {s}"),
            Self::AiFailed(s) => write!(f, "ai request failed: {s}"),
            Self::ParseFailed(s) => write!(f, "parse failed: {s}"),
            Self::CredentialAborted => write!(f, "credential input aborted"),
        }
    }
}

impl std::error::Error for PrepassError {}

/// 预扫描入口：检测未填 ai.auto → 批量请求 → 回写。
///
/// credentials path 从 `DYYL_CREDENTIALS_PATH` 环境变量读，若未设则用默认路径。
/// 无未填占位符时直接返回 Ok。
pub fn run(file: &Path, lang: Lang) -> Result<(), PrepassError> {
    let content = std::fs::read_to_string(file)
        .map_err(|e| PrepassError::Io(format!("read {}: {e}", file.display())))?;
    let placeholders = scan_placeholders(&content);
    if placeholders.is_empty() {
        return Ok(());
    }
    let creds_path = match std::env::var("DYYL_CREDENTIALS_PATH") {
        Ok(p) => std::path::PathBuf::from(p),
        Err(_) => credentials::CredentialsFile::default_path()
            .ok_or_else(|| PrepassError::AiFailed("no config dir".to_string()))?,
    };
    let ai_creds = credentials::ensure_ai(&creds_path, lang)
        .map_err(|_| PrepassError::CredentialAborted)?;
    let provider = crate::ai::build_provider(&ai_creds);
    let (system, user_prompt) = build_batch(&content, &placeholders);
    let response = provider
        .ask(&system, &user_prompt)
        .map_err(|e| PrepassError::AiFailed(e.to_string()))?;
    let values = parse_response(&response)
        .map_err(|e| PrepassError::ParseFailed(e))?;
    let new_content = rewrite_placeholders(&content, &placeholders, &values);
    std::fs::write(file, new_content)
        .map_err(|e| PrepassError::Io(format!("write {}: {e}", file.display())))?;
    Ok(())
}

/// build 子命令入口：重置所有 ai.auto.filled → ai.auto，然后 run。
///
/// 不执行脚本。
pub fn build_only(file: &Path, lang: Lang) -> Result<(), PrepassError> {
    let content = std::fs::read_to_string(file)
        .map_err(|e| PrepassError::Io(format!("read {}: {e}", file.display())))?;
    let reset = reset_filled(&content);
    std::fs::write(file, reset)
        .map_err(|e| PrepassError::Io(format!("write {}: {e}", file.display())))?;
    run(file, lang)
}
```

- [ ] **Step 4: 运行测试确认通过**

Run: `cargo test --test prepass_tests`
Expected: PASS

- [ ] **Step 5: 提交**

```bash
git add src/prepass.rs tests/prepass_tests.rs
git commit -m "feat(prepass): add run() and build_only() entry points"
```

---

## Task 13: main.rs + cli — build 子命令 + 预扫描集成

**Files:**
- Modify: [src/main.rs](file:///workspace/src/main.rs), [src/cli/mod.rs](file:///workspace/src/cli/mod.rs)
- Test: `tests/integration_build_subcommand.rs`

- [ ] **Step 1: 写失败测试**

Create `tests/integration_build_subcommand.rs`:

```rust
use std::process::Command;

#[test]
fn dyyl_build_subcommand_is_recognized() {
    let output = Command::new("cargo")
        .args(["run", "--", "build", "nonexistent.dyyl"])
        .output()
        .expect("spawn");
    let stderr = String::from_utf8_lossy(&output.stderr);
    // 不应出现 "unknown option"。
    assert!(!stderr.contains("unknown option"), "stderr was: {stderr}");
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test --test integration_build_subcommand`
Expected: FAIL — `unknown option 'build'`

- [ ] **Step 3: 在 cli/mod.rs 加 build 分发**

Edit [src/cli/mod.rs](file:///workspace/src/cli/mod.rs)，在 `"install" | "update" | "remove" | "autoremove" | "list"` 分支后加：

```rust
            "build" => {
                let file = match args.get(1) {
                    Some(f) => f,
                    None => {
                        eprintln!("Usage: dyyl build <filename>");
                        return CliResult::Handled(1);
                    }
                };
                let code = match crate::prepass::build_only(
                    std::path::Path::new(file),
                    *lang,
                ) {
                    Ok(()) => 0,
                    Err(e) => {
                        eprintln!("prepass failed: {e}");
                        2
                    }
                };
                return CliResult::Handled(code);
            }
```

- [ ] **Step 4: 在 main.rs run_script 前插入预扫描**

Edit [src/main.rs](file:///workspace/src/main.rs)，在 `let source = match fs::read_to_string(&filename)` 之前加：

```rust
    // 预扫描：检测未填 ai.auto → 批量请求 → 回写。
    let prepass_path = std::path::PathBuf::from(&filename);
    if let Err(e) = dyyl::prepass::run(&prepass_path, lang) {
        eprintln!(
            "dyyl: {}",
            dyyl::i18n::t(lang, "ai.prepass_failed", &[("reason", &e.to_string())])
        );
        process::exit(2);
    }
```

- [ ] **Step 5: 运行测试确认通过**

Run: `cargo test --test integration_build_subcommand`
Expected: PASS

- [ ] **Step 6: 跑现有测试套件确认无回归**

Run: `cargo test`
Expected: 现有测试全过（预扫描无 ai.auto 时直接返回 Ok）

- [ ] **Step 7: 提交**

```bash
git add src/cli/mod.rs src/main.rs tests/integration_build_subcommand.rs
git commit -m "feat(cli): add build subcommand and integrate prepass into run_script"
```

---

## Task 14: logic.end 开放块（第 1 部分：预扫描）

**Files:**
- Modify: [src/runtime/execute.rs](file:///workspace/src/runtime/execute.rs)
- Test: `tests/logic_end_tests.rs`

- [ ] **Step 1: 写失败测试**

Create `tests/logic_end_tests.rs`:

```rust
use dyyl::runtime::execute::scan_open_blocks;
use dyyl::parser::parse_source;

#[test]
fn scan_open_blocks_finds_matching_end() {
    let src = "logic.if 1, _\n  io.out a\nlogic.end\n";
    let cmds = parse_source(src).expect("parse");
    let map = scan_open_blocks(&cmds);
    assert_eq!(map.get(&0), Some(&3));
}

#[test]
fn scan_open_blocks_handles_nesting() {
    let src = "\
logic.while 1, _
  logic.if 1, _
    io.out x
  logic.end
  io.out y
logic.end
";
    let cmds = parse_source(src).expect("parse");
    let map = scan_open_blocks(&cmds);
    assert_eq!(map.get(&0), Some(&(cmds.len() - 1)));
    assert_eq!(map.get(&1), Some(&4));
}

#[test]
fn scan_open_blocks_ignores_explicit_line_counts() {
    let src = "logic.if 1, 1\n  io.out a\n";
    let cmds = parse_source(src).expect("parse");
    let map = scan_open_blocks(&cmds);
    assert!(map.is_empty(), "explicit line count should not be in open-block map");
}

#[test]
fn scan_open_blocks_mixed_explicit_and_open() {
    let src = "\
logic.if 1, 1
  io.out a
logic.while 1, _
  io.out b
logic.end
";
    let cmds = parse_source(src).expect("parse");
    let map = scan_open_blocks(&cmds);
    assert_eq!(map.get(&2), Some(&5));
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test --test logic_end_tests scan_open_blocks`
Expected: FAIL — `unresolved function scan_open_blocks`

- [ ] **Step 3: 实现 scan_open_blocks**

Edit [src/runtime/execute.rs](file:///workspace/src/runtime/execute.rs)，在文件顶部 `use` 块后加：

```rust
use std::collections::HashMap;
use crate::parser::types::Expr;

/// 扫描命令列表，为每个 `_` 开放块（logic.if/else/while/for 的行数参数为 `_`）
/// 找到对应的 `logic.end` 索引。
///
/// 返回 命令索引 -> end 索引 的映射。显式行数块不在映射中。
#[must_use]
pub fn scan_open_blocks(commands: &[ParsedCommand]) -> HashMap<usize, usize> {
    let mut map = HashMap::new();
    let mut stack: Vec<usize> = Vec::new();
    for (i, cmd) in commands.iter().enumerate() {
        match cmd.call.command.as_str() {
            "logic.if" | "logic.else" | "logic.while" | "logic.for" => {
                if is_open_block(&cmd.call) {
                    stack.push(i);
                }
            }
            "logic.end" => {
                if let Some(start) = stack.pop() {
                    map.insert(start, i);
                }
            }
            _ => {}
        }
    }
    map
}

/// 判断 logic.if/else/while/for 的行数参数是否是 `_`（开放块）。
fn is_open_block(call: &crate::parser::types::Call) -> bool {
    matches!(call.args.get(1), Some(Expr::Empty))
}
```

- [ ] **Step 4: 运行测试确认通过**

Run: `cargo test --test logic_end_tests scan_open_blocks`
Expected: PASS（4 个测试）

- [ ] **Step 5: 提交**

```bash
git add src/runtime/execute.rs tests/logic_end_tests.rs
git commit -m "feat(logic): add open-block pre-scan for logic.end"
```

---

## Task 15: logic.end 开放块（第 2 部分：执行集成）

**Files:**
- Modify: [src/runtime/exec_block.rs](file:///workspace/src/runtime/exec_block.rs), [src/runtime/execute.rs](file:///workspace/src/runtime/execute.rs)
- Test: `tests/logic_end_tests.rs`

- [ ] **Step 1: 写失败测试**

Append to `tests/logic_end_tests.rs`:

```rust
use dyyl::runtime::execute::run_script;
use dyyl::runtime::Value;

#[test]
fn logic_if_open_block_executes_body() {
    let v = run_script(
        "create.num x\nset $x, 0\nlogic.if 1, _\n  set $x, 42\nlogic.end\nio.out $x\n",
        false,
    ).values;
    assert_eq!(v[3], Value::Num(1), "if true returns 1");
    assert_eq!(v[5], Value::Num(42), "body executed");
}

#[test]
fn logic_if_open_block_false_skips_body() {
    let v = run_script(
        "create.num x\nset $x, 0\nlogic.if 0, _\n  set $x, 99\nlogic.end\nio.out $x\n",
        false,
    ).values;
    assert_eq!(v[3], Value::Num(0), "if false returns 0");
    assert_eq!(v[5], Value::Num(0), "body skipped");
}

#[test]
fn logic_while_open_block_loops() {
    let v = run_script(
        "create.num i\nset $i, 0\nlogic.while logic.less($i, 3), _\n  set $i, math.add($i, 1)\nlogic.end\nio.out $i\n",
        false,
    ).values;
    assert_eq!(v[5], Value::Num(3), "while ran 3 times");
}

#[test]
fn logic_for_open_block_loops() {
    let v = run_script(
        "create.num sum\nset $sum, 0\nlogic.for 3, _\n  set $sum, math.add($sum, 1)\nlogic.end\nio.out $sum\n",
        false,
    ).values;
    assert_eq!(v[5], Value::Num(3), "for ran 3 times");
}

#[test]
fn logic_nested_open_blocks() {
    let v = run_script(
        "\
create.num i
set $i, 0
logic.while logic.less($i, 2), _
  logic.if logic.same($i, 1), _
    set $i, math.add($i, 10)
  logic.end
  set $i, math.add($i, 1)
logic.end
io.out $i
",
        false,
    ).values;
    // i=0: if false, i=1; i=1: if true i=11, then i=12; 12>=2 stop
    assert_eq!(v[10], Value::Num(12), "nested open blocks work");
}

#[test]
fn logic_end_without_open_block_returns_sentinel() {
    let v = run_script("logic.end\n", false).values;
    assert_eq!(v[0], Value::Num(0), "logic.end without open block returns 0");
}

#[test]
fn mixed_explicit_and_open_blocks() {
    let v = run_script(
        "\
create.num x
set $x, 1
logic.if 1, 1
  set $x, 2
logic.while logic.less($x, 5), _
  set $x, math.add($x, 1)
logic.end
io.out $x
",
        false,
    ).values;
    assert_eq!(v[7], Value::Num(5), "mixed blocks work");
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test --test logic_end_tests`
Expected: FAIL（现有 exec_block 不识别 `_` 开放块）

- [ ] **Step 3: 修改 exec_block.rs 支持 `_` 开放块**

读 [src/runtime/exec_block.rs](file:///workspace/src/runtime/exec_block.rs) 全文，理解 `exec_block_cmd` 的现有签名和 `body_lines` 计算。

修改 `exec_block_cmd` 函数签名加 `open_blocks: &HashMap<usize, usize>` 参数，并改 `body_lines` 计算：

找到 `body_lines` 的计算（通常是 `let body_lines = ...`），替换为：

```rust
    let body_lines = match cmd.call.args.get(1) {
        Some(Expr::Num(n)) if *n >= 0 => *n as usize,
        Some(Expr::Empty) => {
            // 开放块：查预扫描结果。
            match open_blocks.get(&i) {
                Some(end_idx) => end_idx.saturating_sub(i + 1),
                None => 0,
            }
        }
        _ => 0,
    };
```

在 exec_block.rs 顶部 `use` 块加：

```rust
use std::collections::HashMap;
use crate::parser::types::Expr;
```

- [ ] **Step 4: 修改 execute.rs 传递 open_blocks**

Edit [src/runtime/execute.rs](file:///workspace/src/runtime/execute.rs)：

1. 在 `exec_commands_range` 开头计算 `open_blocks` 并传给 `exec_one_command`：

```rust
fn exec_commands_range(
    commands: &[ParsedCommand],
    start: usize,
    count: usize,
    env: &mut Env,
    values: &mut Vec<Value>,
    debug: bool,
    io_provider: &Arc<dyn IoProvider>,
) -> usize {
    let total = commands.len();
    let end = total.min(start + count);
    let open_blocks = scan_open_blocks(commands);
    let mut i = start;
    let mut prev_if_was_false = false;

    while i < end {
        let cmd = &commands[i];
        let consumed = exec_one_command(
            cmd,
            env,
            values,
            &mut prev_if_was_false,
            commands,
            i,
            end,
            debug,
            io_provider,
            &open_blocks,
        );
        i += consumed;
    }

    i - start
}
```

2. 同样修改 `exec_commands_range_with_error`（加 `let open_blocks = scan_open_blocks(commands);` 并传给 `exec_one_command_with_error`）。

3. 修改 `exec_one_command` 和 `exec_one_command_with_error` 签名加 `open_blocks: &HashMap<usize, usize>`，传给 `exec_block::exec_block_cmd`。

4. 在 `exec_one_command` 的 match 加 `logic.end` 分支：

```rust
        "logic.end" => {
            let is_matched = open_blocks.values().any(|&end_idx| end_idx == i);
            if !is_matched && debug {
                eprintln!(
                    "line {}: {}",
                    cmd.line,
                    crate::i18n::t(env.lang(), "logic.end_without_open", &[])
                );
            }
            values.push(Value::Num(if is_matched { 1 } else { 0 }));
            1
        }
```

注意：`exec_one_command` 的现有 match 可能是 `match cmd.call.command.as_str() { "logic.if" | ... => exec_block_cmd(...), _ => ... }`。把 `logic.end` 加为单独分支：

```rust
    match cmd.call.command.as_str() {
        "logic.if" | "logic.else" | "logic.while" | "logic.for" => exec_block::exec_block_cmd(
            cmd, env, values, prev_if_was_false, commands, i, end, debug, io_provider, open_blocks,
        ),
        "logic.end" => {
            let is_matched = open_blocks.values().any(|&end_idx| end_idx == i);
            if !is_matched && debug {
                eprintln!(
                    "line {}: {}",
                    cmd.line,
                    crate::i18n::t(env.lang(), "logic.end_without_open", &[])
                );
            }
            values.push(Value::Num(if is_matched { 1 } else { 0 }));
            1
        }
        _ => {
            let ctx = ExecContext::from_command(cmd, debug, Arc::clone(io_provider), env.lang());
            let result = dispatch_call(&cmd.call, env, &ctx);
            push_result(result, cmd, values, debug, env.lang());
            1
        }
    }
```

- [ ] **Step 5: 运行测试确认通过**

Run: `cargo test --test logic_end_tests`
Expected: PASS

- [ ] **Step 6: 跑现有 logic 测试确认无回归**

Run: `cargo test --test logic_tests --test logic_control_flow_tests --test logic_combined_tests`
Expected: PASS

- [ ] **Step 7: 跑全测试套件**

Run: `cargo test`
Expected: PASS

- [ ] **Step 8: 提交**

```bash
git add src/runtime/exec_block.rs src/runtime/execute.rs tests/logic_end_tests.rs
git commit -m "feat(logic): add logic.end open-block execution with stack-based nesting"
```

---

## Task 16: 集成测试 — 端到端 ai.auto 填值

**Files:**
- Create: `tests/fixtures/mock_ai_server.rs`
- Create: `tests/ai_integration_tests.rs`

- [ ] **Step 1: 创建 mock AI server 辅助**

Create `tests/fixtures/mod.rs`:

```rust
pub mod mock_ai_server;
```

Create `tests/fixtures/mock_ai_server.rs`:

```rust
//! Mock AI HTTP server for testing.

use std::sync::Arc;
use std::sync::Mutex;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

pub struct MockAiServer {
    pub port: u16,
    pub requests: Arc<Mutex<Vec<MockRequest>>>,
    shutdown: Arc<tokio::sync::Notify>,
}

#[derive(Clone, Debug)]
pub struct MockRequest {
    pub path: String,
    pub body: String,
}

impl MockAiServer {
    pub async fn start(response_body: String) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let port = listener.local_addr().expect("addr").port();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let shutdown = Arc::new(tokio::sync::Notify::new());
        let requests_clone = Arc::clone(&requests);
        let shutdown_clone = Arc::clone(&shutdown);
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    accept = listener.accept() => {
                        let (mut sock, _) = match accept {
                            Ok(s) => s,
                            Err(_) => continue,
                        };
                        let requests_clone = Arc::clone(&requests_clone);
                        let response_body = response_body.clone();
                        tokio::spawn(async move {
                            let mut buf = vec![0u8; 8192];
                            let n = sock.read(&mut buf).await.unwrap_or(0);
                            let raw = String::from_utf8_lossy(&buf[..n]).to_string();
                            let (path, body) = parse_http_request(&raw);
                            requests_clone.lock().unwrap().push(MockRequest { path, body });
                            let resp = format!(
                                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                                response_body.len(),
                                response_body
                            );
                            let _ = sock.write_all(resp.as_bytes()).await;
                        });
                    }
                    _ = shutdown_clone.notified() => break,
                }
            }
        });
        Self { port, requests, shutdown }
    }

    pub fn stop(&self) {
        self.shutdown.notify_waiters();
    }

    pub fn captured_requests(&self) -> Vec<MockRequest> {
        self.requests.lock().unwrap().clone()
    }
}

fn parse_http_request(raw: &str) -> (String, String) {
    let mut lines = raw.split("\r\n");
    let first_line = lines.next().unwrap_or("");
    let path = first_line.split_whitespace().nth(1).unwrap_or("").to_string();
    let mut body = String::new();
    let mut in_body = false;
    for line in lines {
        if in_body {
            body.push_str(line);
        } else if line.is_empty() {
            in_body = true;
        }
    }
    (path, body)
}
```

- [ ] **Step 2: 写端到端测试**

Create `tests/ai_integration_tests.rs`:

```rust
mod fixtures;
use fixtures::mock_ai_server::MockAiServer;
use std::fs;
use tempfile::tempdir;

#[tokio::test]
async fn ai_auto_filled_via_prepass_with_mock_server() {
    let response = r#"{"1":{"type":"string","value":"Steve"},"2":{"type":"number","value":25565}}"#;
    let server = MockAiServer::start(response.to_string()).await;
    let dir = tempdir().unwrap();
    let creds_path = dir.path().join("credentials.toml");
    fs::write(&creds_path, format!(
        "[ai]\nprovider = \"openai-chat\"\napi_key = \"sk-test\"\nmodel = \"gpt-4o-mini\"\nbase_url = \"http://127.0.0.1:{}\"\n",
        server.port
    )).unwrap();
    let script_path = dir.path().join("script.dyyl");
    fs::write(&script_path, "set $name, ai.auto \"用户名\"\nset $port, ai.auto \"端口\"\n").unwrap();
    std::env::set_var("DYYL_CREDENTIALS_PATH", &creds_path);
    let result = dyyl::prepass::run(&script_path, dyyl::i18n::Lang::En);
    std::env::remove_var("DYYL_CREDENTIALS_PATH");
    assert!(result.is_ok(), "prepass should succeed: {:?}", result);
    let filled = fs::read_to_string(&script_path).unwrap();
    assert!(filled.contains("ai.auto.filled \"用户名\", \"Steve\""), "got: {filled}");
    assert!(filled.contains("ai.auto.filled \"端口\", 25565"), "got: {filled}");
    server.stop();
}

#[tokio::test]
async fn dyyl_build_resets_and_refills() {
    let response = r#"{"1":{"type":"number","value":100}}"#;
    let server = MockAiServer::start(response.to_string()).await;
    let dir = tempdir().unwrap();
    let creds_path = dir.path().join("credentials.toml");
    fs::write(&creds_path, format!(
        "[ai]\nprovider = \"openai-chat\"\napi_key = \"sk-test\"\nmodel = \"gpt-4o-mini\"\nbase_url = \"http://127.0.0.1:{}\"\n",
        server.port
    )).unwrap();
    let script_path = dir.path().join("script.dyyl");
    fs::write(&script_path, "set $x, ai.auto.filled \"hint\", 42\n").unwrap();
    std::env::set_var("DYYL_CREDENTIALS_PATH", &creds_path);
    let result = dyyl::prepass::build_only(&script_path, dyyl::i18n::Lang::En);
    std::env::remove_var("DYYL_CREDENTIALS_PATH");
    assert!(result.is_ok());
    let filled = fs::read_to_string(&script_path).unwrap();
    assert!(filled.contains("ai.auto.filled \"hint\", 100"), "should be refilled with 100, got: {filled}");
    server.stop();
}
```

- [ ] **Step 3: 运行测试确认通过**

Run: `cargo test --test ai_integration_tests`
Expected: PASS

- [ ] **Step 4: 提交**

```bash
git add tests/fixtures/ tests/ai_integration_tests.rs
git commit -m "test(ai): add end-to-end ai.auto fill tests with mock HTTP server"
```

---

## Task 17: manifest.rs — credentials 段

**Files:**
- Modify: [src/runtime/plugin/manifest.rs](file:///workspace/src/runtime/plugin/manifest.rs)
- Test: `tests/plugin_credentials_tests.rs`

- [ ] **Step 1: 写失败测试**

Create `tests/plugin_credentials_tests.rs`:

```rust
use dyyl::runtime::plugin::manifest::RemoteManifest;

#[test]
fn parse_manifest_with_credentials() {
    let json = r#"{
        "name": "migpt",
        "version": "0.1.0",
        "abi_version": 2,
        "dyyl_min": "0.2.0",
        "platforms": [{"platform": "linux-x86_64", "url": "http://x", "sha256": "abc"}],
        "credentials": {
            "fields": [
                {"name": "token", "type": "string", "secret": true, "description": "GitHub PAT"},
                {"name": "repo", "type": "string", "secret": false, "description": "Default repo"}
            ]
        }
    }"#;
    let m: RemoteManifest = serde_json::from_str(json).expect("parse");
    assert_eq!(m.name, "migpt");
    let creds = m.credentials.expect("credentials present");
    assert_eq!(creds.fields.len(), 2);
    assert_eq!(creds.fields[0].name, "token");
    assert!(creds.fields[0].secret);
    assert_eq!(creds.fields[1].name, "repo");
    assert!(!creds.fields[1].secret);
}

#[test]
fn parse_manifest_without_credentials() {
    let json = r#"{
        "name": "simple",
        "version": "0.1.0",
        "abi_version": 2,
        "dyyl_min": "0.2.0",
        "platforms": [{"platform": "linux-x86_64", "url": "http://x", "sha256": "abc"}]
    }"#;
    let m: RemoteManifest = serde_json::from_str(json).expect("parse");
    assert!(m.credentials.is_none());
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test --test plugin_credentials_tests`
Expected: FAIL — `no field credentials on type RemoteManifest`

- [ ] **Step 3: 扩展 manifest.rs**

Edit [src/runtime/plugin/manifest.rs](file:///workspace/src/runtime/plugin/manifest.rs)，在文件中加新结构：

```rust
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
```

在 `RemoteManifest` struct 加字段：

```rust
    /// 可选：插件声明需要的凭证字段。
    #[serde(default)]
    pub credentials: Option<CredentialsSpec>,
```

在 `LocalPluginToml` struct 加同样字段：

```rust
    #[serde(default)]
    pub credentials: Option<CredentialsSpec>,
```

- [ ] **Step 4: 运行测试确认通过**

Run: `cargo test --test plugin_credentials_tests`
Expected: PASS

- [ ] **Step 5: 提交**

```bash
git add src/runtime/plugin/manifest.rs tests/plugin_credentials_tests.rs
git commit -m "feat(plugin): add credentials spec to plugin manifest"
```

---

## Task 18: abi.rs — ABI v2 + loader.rs 凭证注入

**Files:**
- Modify: [src/runtime/plugin/abi.rs](file:///workspace/src/runtime/plugin/abi.rs), [src/runtime/plugin/loader.rs](file:///workspace/src/runtime/plugin/loader.rs), [tests/fixtures/example-plugin/src/lib.rs](file:///workspace/tests/fixtures/example-plugin/src/lib.rs)
- Test: `tests/plugin_credentials_tests.rs`

- [ ] **Step 1: 写失败测试**

Append to `tests/plugin_credentials_tests.rs`:

```rust
use dyyl::runtime::plugin::abi::{required_symbol_names, DYRL_API_VERSION};

#[test]
fn abi_version_is_2() {
    assert_eq!(DYRL_API_VERSION, 2);
}

#[test]
fn required_symbols_include_set_credentials() {
    let names = required_symbol_names();
    assert!(names.contains(&"dyyl_plugin_set_credentials"));
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test --test plugin_credentials_tests abi`
Expected: FAIL — `DYRL_API_VERSION` 是 1

- [ ] **Step 3: 读 abi.rs 现有结构**

读 [src/runtime/plugin/abi.rs](file:///workspace/src/runtime/plugin/abi.rs) 全文，理解：
- `DYRL_API_VERSION` 常量
- `symbols` 模块内的类型别名
- `required_symbol_names` 函数返回数组大小
- `AbiError` 枚举变体

- [ ] **Step 4: 修改 abi.rs**

1. 改 `DYRL_API_VERSION`：
```rust
pub const DYRL_API_VERSION: u32 = 2;
```

2. 在 `symbols` 模块加新类型别名：
```rust
    pub type SetCredentials = unsafe extern "C" fn(PluginHandle, *const c_char) -> c_int;
```

3. 改 `required_symbol_names` 返回 15 个（加 `"dyyl_plugin_set_credentials"`）：

找到现有 `required_symbol_names` 函数，在数组末尾加 `"dyyl_plugin_set_credentials"`，并把返回类型数组大小从 14 改为 15。

4. 在 `AbiError` 枚举加变体：
```rust
    /// set_credentials 返回非 0。
    SetCredentialsFailed(i32),
```

5. 在 `Display` impl 加：
```rust
            Self::SetCredentialsFailed(c) => write!(f, "set_credentials() failed with code {c}"),
```

- [ ] **Step 5: 修改 loader.rs 支持 v1+v2**

Edit [src/runtime/plugin/loader.rs](file:///workspace/src/runtime/plugin/loader.rs)，在 `load` 函数改版本检查。

找到：
```rust
            if plugin_api_version != DYRL_API_VERSION {
                std::mem::drop(library);
                return Err(AbiError::SymbolMissing(format!(
                    "API version mismatch: plugin={plugin_api_version}, dyyl={DYRL_API_VERSION}"
                )));
            }
```

替换为：
```rust
            // 支持 v1 和 v2 插件。v2 才有 set_credentials。
            if plugin_api_version != 1 && plugin_api_version != DYRL_API_VERSION {
                std::mem::drop(library);
                return Err(AbiError::SymbolMissing(format!(
                    "API version mismatch: plugin={plugin_api_version}, dyyl supports 1 and {DYRL_API_VERSION}"
                )));
            }
            let is_v2 = plugin_api_version >= 2;
```

在 `on_load` 调用之前（init 之后），加 set_credentials 注入：

```rust
            // 3. set_credentials（仅 v2 插件，若有 credentials 声明）。
            if is_v2 {
                if let Ok(set_creds) = library.get::<symbols::SetCredentials>(b"dyyl_plugin_set_credentials\0")
                {
                    let set_creds = *set_creds;
                    // credentials 注入由 PluginManager 在 load 后单独调用
                    // （因为需要 manifest + credentials.toml）。
                    // 这里仅保存 set_creds 函数指针供后续调用。
                    // 简化：loader 不直接调，由上层 manager 调。
                    // 但 loader 的 API 是 load(path, name)，无法传 credentials。
                    // 修改：load 接受可选 credentials_json 参数。
                    // 此处先不调用，保留 is_v2 标记。
                    let _ = set_creds;
                }
            }
```

注：为了让 loader 能注入凭证，需修改 `load` 签名加 `credentials_json: Option<&str>` 参数。

- [ ] **Step 6: 修改 loader load 签名加 credentials_json**

把 `load` 签名改为：

```rust
    pub fn load(path: &Path, plugin_name: &str, credentials_json: Option<&str>) -> Result<Self, AbiError> {
```

在 init 之后、on_load 之前加：

```rust
            // 3. set_credentials（仅当插件是 v2 且传入了 credentials_json）。
            if let Some(json) = credentials_json {
                let set_creds: symbols::SetCredentials = *library
                    .get(b"dyyl_plugin_set_credentials\0")
                    .map_err(|_| AbiError::SymbolMissing("dyyl_plugin_set_credentials".to_string()))?;
                let json_c = CString::new(json).map_err(|_| AbiError::InvalidUtf8)?;
                let rc = set_creds(handle, json_c.as_ptr());
                if rc != 0 {
                    std::mem::drop(library);
                    return Err(AbiError::SetCredentialsFailed(rc));
                }
            }
```

- [ ] **Step 7: 更新所有 load 调用点**

Run: `grep -rn "PluginLoader::load" src/ tests/`
找到所有调用 `PluginLoader::load(path, name)` 的地方，加第三个参数 `None`（或实际的 credentials_json）。

通常在 [src/runtime/plugin/manager.rs](file:///workspace/src/runtime/plugin/manager.rs) 或类似文件。每个调用加 `None`：

```rust
PluginLoader::load(path, name, None)
```

或如果 manager 已加载 manifest 和 credentials，传实际 JSON：

```rust
let creds_json = if let Some(spec) = &manifest.credentials {
    // 构造 JSON。
    let mut map = serde_json::Map::new();
    if let Some(plugin_creds) = credentials_file.plugins.get(name) {
        for field in &spec.fields {
            if let Some(v) = plugin_creds.get(&field.name) {
                map.insert(field.name.clone(), serde_json::Value::String(v.clone()));
            }
        }
    }
    Some(serde_json::Value::Object(map).to_string())
} else {
    None
};
PluginLoader::load(path, name, creds_json.as_deref())
```

注：完整凭证预检 + 交互提示补齐在 manager 层实现，此处仅传已加载的 JSON。Manager 层的凭证预检逻辑：

1. 读 manifest.credentials.fields
2. 读 credentials.toml [plugin.<name>]
3. 比对缺失字段
4. 缺失则交互提示（类似 ensure_ai）
5. 全有后构造 JSON 传给 load

由于 manager 层改动较大，先确保现有调用传 `None` 能编译通过，凭证预检逻辑可作为后续增强（或在此 Task 内完整实现）。

- [ ] **Step 8: 更新 fixture 插件到 v2**

读 [tests/fixtures/example-plugin/src/lib.rs](file:///workspace/tests/fixtures/example-plugin/src/lib.rs)，改 `get_api_version` 返回 2，加 `set_credentials` 空实现：

```rust
#[no_mangle]
pub extern "C" fn dyyl_plugin_get_api_version() -> u32 {
    2
}

#[no_mangle]
pub extern "C" fn dyyl_plugin_set_credentials(
    _handle: *mut std::ffi::c_void,
    _creds_json: *const std::os::raw::c_char,
) -> std::os::raw::c_int {
    0
}
```

- [ ] **Step 9: 运行测试确认通过**

Run: `cargo test --test plugin_credentials_tests abi`
Expected: PASS

- [ ] **Step 10: 跑全测试套件**

Run: `cargo test`
Expected: PASS（fixture 升级到 v2，load 调用传 None 兼容）

- [ ] **Step 11: 提交**

```bash
git add src/runtime/plugin/abi.rs src/runtime/plugin/loader.rs src/runtime/plugin/manager.rs tests/fixtures/example-plugin/src/lib.rs tests/plugin_credentials_tests.rs
git commit -m "feat(plugin): bump ABI to v2, add set_credentials, support v1+v2 plugins"
```

---

## 完成检查清单

实现全部 18 个 Task 后，确认：

- [ ] `cargo test` 全过
- [ ] `cargo clippy --all-targets --all-features` 无 deny 级警告
- [ ] `cargo fmt --check` 通过
- [ ] 手动验证：`dyyl build <file>` 能重置 + 重新填 ai.auto
- [ ] 手动验证：`dyyl <file>` 含 ai.auto 时预扫描填值后执行
- [ ] 手动验证：`ai.ask` 运行时请求 AI 返回字符串
- [ ] 手动验证：`logic.end` 关闭 `_` 开放块，支持嵌套
- [ ] 手动验证：凭证缺失时交互式提示用户输入
- [ ] 文档：dyyl-api-reference.md 加 ai/credentials/logic.end 章节（如存在文档）
