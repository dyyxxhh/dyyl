//! Bilingual internationalization (English / Chinese).
//!
//! Every user-facing message — runtime errors, debug warnings, CLI output —
//! routes through this module so the interpreter can switch languages at
//! runtime via the `language` command.

// ── Language enum ────────────────────────────────────────────────────

/// Supported UI languages.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lang {
    En,
    Zh,
}

impl Default for Lang {
    fn default() -> Self {
        Self::En
    }
}

impl Lang {
    /// Parse a language name from user input.
    #[must_use]
    pub fn from_name(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "en" | "english" => Some(Self::En),
            "zh" | "chinese" | "中文" => Some(Self::Zh),
            _ => None,
        }
    }

    /// Return the short code for this language.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::En => "en",
            Self::Zh => "zh",
        }
    }
}

// ── Message store (key-value table backed by JSON resources) ───────

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

/// Plugin-registered message tables.
#[derive(Default)]
pub struct PluginMessages {
    en: HashMap<String, String>,
    zh: HashMap<String, String>,
}

/// Central message store. Holds dyyl's own en/zh tables (compiled in)
/// plus plugin-registered tables.
pub struct MessageStore {
    en: HashMap<String, String>,
    zh: HashMap<String, String>,
    plugins: Mutex<HashMap<String, PluginMessages>>,
}

static STORE: OnceLock<MessageStore> = OnceLock::new();

const EN_JSON: &str = include_str!("../locales/en.json");
const ZH_JSON: &str = include_str!("../locales/zh.json");

fn parse_json(json: &str) -> HashMap<String, String> {
    serde_json::from_str(json).unwrap_or_default()
}

fn init_store() -> MessageStore {
    MessageStore {
        en: parse_json(EN_JSON),
        zh: parse_json(ZH_JSON),
        plugins: Mutex::new(HashMap::new()),
    }
}

fn store() -> &'static MessageStore {
    STORE.get_or_init(init_store)
}

/// Look up a message by key, interpolate `{placeholder}` args, and return
/// the rendered string. Falls back zh→en if the zh key is missing (emits a
/// stderr warning). If neither language has the key, returns the key name
/// itself with a warning.
#[must_use]
pub fn t(lang: Lang, key: &str, args: &[(&str, &str)]) -> String {
    let s = store();
    // Determine which plugin table to check first (if key starts with "<plugin>.")
    let plugin_name = key.split('.').next().unwrap_or(key);
    let template = s.lookup_template(lang, key, plugin_name);
    interpolate(&template, args)
}

/// Register a plugin's message tables. Called by PluginManager when loading
/// a plugin that ships locale files.
pub fn register_plugin(
    name: &str,
    en: HashMap<String, String>,
    zh: HashMap<String, String>,
) {
    let s = store();
    let mut plugins = s.plugins.lock().expect("plugins mutex poisoned");
    plugins.insert(name.to_string(), PluginMessages { en, zh });
}

/// Return all keys in the dyyl main table (not plugin keys).
/// Used for coverage testing.
#[must_use]
pub fn all_keys() -> Vec<&'static str> {
    let s = store();
    s.en.keys().map(String::as_str).collect()
}

/// Return keys present in en but missing in the given language's main table.
/// For Zh, this checks zh.json. For En, always empty (en is the source).
/// Must return empty in CI — serves as coverage gate.
#[must_use]
pub fn missing_translations(lang: Lang) -> Vec<&'static str> {
    let s = store();
    match lang {
        Lang::En => Vec::new(),
        Lang::Zh => s
            .en
            .keys()
            .filter(|k| !s.zh.contains_key(*k))
            .map(String::as_str)
            .collect(),
    }
}

