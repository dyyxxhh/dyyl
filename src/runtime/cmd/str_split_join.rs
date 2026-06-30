//! Split and join operations.

use super::str::{resolve_str_arg, resolve_val, StrCtx};
use crate::i18n;
use crate::runtime::error::RuntimeError;
use crate::runtime::value::Value;

pub(super) fn dispatch_split_join(sub: &str, sc: &mut StrCtx) -> Result<Value, RuntimeError> {
    match sub {
        "split" => do_split(sc),
        "join" => do_join(sc),
        _ => Err(RuntimeError::new(
            sc.exec.line,
            &sc.call.command,
            i18n::unknown_command(sc.exec.lang.get(), "str.split/join", sub),
        )),
    }
}

fn do_split(sc: &mut StrCtx) -> Result<Value, RuntimeError> {
    let s = resolve_str_arg(sc, 0)?;
    let sep = resolve_str_arg(sc, 1)?;
    let items: Vec<Value> = if sep.is_empty() {
        s.chars().map(|c| Value::Str(c.to_string())).collect()
    } else {
        s.split(&*sep)
            .map(|part| Value::Str(part.to_string()))
            .collect()
    };
    Ok(Value::List(items))
}

fn do_join(sc: &mut StrCtx) -> Result<Value, RuntimeError> {
    let sep = resolve_str_arg(sc, 0)?;
    // arg[1] should be a list value (resolved from variable).
    let val = resolve_val(sc, 1)?;
    match val {
        Value::List(items) => {
            let parts: Vec<String> = items.iter().map(Value::to_string).collect();
            Ok(Value::Str(parts.join(&sep)))
        }
        Value::Str(s) => Ok(Value::Str(s)),
        _ => Err(RuntimeError::new(
            sc.exec.line,
            &sc.call.command,
            i18n::expected_list_or_string(sc.exec.lang.get(), &val),
        )),
    }
}
