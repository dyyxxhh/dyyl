//! Bilingual internationalization (English / Chinese).
//!
//! Every user-facing message — runtime errors, debug warnings, CLI output —
//! routes through this module so the interpreter can switch languages at
//! runtime via the `language` command.

// ── Language enum ────────────────────────────────────────────────────

/// Supported UI languages.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Lang {
    #[default]
    En,
    Zh,
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
pub fn register_plugin(name: &str, en: HashMap<String, String>, zh: HashMap<String, String>) {
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
        Lang::Zh => {
            s.en.keys()
                .filter(|k| !s.zh.contains_key(*k))
                .map(String::as_str)
                .collect()
        }
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
    t(
        lang,
        "cli.cannot_read",
        &[("path", path), ("e", &e.to_string())],
    )
}

// ── Generic runtime error patterns ───────────────────────────────────

/// "unknown {family} command '{sub}'"
#[must_use]
pub fn unknown_command(lang: Lang, family: &str, sub: &str) -> String {
    let family_zh = match lang {
        Lang::En => family,
        Lang::Zh => zh_family(family),
    };
    t(
        lang,
        "runtime.unknown_command",
        &[("family", family_zh), ("sub", sub)],
    )
}

/// "unknown command '{cmd}'" (top-level, no family prefix)
#[must_use]
pub fn unknown_top_command(lang: Lang, cmd: &str) -> String {
    t(lang, "runtime.unknown_top_command", &[("cmd", cmd)])
}

/// "undefined variable '{name}'"
#[must_use]
pub fn undefined_variable(lang: Lang, name: &str) -> String {
    t(lang, "runtime.undefined_variable", &[("name", name)])
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
    t(lang, "runtime.requires_args", &[("n", &n.to_string())])
}

/// "requires {n} arguments" (exact count)
#[must_use]
pub fn requires_n_args(lang: Lang, n: usize) -> String {
    t(lang, "runtime.requires_n_args", &[("n", &n.to_string())])
}

/// "expected string value, got {val:?}"
#[must_use]
pub fn expected_string(lang: Lang, val: &crate::runtime::value::Value) -> String {
    t(
        lang,
        "runtime.expected_string",
        &[("val", &format!("{val:?}"))],
    )
}

/// "expected numeric value, got '{s}'"
#[must_use]
pub fn expected_numeric_str(lang: Lang, s: &str) -> String {
    t(lang, "runtime.expected_numeric_str", &[("s", s)])
}

/// "expected numeric value, got {val:?}"
#[must_use]
pub fn expected_numeric(lang: Lang, val: &crate::runtime::value::Value) -> String {
    t(
        lang,
        "runtime.expected_numeric",
        &[("val", &format!("{val:?}"))],
    )
}

/// "expected variable name, got {expr:?}"
#[must_use]
pub fn expected_var_name(lang: Lang, expr: &crate::parser::types::Expr) -> String {
    t(
        lang,
        "runtime.expected_var_name",
        &[("expr", &format!("{expr:?}"))],
    )
}

/// "expected variable reference, got {expr:?}"
#[must_use]
pub fn expected_var_ref(lang: Lang, expr: &crate::parser::types::Expr) -> String {
    t(
        lang,
        "runtime.expected_var_ref",
        &[("expr", &format!("{expr:?}"))],
    )
}

/// "expected numeric, got {val:?}"
#[must_use]
pub fn expected_numeric_any(lang: Lang, val: &crate::runtime::value::Value) -> String {
    t(
        lang,
        "runtime.expected_numeric_any",
        &[("val", &format!("{val:?}"))],
    )
}

/// "expected numeric index, got {val:?}"
#[must_use]
pub fn expected_numeric_index(lang: Lang, val: &crate::runtime::value::Value) -> String {
    t(
        lang,
        "runtime.expected_numeric_index",
        &[("val", &format!("{val:?}"))],
    )
}

/// "index must be non-negative, got {n}"
#[must_use]
pub fn index_must_be_nonnegative(lang: Lang, n: i64) -> String {
    t(
        lang,
        "runtime.index_must_be_nonnegative",
        &[("n", &n.to_string())],
    )
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
    let kind_zh = match lang {
        Lang::En => kind,
        Lang::Zh => match kind {
            "dict" => "字典",
            "list" => "列表",
            _ => kind,
        },
    };
    t(lang, "runtime.first_arg_must_be", &[("kind", kind_zh)])
}