impl MessageStore {
    fn lookup_template(&self, lang: Lang, key: &str, plugin_name: &str) -> String {
        // 1. Plugin table (if this plugin is registered and key starts with plugin name)
        if key.starts_with(&format!("{plugin_name}.")) {
            let plugins = self.plugins.lock().expect("plugins mutex poisoned");
            if let Some(pm) = plugins.get(plugin_name) {
                if let Some(tpl) = match lang {
                    Lang::En => pm.en.get(key),
                    Lang::Zh => pm.zh.get(key).or_else(|| pm.en.get(key)),
                } {
                    return tpl.clone();
                }
            }
        }
        // 2. dyyl main table
        let main_tpl = match lang {
            Lang::En => self.en.get(key),
            Lang::Zh => self.zh.get(key).or_else(|| {
                eprintln!("i18n warning: zh translation missing for '{key}', falling back to en");
                self.en.get(key)
            }),
        };
        match main_tpl {
            Some(tpl) => tpl.clone(),
            None => {
                eprintln!("i18n warning: no translation found for '{key}'");
                key.to_string()
            }
        }
    }
}

/// Replace `{placeholder}` occurrences in template with provided args.
/// Unknown placeholders are left as-is.
fn interpolate(template: &str, args: &[(&str, &str)]) -> String {
    let mut result = template.to_string();
    for (name, value) in args {
        let placeholder = format!("{{{name}}}");
        result = result.replace(&placeholder, value);
    }
    result
}

// ── Debug prefix helper ──────────────────────────────────────────────

/// Return the "  reason: " / "  原因: " prefix for debug diagnostics.
#[must_use]
pub fn reason_prefix(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "  reason: ",
        Lang::Zh => "  原因: ",
    }
}

// ── Command family name translation ──────────────────────────────────

/// Translate a command family keyword for zh output.
fn zh_family(family: &str) -> &'static str {
    match family {
        "dict" => "字典",
        "list" => "列表",
        "file" => "文件",
        "net" => "网络",
        "user" => "用户",
        "system" => "系统",
        "time" => "时间",
        "str" => "字符串",
        "str.basic" => "字符串.基础",
        "str.modify" => "字符串.修改",
        "str.convert" => "字符串.转换",
        "str.regex" => "字符串.正则",
        "str.split/join" => "字符串.分割/合并",
        "math" => "数学",
        "logic" => "逻辑",
        _ => "未知",
    }
}

// ── CLI messages (main.rs) ───────────────────────────────────────────

/// "dyyl 0.2.0 — script interpreter" / "dyyl 0.2.0 — 脚本解释器"
#[must_use]
pub fn cli_version_banner(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "dyyl 0.2.0 — script interpreter",
        Lang::Zh => "dyyl 0.2.0 — 脚本解释器",
    }
}

/// "Usage: dyyl [--debug] <filename>" / "用法: dyyl [--debug] <文件名>"
#[must_use]
pub fn cli_usage(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "Usage: dyyl [--debug] <filename>",
        Lang::Zh => "用法: dyyl [--debug] <文件名>",
    }
}

/// "dyyl: cannot read '{path}': {e}" / "dyyl: 无法读取 '{path}': {e}"
#[must_use]
pub fn cli_cannot_read(lang: Lang, path: &str, e: &dyn std::fmt::Display) -> String {
    match lang {
        Lang::En => format!("dyyl: cannot read '{path}': {e}"),
        Lang::Zh => format!("dyyl: 无法读取 '{path}': {e}"),
    }
}

// ── Generic runtime error patterns ───────────────────────────────────

/// "unknown {family} command '{sub}'"
#[must_use]
pub fn unknown_command(lang: Lang, family: &str, sub: &str) -> String {
    match lang {
        Lang::En => format!("unknown {family} command '{sub}'"),
        Lang::Zh => format!("未知{}命令 '{sub}'", zh_family(family)),
    }
}

/// "unknown command '{cmd}'" (top-level, no family prefix)
#[must_use]
pub fn unknown_top_command(lang: Lang, cmd: &str) -> String {
    match lang {
        Lang::En => format!("unknown command '{cmd}'"),
        Lang::Zh => format!("未知命令 '{cmd}'"),
    }
}

/// "undefined variable '{name}'"
#[must_use]
pub fn undefined_variable(lang: Lang, name: &str) -> String {
    match lang {
        Lang::En => format!("undefined variable '{name}'"),
        Lang::Zh => format!("未定义变量 '{name}'"),
    }
}

