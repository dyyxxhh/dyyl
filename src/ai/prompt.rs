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
    let v: serde_json::Value =
        serde_json::from_str(json_str).map_err(|e| format!("invalid JSON: {e}"))?;
    let obj = v
        .as_object()
        .ok_or_else(|| "response is not a JSON object".to_string())?;
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
