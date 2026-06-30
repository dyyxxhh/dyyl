//! String modification operations: replace, replace.all, insert, remove,
//! pad.left, pad.right, trim, trim.left, trim.right.

use super::str::{resolve_index_arg, resolve_str_arg, StrCtx};
use crate::i18n;
use crate::runtime::error::RuntimeError;
use crate::runtime::value::Value;

pub(super) fn dispatch_modify(sub: &str, sc: &mut StrCtx) -> Result<Value, RuntimeError> {
    match sub {
        "replace" => do_replace(sc),
        "replace.all" => do_replace_all(sc),
        "insert" => do_insert(sc),
        "remove" => do_remove(sc),
        "pad.left" => do_pad_left(sc),
        "pad.right" => do_pad_right(sc),
        "trim" => do_trim(sc),
        "trim.left" => do_trim_left(sc),
        "trim.right" => do_trim_right(sc),
        _ => Err(RuntimeError::new(
            sc.exec.line,
            &sc.call.command,
            i18n::unknown_command(sc.exec.lang.get(), "str.modify", sub),
        )),
    }
}

fn do_replace(sc: &mut StrCtx) -> Result<Value, RuntimeError> {
    let s = resolve_str_arg(sc, 0)?;
    let old = resolve_str_arg(sc, 1)?;
    let new = resolve_str_arg(sc, 2)?;
    if old.is_empty() {
        return Ok(Value::Str(s));
    }
    let result = s.replacen(&*old, &new, 1);
    Ok(Value::Str(result))
}

fn do_replace_all(sc: &mut StrCtx) -> Result<Value, RuntimeError> {
    let s = resolve_str_arg(sc, 0)?;
    let old = resolve_str_arg(sc, 1)?;
    let new = resolve_str_arg(sc, 2)?;
    if old.is_empty() {
        return Ok(Value::Str(s));
    }
    let result = s.replace(&*old, &new);
    Ok(Value::Str(result))
}

fn do_insert(sc: &mut StrCtx) -> Result<Value, RuntimeError> {
    let s = resolve_str_arg(sc, 0)?;
    let idx = resolve_index_arg(sc, 1)?;
    let ins = resolve_str_arg(sc, 2)?;
    let chars: Vec<char> = s.chars().collect();
    if idx > chars.len() {
        return Ok(Value::Str(s));
    }
    let mut result: String = chars[..idx].iter().collect();
    result.push_str(&ins);
    result.push_str(&chars[idx..].iter().collect::<String>());
    Ok(Value::Str(result))
}

fn do_remove(sc: &mut StrCtx) -> Result<Value, RuntimeError> {
    let s = resolve_str_arg(sc, 0)?;
    let start = resolve_index_arg(sc, 1)?;
    let end = resolve_index_arg(sc, 2)?;
    let chars: Vec<char> = s.chars().collect();
    let end_clamped = end.min(chars.len());
    if start > end_clamped || start > chars.len() {
        return Ok(Value::Str(s));
    }
    let mut result: String = chars[..start].iter().collect();
    result.push_str(&chars[end_clamped..].iter().collect::<String>());
    Ok(Value::Str(result))
}

fn do_pad_left(sc: &mut StrCtx) -> Result<Value, RuntimeError> {
    let s = resolve_str_arg(sc, 0)?;
    let target_len = resolve_index_arg(sc, 1)?;
    let pad_char = resolve_str_arg(sc, 2)?;
    let current = s.chars().count();
    if current >= target_len || pad_char.is_empty() {
        return Ok(Value::Str(s));
    }
    let needed = target_len - current;
    let pc = pad_char.chars().next().map_or(' ', |c| c);
    let padding: String = std::iter::repeat_n(pc, needed).collect();
    Ok(Value::Str(format!("{padding}{s}")))
}

fn do_pad_right(sc: &mut StrCtx) -> Result<Value, RuntimeError> {
    let s = resolve_str_arg(sc, 0)?;
    let target_len = resolve_index_arg(sc, 1)?;
    let pad_char = resolve_str_arg(sc, 2)?;
    let current = s.chars().count();
    if current >= target_len || pad_char.is_empty() {
        return Ok(Value::Str(s));
    }
    let needed = target_len - current;
    let pc = pad_char.chars().next().map_or(' ', |c| c);
    let padding: String = std::iter::repeat_n(pc, needed).collect();
    Ok(Value::Str(format!("{s}{padding}")))
}

fn do_trim(sc: &mut StrCtx) -> Result<Value, RuntimeError> {
    let s = resolve_str_arg(sc, 0)?;
    Ok(Value::Str(s.trim().to_string()))
}

fn do_trim_left(sc: &mut StrCtx) -> Result<Value, RuntimeError> {
    let s = resolve_str_arg(sc, 0)?;
    Ok(Value::Str(s.trim_start().to_string()))
}

fn do_trim_right(sc: &mut StrCtx) -> Result<Value, RuntimeError> {
    let s = resolve_str_arg(sc, 0)?;
    Ok(Value::Str(s.trim_end().to_string()))
}