/// "division by zero"
#[must_use]
pub fn division_by_zero(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "division by zero",
        Lang::Zh => "除以零",
    }
}

/// "requires at least {n} arg(s)"
#[must_use]
pub fn requires_args(lang: Lang, n: usize) -> String {
    match lang {
        Lang::En => format!("requires at least {n} arg(s)"),
        Lang::Zh => format!("至少需要 {n} 个参数"),
    }
}

/// "requires {n} arguments" (exact count)
#[must_use]
pub fn requires_n_args(lang: Lang, n: usize) -> String {
    match lang {
        Lang::En => format!("requires {n} arguments"),
        Lang::Zh => format!("需要 {n} 个参数"),
    }
}

/// "expected string value, got {val:?}"
#[must_use]
pub fn expected_string(lang: Lang, val: &crate::runtime::value::Value) -> String {
    match lang {
        Lang::En => format!("expected string value, got {val:?}"),
        Lang::Zh => format!("期望字符串值，得到 {val:?}"),
    }
}

/// "expected numeric value, got '{s}'"
#[must_use]
pub fn expected_numeric_str(lang: Lang, s: &str) -> String {
    match lang {
        Lang::En => format!("expected numeric value, got '{s}'"),
        Lang::Zh => format!("期望数值，得到 '{s}'"),
    }
}

/// "expected numeric value, got {val:?}"
#[must_use]
pub fn expected_numeric(lang: Lang, val: &crate::runtime::value::Value) -> String {
    match lang {
        Lang::En => format!("expected numeric value, got {val:?}"),
        Lang::Zh => format!("期望数值，得到 {val:?}"),
    }
}

/// "expected variable name, got {expr:?}"
#[must_use]
pub fn expected_var_name(lang: Lang, expr: &crate::parser::types::Expr) -> String {
    match lang {
        Lang::En => format!("expected variable name, got {expr:?}"),
        Lang::Zh => format!("期望变量名，得到 {expr:?}"),
    }
}

/// "expected variable reference, got {expr:?}"
#[must_use]
pub fn expected_var_ref(lang: Lang, expr: &crate::parser::types::Expr) -> String {
    match lang {
        Lang::En => format!("expected variable reference, got {expr:?}"),
        Lang::Zh => format!("期望变量引用，得到 {expr:?}"),
    }
}

/// "expected numeric, got {val:?}"
#[must_use]
pub fn expected_numeric_any(lang: Lang, val: &crate::runtime::value::Value) -> String {
    match lang {
        Lang::En => format!("expected numeric, got {val:?}"),
        Lang::Zh => format!("期望数值，得到 {val:?}"),
    }
}

/// "expected numeric index, got {val:?}"
#[must_use]
pub fn expected_numeric_index(lang: Lang, val: &crate::runtime::value::Value) -> String {
    match lang {
        Lang::En => format!("expected numeric index, got {val:?}"),
        Lang::Zh => format!("期望数值索引，得到 {val:?}"),
    }
}

/// "index must be non-negative, got {n}"
#[must_use]
pub fn index_must_be_nonnegative(lang: Lang, n: i64) -> String {
    match lang {
        Lang::En => format!("index must be non-negative, got {n}"),
        Lang::Zh => format!("索引必须非负，得到 {n}"),
    }
}

/// "index must be numeric"
#[must_use]
pub fn index_must_be_numeric(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "index must be numeric",
        Lang::Zh => "索引必须是数值",
    }
}

/// "first argument must be a {kind}"
#[must_use]
pub fn first_arg_must_be(lang: Lang, kind: &str) -> String {
    let zh_kind = match kind {
        "dict" => "字典",
        "list" => "列表",
        _ => kind,
    };
    match lang {
        Lang::En => format!("first argument must be a {kind}"),
        Lang::Zh => format!("第一个参数必须是 {zh_kind}"),
    }
}

/// "argument must be a {kind}"
#[must_use]
pub fn argument_must_be(lang: Lang, kind: &str) -> String {
    let zh_kind = match kind {
        "dict" => "字典",
        "list" => "列表",
        _ => kind,
    };
    match lang {
        Lang::En => format!("argument must be a {kind}"),
        Lang::Zh => format!("参数必须是 {zh_kind}"),
    }
}

