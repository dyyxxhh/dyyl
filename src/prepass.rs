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
