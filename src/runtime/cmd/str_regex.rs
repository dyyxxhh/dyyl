//! Regex operations: match, extract, replace.regex, escape, unescape.
//!
//! Uses the `regex` crate. Invalid patterns return the string sentinel
//! and emit a debug warning with "regex" and the line number.

use regex::escape as regex_escape;
use regex::Regex;

use super::str::{resolve_str_arg, StrCtx};
use crate::i18n;
use crate::runtime::error::RuntimeError;
use crate::runtime::value::Value;

pub(super) fn dispatch_regex(sub: &str, sc: &mut StrCtx) -> Result<Value, RuntimeError> {
    match sub {
        "match" => do_match(sc),
        "extract" => do_extract(sc),
        "replace.regex" => do_replace_regex(sc),
        "escape" => do_escape(sc),
        "unescape" => do_unescape(sc),
        _ => Err(RuntimeError::new(
            sc.exec.line,
            &sc.call.command,
            i18n::unknown_command(sc.exec.lang.get(), "str.regex", sub),
        )),
    }
}

fn do_match(sc: &mut StrCtx) -> Result<Value, RuntimeError> {
    let s = resolve_str_arg(sc, 0)?;
    let pat = resolve_str_arg(sc, 1)?;
    let re = match Regex::new(&pat) {
        Ok(r) => r,
        Err(_) => {
            debug_warn_regex(sc, &pat);
            return Ok(Value::sentinel_str());
        }
    };
    Ok(Value::Num(i64::from(re.is_match(&s))))
}

fn do_extract(sc: &mut StrCtx) -> Result<Value, RuntimeError> {
    let s = resolve_str_arg(sc, 0)?;
    let pat = resolve_str_arg(sc, 1)?;
    let re = match Regex::new(&pat) {
        Ok(r) => r,
        Err(_) => {
            debug_warn_regex(sc, &pat);
            return Ok(Value::sentinel_str());
        }
    };
    match re.find(&s) {
        Some(m) => Ok(Value::Str(m.as_str().to_string())),
        None => Ok(Value::sentinel_str()),
    }
}

fn do_replace_regex(sc: &mut StrCtx) -> Result<Value, RuntimeError> {
    let s = resolve_str_arg(sc, 0)?;
    let pat = resolve_str_arg(sc, 1)?;
    let replacement = resolve_str_arg(sc, 2)?;
    let re = match Regex::new(&pat) {
        Ok(r) => r,
        Err(_) => {
            debug_warn_regex(sc, &pat);
            return Ok(Value::sentinel_str());
        }
    };
    let result = re.replace(&s, &*replacement);
    Ok(Value::Str(result.into_owned()))
}

fn do_escape(sc: &mut StrCtx) -> Result<Value, RuntimeError> {
    let s = resolve_str_arg(sc, 0)?;
    Ok(Value::Str(regex_escape(&s)))
}

fn do_unescape(sc: &mut StrCtx) -> Result<Value, RuntimeError> {
    let s = resolve_str_arg(sc, 0)?;
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('d') => result.push_str(r"\d"),
                Some('D') => result.push_str(r"\D"),
                Some('w') => result.push_str(r"\w"),
                Some('W') => result.push_str(r"\W"),
                Some('s') => result.push_str(r"\s"),
                Some('S') => result.push_str(r"\S"),
                Some('b') => result.push_str(r"\b"),
                Some('B') => result.push_str(r"\B"),
                Some(other) => result.push(other),
                None => result.push('\\'),
            }
        } else {
            result.push(c);
        }
    }
    Ok(Value::Str(result))
}

fn debug_warn_regex(sc: &mut StrCtx, pat: &str) {
    if sc.exec.debug {
        eprintln!("line {}: {}", sc.exec.line, sc.exec.text);
        eprintln!(
            "{}{}",
            i18n::reason_prefix(sc.exec.lang.get()),
            i18n::warn_invalid_regex(sc.exec.lang.get(), pat)
        );
    }
}
