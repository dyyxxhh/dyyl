//! Basic string operations: len, get, slice, find, rfind, count,
//! upper, lower, capital, reverse, repeat, start, end, contains, index.

use super::str::{resolve_index_arg, resolve_str_arg, StrCtx};
use crate::i18n;
use crate::runtime::error::RuntimeError;
use crate::runtime::value::Value;

pub(super) fn dispatch_basic(sub: &str, sc: &mut StrCtx) -> Result<Value, RuntimeError> {
    match sub {
        "len" => do_len(sc),
        "get" => do_get(sc),
        "slice" => do_slice(sc),
        "find" => do_find(sc),
        "rfind" => do_rfind(sc),
        "count" => do_count(sc),
        "upper" => do_upper(sc),
        "lower" => do_lower(sc),
        "capital" => do_capital(sc),
        "reverse" => do_reverse(sc),
        "repeat" => do_repeat(sc),
        "start" => do_start(sc),
        "end" => do_end(sc),
        "contains" => do_contains(sc),
        "index" => do_index(sc),
        _ => Err(RuntimeError::new(
            sc.exec.line,
            &sc.call.command,
            i18n::unknown_command(sc.exec.lang.get(), "str.basic", sub),
        )),
    }
}

fn do_len(sc: &mut StrCtx) -> Result<Value, RuntimeError> {
    let s = resolve_str_arg(sc, 0)?;
    Ok(Value::Num(s.chars().count() as i64))
}

fn do_get(sc: &mut StrCtx) -> Result<Value, RuntimeError> {
    let s = resolve_str_arg(sc, 0)?;
    let idx = resolve_index_arg(sc, 1)?;
    match s.chars().nth(idx) {
        Some(c) => Ok(Value::Str(c.to_string())),
        None => Ok(Value::sentinel_num()),
    }
}

fn do_slice(sc: &mut StrCtx) -> Result<Value, RuntimeError> {
    let s = resolve_str_arg(sc, 0)?;
    let start = resolve_index_arg(sc, 1)?;
    let end = resolve_index_arg(sc, 2)?;
    let chars: Vec<char> = s.chars().collect();
    let end_clamped = end.min(chars.len());
    if start > end_clamped || start > chars.len() {
        return Ok(Value::sentinel_str());
    }
    let result: String = chars[start..end_clamped].iter().collect();
    Ok(Value::Str(result))
}

fn do_find(sc: &mut StrCtx) -> Result<Value, RuntimeError> {
    let s = resolve_str_arg(sc, 0)?;
    let pat = resolve_str_arg(sc, 1)?;
    match s.find(&*pat) {
        Some(pos) => {
            // Convert byte position to char position.
            let char_pos = s[..pos].chars().count();
            Ok(Value::Num(char_pos as i64))
        }
        None => Ok(Value::Num(-1)),
    }
}

fn do_rfind(sc: &mut StrCtx) -> Result<Value, RuntimeError> {
    let s = resolve_str_arg(sc, 0)?;
    let pat = resolve_str_arg(sc, 1)?;
    match s.rfind(&*pat) {
        Some(pos) => {
            let char_pos = s[..pos].chars().count();
            Ok(Value::Num(char_pos as i64))
        }
        None => Ok(Value::Num(-1)),
    }
}

fn do_count(sc: &mut StrCtx) -> Result<Value, RuntimeError> {
    let s = resolve_str_arg(sc, 0)?;
    let pat = resolve_str_arg(sc, 1)?;
    if pat.is_empty() {
        return Ok(Value::Num(0));
    }
    let count = s.matches(&*pat).count();
    Ok(Value::Num(count as i64))
}

fn do_upper(sc: &mut StrCtx) -> Result<Value, RuntimeError> {
    let s = resolve_str_arg(sc, 0)?;
    Ok(Value::Str(s.to_uppercase()))
}

fn do_lower(sc: &mut StrCtx) -> Result<Value, RuntimeError> {
    let s = resolve_str_arg(sc, 0)?;
    Ok(Value::Str(s.to_lowercase()))
}

fn do_capital(sc: &mut StrCtx) -> Result<Value, RuntimeError> {
    let s = resolve_str_arg(sc, 0)?;
    let mut chars = s.chars();
    let result = match chars.next() {
        None => String::new(),
        Some(first) => {
            let upper: String = first.to_uppercase().collect();
            let rest: String = chars.collect();
            format!("{upper}{rest}")
        }
    };
    Ok(Value::Str(result))
}

fn do_reverse(sc: &mut StrCtx) -> Result<Value, RuntimeError> {
    let s = resolve_str_arg(sc, 0)?;
    let result: String = s.chars().rev().collect();
    Ok(Value::Str(result))
}

fn do_repeat(sc: &mut StrCtx) -> Result<Value, RuntimeError> {
    let s = resolve_str_arg(sc, 0)?;
    let n = resolve_index_arg(sc, 1)?;
    let result: String = s.repeat(n);
    Ok(Value::Str(result))
}

fn do_start(sc: &mut StrCtx) -> Result<Value, RuntimeError> {
    let s = resolve_str_arg(sc, 0)?;
    let prefix = resolve_str_arg(sc, 1)?;
    Ok(Value::Num(i64::from(s.starts_with(&*prefix))))
}

fn do_end(sc: &mut StrCtx) -> Result<Value, RuntimeError> {
    let s = resolve_str_arg(sc, 0)?;
    let suffix = resolve_str_arg(sc, 1)?;
    Ok(Value::Num(i64::from(s.ends_with(&*suffix))))
}

fn do_contains(sc: &mut StrCtx) -> Result<Value, RuntimeError> {
    let s = resolve_str_arg(sc, 0)?;
    let pat = resolve_str_arg(sc, 1)?;
    Ok(Value::Num(i64::from(s.contains(&*pat))))
}

fn do_index(sc: &mut StrCtx) -> Result<Value, RuntimeError> {
    let s = resolve_str_arg(sc, 0)?;
    let pat = resolve_str_arg(sc, 1)?;
    match s.find(&*pat) {
        Some(pos) => {
            let char_pos = s[..pos].chars().count();
            Ok(Value::Num(char_pos as i64))
        }
        None => Ok(Value::Num(-1)),
    }
}