/// "{cmd} requires a variable name"
#[must_use]
pub fn cmd_requires_var(lang: Lang, cmd: &str) -> String {
    match lang {
        Lang::En => format!("{cmd} requires a variable name"),
        Lang::Zh => format!("{cmd} 需要变量名"),
    }
}

/// "{cmd} requires a {kind} argument"
#[must_use]
pub fn cmd_requires_container(lang: Lang, cmd: &str, kind: &str) -> String {
    let zh_kind = match kind {
        "dict" => "字典",
        "list" => "列表",
        _ => kind,
    };
    match lang {
        Lang::En => format!("{cmd} requires a {kind} argument"),
        Lang::Zh => format!("{cmd} 需要一个 {zh_kind} 参数"),
    }
}

/// "expected list or string for join, got {val:?}"
#[must_use]
pub fn expected_list_or_string(lang: Lang, val: &crate::runtime::value::Value) -> String {
    match lang {
        Lang::En => format!("expected list or string for join, got {val:?}"),
        Lang::Zh => format!("join 期望列表或字符串，得到 {val:?}"),
    }
}

/// "separator must be a string or number"
#[must_use]
pub fn separator_must_be_str_or_num(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "separator must be a string or number",
        Lang::Zh => "分隔符必须是字符串或数值",
    }
}

/// "set requires 2 arguments: $var and value"
#[must_use]
pub fn set_requires_two_args(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "set requires 2 arguments: $var and value",
        Lang::Zh => "set 需要 2 个参数：$var 和 value",
    }
}

/// "clamp requires 3 numeric arguments"
#[must_use]
pub fn clamp_requires_three(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "clamp requires 3 numeric arguments",
        Lang::Zh => "clamp 需要 3 个数值参数",
    }
}

// ── IO error messages ────────────────────────────────────────────────

/// "io.in takes no arguments"
#[must_use]
pub fn io_in_no_args(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "io.in takes no arguments",
        Lang::Zh => "io.in 不接受参数",
    }
}

/// "io.get takes no arguments"
#[must_use]
pub fn io_get_no_args(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "io.get takes no arguments",
        Lang::Zh => "io.get 不接受参数",
    }
}

// ── File/Net error messages ──────────────────────────────────────────

/// "path must be absolute, got '{path}'"
#[must_use]
pub fn path_must_be_absolute(lang: Lang, path: &str) -> String {
    match lang {
        Lang::En => format!("path must be absolute, got '{path}'"),
        Lang::Zh => format!("路径必须是绝对路径，得到 '{path}'"),
    }
}

/// "failed to write '{path}': {e}"
#[must_use]
pub fn failed_to_write(lang: Lang, path: &str, e: &dyn std::fmt::Display) -> String {
    match lang {
        Lang::En => format!("failed to write '{path}': {e}"),
        Lang::Zh => format!("写入失败 '{path}': {e}"),
    }
}

/// "failed to append '{path}': {e}"
#[must_use]
pub fn failed_to_append(lang: Lang, path: &str, e: &dyn std::fmt::Display) -> String {
    match lang {
        Lang::En => format!("failed to append '{path}': {e}"),
        Lang::Zh => format!("追加失败 '{path}': {e}"),
    }
}

/// "failed to read '{path}': {e}"
#[must_use]
pub fn failed_to_read(lang: Lang, path: &str, e: &dyn std::fmt::Display) -> String {
    match lang {
        Lang::En => format!("failed to read '{path}': {e}"),
        Lang::Zh => format!("读取失败 '{path}': {e}"),
    }
}

/// "failed to fetch '{url}': {e}"
#[must_use]
pub fn failed_to_fetch(lang: Lang, url: &str, e: &dyn std::fmt::Display) -> String {
    match lang {
        Lang::En => format!("failed to fetch '{url}': {e}"),
        Lang::Zh => format!("获取失败 '{url}': {e}"),
    }
}

