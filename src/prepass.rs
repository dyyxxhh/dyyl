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
        // consumed_after 相对于 trim 后的 rest；补回前导空白以正确切片 after。
        let leading = after.len() - rest.len();
        let (hint, consumed_after) = parse_hint(rest);
        let original_text = format!("ai.auto{}", &after[..leading + consumed_after]);
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
    let first_byte = rest.as_bytes().first().copied();
    // 引号提示：绑定到 quote_byte，避免 unwrap。
    if let Some(quote_byte @ (b'"' | b'\'')) = first_byte {
        let quote = quote_byte as char;
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