/// "argument must be a {kind}"
#[must_use]
pub fn argument_must_be(lang: Lang, kind: &str) -> String {
    let kind_zh = match lang {
        Lang::En => kind,
        Lang::Zh => match kind {
            "dict" => "字典",
            "list" => "列表",
            _ => kind,
        },
    };
    t(lang, "runtime.argument_must_be", &[("kind", kind_zh)])
}

/// "{cmd} requires a variable name"
#[must_use]
pub fn cmd_requires_var(lang: Lang, cmd: &str) -> String {
    t(lang, "runtime.cmd_requires_var", &[("cmd", cmd)])
}

/// "{cmd} requires a {kind} argument"
#[must_use]
pub fn cmd_requires_container(lang: Lang, cmd: &str, kind: &str) -> String {
    let kind_zh = match lang {
        Lang::En => kind,
        Lang::Zh => match kind {
            "dict" => "字典",
            "list" => "列表",
            _ => kind,
        },
    };
    t(
        lang,
        "runtime.cmd_requires_container",
        &[("cmd", cmd), ("kind", kind_zh)],
    )
}

/// "expected list or string for join, got {val:?}"
#[must_use]
pub fn expected_list_or_string(lang: Lang, val: &crate::runtime::value::Value) -> String {
    t(
        lang,
        "runtime.expected_list_or_string",
        &[("val", &format!("{val:?}"))],
    )
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
    t(lang, "file.path_must_be_absolute", &[("path", path)])
}

/// "failed to write '{path}': {e}"
#[must_use]
pub fn failed_to_write(lang: Lang, path: &str, e: &dyn std::fmt::Display) -> String {
    t(
        lang,
        "file.failed_to_write",
        &[("path", path), ("e", &e.to_string())],
    )
}

/// "failed to append '{path}': {e}"
#[must_use]
pub fn failed_to_append(lang: Lang, path: &str, e: &dyn std::fmt::Display) -> String {
    t(
        lang,
        "file.failed_to_append",
        &[("path", path), ("e", &e.to_string())],
    )
}

/// "failed to read '{path}': {e}"
#[must_use]
pub fn failed_to_read(lang: Lang, path: &str, e: &dyn std::fmt::Display) -> String {
    t(
        lang,
        "file.failed_to_read",
        &[("path", path), ("e", &e.to_string())],
    )
}

/// "failed to fetch '{url}': {e}"
#[must_use]
pub fn failed_to_fetch(lang: Lang, url: &str, e: &dyn std::fmt::Display) -> String {
    t(
        lang,
        "net.failed_to_fetch",
        &[("url", url), ("e", &e.to_string())],
    )
}

/// "failed to read response from '{url}': {e}"
#[must_use]
pub fn failed_to_read_response(lang: Lang, url: &str, e: &dyn std::fmt::Display) -> String {
    t(
        lang,
        "net.failed_to_read_response",
        &[("url", url), ("e", &e.to_string())],
    )
}

// ── Sqrt radicand errors ─────────────────────────────────────────────

/// "invalid sqrt radicand numerator: '{s}'"
#[must_use]
pub fn invalid_sqrt_num(lang: Lang, s: &str) -> String {
    t(lang, "sqrt.invalid_num", &[("s", s)])
}

/// "invalid sqrt radicand denominator: '{s}'"
#[must_use]
pub fn invalid_sqrt_den(lang: Lang, s: &str) -> String {
    t(lang, "sqrt.invalid_den", &[("s", s)])
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
    t(lang, "sqrt.invalid", &[("s", s)])
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
    t(lang, "debug.io_in_no_input", &[])
}

/// "io.get — no input available"
#[must_use]
pub fn warn_io_get_no_input(lang: Lang) -> String {
    t(lang, "debug.io_get_no_input", &[])
}

/// "io.inpasswd — no input available"
#[must_use]
pub fn warn_io_inpasswd_no_input(lang: Lang) -> String {
    t(lang, "debug.io_inpasswd_no_input", &[])
}

/// "user.id — could not determine user id"
#[must_use]
pub fn warn_user_id(lang: Lang) -> String {
    t(lang, "debug.user_id", &[])
}

/// "user.name — could not determine username"
#[must_use]
pub fn warn_user_name(lang: Lang) -> String {
    t(lang, "debug.user_name", &[])
}

/// "user.bash — command '{cmd}' failed with status {status}"
#[must_use]
pub fn warn_user_bash_status(lang: Lang, cmd: &str, status: &dyn std::fmt::Display) -> String {
    t(
        lang,
        "debug.user_bash_status",
        &[("cmd", cmd), ("status", &status.to_string())],
    )
}