/// "failed to read response from '{url}': {e}"
#[must_use]
pub fn failed_to_read_response(lang: Lang, url: &str, e: &dyn std::fmt::Display) -> String {
    match lang {
        Lang::En => format!("failed to read response from '{url}': {e}"),
        Lang::Zh => format!("读取响应失败 '{url}': {e}"),
    }
}

// ── Sqrt radicand errors ─────────────────────────────────────────────

/// "invalid sqrt radicand numerator: '{s}'"
#[must_use]
pub fn invalid_sqrt_num(lang: Lang, s: &str) -> String {
    match lang {
        Lang::En => format!("invalid sqrt radicand numerator: '{s}'"),
        Lang::Zh => format!("无效平方根分子: '{s}'"),
    }
}

/// "invalid sqrt radicand denominator: '{s}'"
#[must_use]
pub fn invalid_sqrt_den(lang: Lang, s: &str) -> String {
    match lang {
        Lang::En => format!("invalid sqrt radicand denominator: '{s}'"),
        Lang::Zh => format!("无效平方根分母: '{s}'"),
    }
}

/// "sqrt radicand denominator is zero"
#[must_use]
pub fn sqrt_den_zero(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "sqrt radicand denominator is zero",
        Lang::Zh => "平方根分母为零",
    }
}

/// "invalid sqrt radicand: '{s}'"
#[must_use]
pub fn invalid_sqrt(lang: Lang, s: &str) -> String {
    match lang {
        Lang::En => format!("invalid sqrt radicand: '{s}'"),
        Lang::Zh => format!("无效平方根被开方数: '{s}'"),
    }
}

// ── Time error messages ──────────────────────────────────────────────

/// "time.wait requires a non-negative millisecond value"
#[must_use]
pub fn time_wait_nonnegative(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "time.wait requires a non-negative millisecond value",
        Lang::Zh => "time.wait 需要非负毫秒值",
    }
}

// ── Debug warning messages ───────────────────────────────────────────

/// "io.in — no input available"
#[must_use]
pub fn warn_io_in_no_input(lang: Lang) -> String {
    match lang {
        Lang::En => "io.in — no input available".to_string(),
        Lang::Zh => "io.in — 无可用输入".to_string(),
    }
}

/// "io.get — no input available"
#[must_use]
pub fn warn_io_get_no_input(lang: Lang) -> String {
    match lang {
        Lang::En => "io.get — no input available".to_string(),
        Lang::Zh => "io.get — 无可用输入".to_string(),
    }
}

/// "io.inpasswd — no input available"
#[must_use]
pub fn warn_io_inpasswd_no_input(lang: Lang) -> String {
    match lang {
        Lang::En => "io.inpasswd — no input available".to_string(),
        Lang::Zh => "io.inpasswd — 无可用输入".to_string(),
    }
}

/// "user.id — could not determine user id"
#[must_use]
pub fn warn_user_id(lang: Lang) -> String {
    match lang {
        Lang::En => "user.id — could not determine user id".to_string(),
        Lang::Zh => "user.id — 无法确定用户 ID".to_string(),
    }
}

/// "user.name — could not determine username"
#[must_use]
pub fn warn_user_name(lang: Lang) -> String {
    match lang {
        Lang::En => "user.name — could not determine username".to_string(),
        Lang::Zh => "user.name — 无法确定用户名".to_string(),
    }
}

/// "user.bash — command '{cmd}' failed with status {status}"
#[must_use]
pub fn warn_user_bash_status(lang: Lang, cmd: &str, status: &dyn std::fmt::Display) -> String {
    match lang {
        Lang::En => format!("user.bash — command '{cmd}' failed with status {status}"),
        Lang::Zh => format!("user.bash — 命令 '{cmd}' 失败，状态 {status}"),
    }
}

/// "user.bash — failed to execute '{cmd}': {e}"
#[must_use]
pub fn warn_user_bash_exec(lang: Lang, cmd: &str, e: &dyn std::fmt::Display) -> String {
    match lang {
        Lang::En => format!("user.bash — failed to execute '{cmd}': {e}"),
        Lang::Zh => format!("user.bash — 执行 '{cmd}' 失败: {e}"),
    }
}

/// "dict.get — missing key"
#[must_use]
pub fn warn_dict_get_missing_key(lang: Lang) -> String {
    match lang {
        Lang::En => "dict.get — missing key".to_string(),
        Lang::Zh => "dict.get — 键不存在".to_string(),
    }
}

/// "list.get — index {idx} out of bounds (len {len})"
#[must_use]
pub fn warn_list_get_oob(lang: Lang, idx: i64, len: usize) -> String {
    match lang {
        Lang::En => format!("list.get — index {idx} out of bounds (len {len})"),
        Lang::Zh => format!("list.get — 索引 {idx} 越界（长度 {len}）"),
    }
}

/// "list.remove — index {idx} out of bounds (len {len})"
#[must_use]
pub fn warn_list_remove_oob(lang: Lang, idx: i64, len: usize) -> String {
    match lang {
        Lang::En => format!("list.remove — index {idx} out of bounds (len {len})"),
        Lang::Zh => format!("list.remove — 索引 {idx} 越界（长度 {len}）"),
    }
}

/// "invalid regex pattern '{pat}'"
#[must_use]
pub fn warn_invalid_regex(lang: Lang, pat: &str) -> String {
    match lang {
        Lang::En => format!("invalid regex pattern '{pat}'"),
        Lang::Zh => format!("无效正则表达式 '{pat}'"),
    }
}

/// "block span underdeclared — declared {body} lines, only {avail} available"
#[must_use]
pub fn warn_block_underdeclared(lang: Lang, body: usize, avail: usize) -> String {
    match lang {
        Lang::En => {
            format!("block span underdeclared — declared {body} lines, only {avail} available")
        }
        Lang::Zh => format!("代码块行数不足 — 声明 {body} 行，仅有 {avail} 行可用"),
    }
}

/// "invalid char offset"
#[must_use]
pub fn warn_invalid_char_offset(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "invalid char offset",
        Lang::Zh => "无效字符偏移",
    }
}

/// "mixed types"
#[must_use]
pub fn warn_mixed_types(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "mixed types",
        Lang::Zh => "混合类型",
    }
}

// ── MCM host protocol messages ──────────────────────────────────────

/// "mcm command requires a host provider (use --host-json)"
#[must_use]
pub fn mcm_no_host_provider(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "mcm command requires a host provider (use --host-json)",
        Lang::Zh => "mcm 命令需要主机提供者（使用 --host-json）",
    }
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lang_from_name() {
        assert_eq!(Lang::from_name("en"), Some(Lang::En));
        assert_eq!(Lang::from_name("English"), Some(Lang::En));
        assert_eq!(Lang::from_name("zh"), Some(Lang::Zh));
        assert_eq!(Lang::from_name("chinese"), Some(Lang::Zh));
        assert_eq!(Lang::from_name("中文"), Some(Lang::Zh));
        assert_eq!(Lang::from_name("fr"), None);
    }

    #[test]
    fn lang_name_roundtrip() {
        assert_eq!(Lang::En.name(), "en");
        assert_eq!(Lang::Zh.name(), "zh");
    }

    #[test]
    fn default_is_english() {
        assert_eq!(Lang::default(), Lang::En);
    }

    #[test]
    fn reason_prefix_translations() {
        assert_eq!(reason_prefix(Lang::En), "  reason: ");
        assert_eq!(reason_prefix(Lang::Zh), "  原因: ");
    }

    #[test]
    fn unknown_command_translations() {
        let en = unknown_command(Lang::En, "dict", "foo");
        assert!(en.contains("unknown dict command"));
        let zh = unknown_command(Lang::Zh, "dict", "foo");
        assert!(zh.contains("未知字典命令"));
    }

    #[test]
    fn division_by_zero_translations() {
        assert_eq!(division_by_zero(Lang::En), "division by zero");
        assert_eq!(division_by_zero(Lang::Zh), "除以零");
    }
}

pub fn cannot_read_file(lang: Lang, path: &str, e: &dyn std::fmt::Display) -> String {
    cli_cannot_read(lang, path, e)
}