/// "user.bash — failed to execute '{cmd}': {e}"
#[must_use]
pub fn warn_user_bash_exec(lang: Lang, cmd: &str, e: &dyn std::fmt::Display) -> String {
    t(
        lang,
        "debug.user_bash_exec",
        &[("cmd", cmd), ("e", &e.to_string())],
    )
}

/// "dict.get — missing key"
#[must_use]
pub fn warn_dict_get_missing_key(lang: Lang) -> String {
    t(lang, "debug.dict_get_missing_key", &[])
}

/// "list.get — index {idx} out of bounds (len {len})"
#[must_use]
pub fn warn_list_get_oob(lang: Lang, idx: i64, len: usize) -> String {
    t(
        lang,
        "debug.list_get_oob",
        &[("idx", &idx.to_string()), ("len", &len.to_string())],
    )
}

/// "list.remove — index {idx} out of bounds (len {len})"
#[must_use]
pub fn warn_list_remove_oob(lang: Lang, idx: i64, len: usize) -> String {
    t(
        lang,
        "debug.list_remove_oob",
        &[("idx", &idx.to_string()), ("len", &len.to_string())],
    )
}

/// "invalid regex pattern '{pat}'"
#[must_use]
pub fn warn_invalid_regex(lang: Lang, pat: &str) -> String {
    t(lang, "debug.invalid_regex", &[("pat", pat)])
}

/// "block span underdeclared — declared {body} lines, only {avail} available"
#[must_use]
pub fn warn_block_underdeclared(lang: Lang, body: usize, avail: usize) -> String {
    t(
        lang,
        "debug.block_underdeclared",
        &[("body", &body.to_string()), ("avail", &avail.to_string())],
    )
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

/// "unknown command: mcm.* requires a host provider (use --host-json)"
///
/// Without a host provider, `mcm.*` commands are unknown to the runtime —
/// they return the unknown-command sentinel (`-1`) with this warning.
#[must_use]
pub fn mcm_no_host_provider(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "unknown command: mcm.* requires a host provider (use --host-json)",
        Lang::Zh => "未知命令：mcm.* 需要主机提供者（使用 --host-json）",
    }
}

// ── Plugin system messages ──────────────────────────────────────────

#[must_use]
pub fn plugin_install_success(lang: Lang, name: &str, ver: &str) -> String {
    t(
        lang,
        "plugin.install_success",
        &[("name", name), ("ver", ver)],
    )
}

#[must_use]
pub fn plugin_already_installed(lang: Lang, name: &str, ver: &str) -> String {
    t(
        lang,
        "plugin.already_installed",
        &[("name", name), ("ver", ver)],
    )
}

#[must_use]
pub fn plugin_updated(lang: Lang, name: &str, old: &str, new: &str) -> String {
    t(
        lang,
        "plugin.updated",
        &[("name", name), ("old", old), ("new", new)],
    )
}

#[must_use]
pub fn plugin_already_latest(lang: Lang, name: &str, ver: &str) -> String {
    t(
        lang,
        "plugin.already_latest",
        &[("name", name), ("ver", ver)],
    )
}

#[must_use]
pub fn plugin_removed(lang: Lang, name: &str) -> String {
    t(lang, "plugin.removed", &[("name", name)])
}

#[must_use]
pub fn plugin_not_installed(lang: Lang, name: &str) -> String {
    t(lang, "plugin.not_installed", &[("name", name)])
}

#[must_use]
pub fn plugin_install_failed(lang: Lang, name: &str, reason: &str) -> String {
    t(
        lang,
        "plugin.install_failed",
        &[("name", name), ("reason", reason)],
    )
}

#[must_use]
pub fn plugin_update_failed(lang: Lang, name: &str, reason: &str) -> String {
    t(
        lang,
        "plugin.update_failed",
        &[("name", name), ("reason", reason)],
    )
}

#[must_use]
pub fn plugin_remove_failed(lang: Lang, name: &str, reason: &str) -> String {
    t(
        lang,
        "plugin.remove_failed",
        &[("name", name), ("reason", reason)],
    )
}

#[must_use]
pub fn plugin_update_all_summary(
    lang: Lang,
    updated: usize,
    latest: usize,
    failed: usize,
) -> String {
    t(
        lang,
        "plugin.update_all_summary",
        &[
            ("updated", &updated.to_string()),
            ("latest", &latest.to_string()),
            ("failed", &failed.to_string()),
        ],
    )
}

#[must_use]
pub fn plugin_autoremove_summary(lang: Lang, count: usize) -> String {
    t(
        lang,
        "plugin.autoremove_summary",
        &[("count", &count.to_string())],
    )
}

#[must_use]
pub fn plugin_autoremove_removed(lang: Lang, name: &str, days_ago: u64) -> String {
    t(
        lang,
        "plugin.autoremove_removed",
        &[("name", name), ("days_ago", &days_ago.to_string())],
    )
}

#[must_use]
pub fn plugin_list_header(lang: Lang) -> String {
    t(lang, "plugin.list_header", &[])
}

#[must_use]
pub fn plugin_list_empty(lang: Lang) -> String {
    t(lang, "plugin.list_empty", &[])
}

#[must_use]
pub fn plugin_fetch_manifest_failed(lang: Lang, name: &str, reason: &str) -> String {
    t(
        lang,
        "plugin.fetch_manifest_failed",
        &[("name", name), ("reason", reason)],
    )
}

#[must_use]
pub fn plugin_manifest_not_found(lang: Lang, name: &str) -> String {
    t(lang, "plugin.manifest_not_found", &[("name", name)])
}

#[must_use]
pub fn plugin_abi_mismatch(lang: Lang, name: &str, expected: u32, actual: u32) -> String {
    t(
        lang,
        "plugin.abi_mismatch",
        &[
            ("name", name),
            ("expected", &expected.to_string()),
            ("actual", &actual.to_string()),
        ],
    )
}

#[must_use]
pub fn plugin_dyyl_min_unmet(lang: Lang, name: &str, required: &str, current: &str) -> String {
    t(
        lang,
        "plugin.dyyl_min_unmet",
        &[("name", name), ("required", required), ("current", current)],
    )
}

#[must_use]
pub fn plugin_platform_unavailable(
    lang: Lang,
    name: &str,
    platform: &str,
    available: &str,
) -> String {
    t(
        lang,
        "plugin.platform_unavailable",
        &[
            ("name", name),
            ("platform", platform),
            ("available", available),
        ],
    )
}

#[must_use]
pub fn plugin_sha256_mismatch(lang: Lang, name: &str) -> String {
    t(lang, "plugin.sha256_mismatch", &[("name", name)])
}

#[must_use]
pub fn plugin_download_failed(lang: Lang, name: &str, reason: &str) -> String {
    t(
        lang,
        "plugin.download_failed",
        &[("name", name), ("reason", reason)],
    )
}

#[must_use]
pub fn plugin_dlopen_failed(lang: Lang, name: &str, reason: &str) -> String {
    t(
        lang,
        "plugin.dlopen_failed",
        &[("name", name), ("reason", reason)],
    )
}

#[must_use]
pub fn plugin_symbol_missing(lang: Lang, name: &str, symbol: &str) -> String {
    t(
        lang,
        "plugin.symbol_missing",
        &[("name", name), ("symbol", symbol)],
    )
}

#[must_use]
pub fn plugin_init_failed(lang: Lang, name: &str) -> String {
    t(lang, "plugin.init_failed", &[("name", name)])
}

#[must_use]
pub fn plugin_on_load_failed(lang: Lang, name: &str, code: i32) -> String {
    t(
        lang,
        "plugin.on_load_failed",
        &[("name", name), ("code", &code.to_string())],
    )
}

#[must_use]
pub fn plugin_unknown_subcommand(lang: Lang, name: &str, sub: &str) -> String {
    t(
        lang,
        "plugin.unknown_subcommand",
        &[("name", name), ("sub", sub)],
    )
}

#[must_use]
pub fn plugin_arity_mismatch(
    lang: Lang,
    name: &str,
    sub: &str,
    expected: usize,
    actual: usize,
) -> String {
    t(
        lang,
        "plugin.arity_mismatch",
        &[
            ("name", name),
            ("sub", sub),
            ("expected", &expected.to_string()),
            ("actual", &actual.to_string()),
        ],
    )
}

#[must_use]
pub fn plugin_command_failed(lang: Lang, name: &str, sub: &str, code: &str) -> String {
    t(
        lang,
        "plugin.command_failed",
        &[("name", name), ("sub", sub), ("code", code)],
    )
}

#[must_use]
pub fn plugin_panic_warning(lang: Lang, name: &str) -> String {
    t(lang, "plugin.panic_warning", &[("name", name)])
}

// ── CLI plugin subcommand messages ──────────────────────────────────

#[must_use]
pub fn cli_plugin_usage(lang: Lang) -> String {
    t(lang, "cli.plugin_usage", &[])
}

#[must_use]
pub fn cli_plugin_subcommand_unknown(lang: Lang, sub: &str) -> String {
    t(lang, "cli.plugin_subcommand_unknown", &[("sub", sub)])
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
