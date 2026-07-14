//! DyylValue JSON encode/decode, symmetric with dyyl's `value_codec.rs`.
//!
//! JSON format (matches `/workspace/src/runtime/plugin/value_codec.rs`):
//! - Num:   `{"type":"num","value":"123"}`  (num as string for arbitrary precision)
//! - Str:   `{"type":"str","value":"hello"}`
//! - Empty: `{"type":"empty"}`
//! - List:  `{"type":"list","value":[...]}`  (array of value objects)
//! - Dict:  `{"type":"dict","value":[{"key":{...},"val":{...}},...]}`
//!
//! dyyl also has an `Expr` type encoded as `{"type":"expr","value":"..."}`;
//! the plugin treats it as Num on decode (best-effort parse).

use std::ffi::CString;
use std::os::raw::c_char;

use serde_json::{json, Value as JsonValue};

/// A dyyl value, mirrored from the host for cross-FFI marshalling.
#[derive(Debug, Clone)]
pub enum DyylValue {
    Num(String),
    Str(String),
    Empty,
    List(Vec<DyylValue>),
    Dict(Vec<(DyylValue, DyylValue)>),
}

impl DyylValue {
    /// Returns the inner `&str` if this is a `Str`, else `None`.
    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        match self {
            DyylValue::Str(s) => Some(s),
            _ => None,
        }
    }

    /// Returns the inner num string if this is a `Num`, else `None`.
    #[must_use]
    pub fn as_num(&self) -> Option<&str> {
        match self {
            DyylValue::Num(s) => Some(s),
            _ => None,
        }
    }

    /// Returns the inner slice if this is a `List`, else `None`.
    #[must_use]
    pub fn as_list(&self) -> Option<&[DyylValue]> {
        match self {
            DyylValue::List(items) => Some(items),
            _ => None,
        }
    }
}

/// Decode a JSON array of value objects into `Vec<DyylValue>`.
pub fn decode_args(json: &str) -> Result<Vec<DyylValue>, String> {
    let parsed: JsonValue =
        serde_json::from_str(json).map_err(|e| format!("parse args json: {e}"))?;
    let arr = parsed
        .as_array()
        .ok_or_else(|| "args json is not an array".to_string())?;
    let mut out = Vec::with_capacity(arr.len());
    for item in arr {
        out.push(decode_value(item)?);
    }
    Ok(out)
}

fn decode_value(jv: &JsonValue) -> Result<DyylValue, String> {
    let ty = jv.get("type").and_then(|t| t.as_str()).unwrap_or("empty");
    match ty {
        "num" => {
            let s = jv.get("value").and_then(|v| v.as_str()).unwrap_or("0");
            Ok(DyylValue::Num(s.to_string()))
        }
        // dyyl's `Expr` type — treat as Num (best-effort parse).
        "expr" => {
            let s = jv.get("value").and_then(|v| v.as_str()).unwrap_or("0");
            Ok(DyylValue::Num(s.to_string()))
        }
        "str" => {
            let s = jv.get("value").and_then(|v| v.as_str()).unwrap_or("");
            Ok(DyylValue::Str(s.to_string()))
        }
        "empty" => Ok(DyylValue::Empty),
        "list" => {
            let arr = jv
                .get("value")
                .and_then(|v| v.as_array())
                .ok_or_else(|| "list missing value array".to_string())?;
            let mut items = Vec::with_capacity(arr.len());
            for item in arr {
                items.push(decode_value(item)?);
            }
            Ok(DyylValue::List(items))
        }
        "dict" => {
            let arr = jv
                .get("value")
                .and_then(|v| v.as_array())
                .ok_or_else(|| "dict missing value array".to_string())?;
            let mut pairs = Vec::with_capacity(arr.len());
            for p in arr {
                let k = p
                    .get("key")
                    .ok_or_else(|| "dict entry missing key".to_string())?;
                let v = p
                    .get("val")
                    .ok_or_else(|| "dict entry missing val".to_string())?;
                pairs.push((decode_value(k)?, decode_value(v)?));
            }
            Ok(DyylValue::Dict(pairs))
        }
        _ => Ok(DyylValue::Empty),
    }
}

/// Encode a single `DyylValue` to its JSON string representation.
#[must_use]
pub fn encode_value(v: &DyylValue) -> String {
    encode_value_to_json(v).to_string()
}

fn encode_value_to_json(v: &DyylValue) -> JsonValue {
    match v {
        DyylValue::Num(n) => json!({"type": "num", "value": n}),
        DyylValue::Str(s) => json!({"type": "str", "value": s}),
        DyylValue::Empty => json!({"type": "empty"}),
        DyylValue::List(items) => {
            let arr: Vec<JsonValue> = items.iter().map(encode_value_to_json).collect();
            json!({"type": "list", "value": arr})
        }
        DyylValue::Dict(pairs) => {
            let arr: Vec<JsonValue> = pairs
                .iter()
                .map(|(k, v)| {
                    json!({"key": encode_value_to_json(k), "val": encode_value_to_json(v)})
                })
                .collect();
            json!({"type": "dict", "value": arr})
        }
    }
}

/// Encode `v` and write it to the `out` parameter (allocates via
/// `CString::into_raw`). The caller must free the buffer with
/// `dyyl_plugin_free_string`.
///
/// Contract: `out` must be a valid, non-null pointer to a `*mut c_char`
/// slot owned by the caller.
pub fn encode_out(out: *mut *mut c_char, v: &DyylValue) {
    let json = encode_value(v);
    let c = cstring_from_str(&json);
    // SAFETY: caller guarantees `out` is a valid pointer to a slot.
    unsafe {
        *out = c.into_raw();
    }
}

/// Build a `CString` from `&str`, stripping any NUL bytes (which would
/// otherwise make `CString::new` fail). After stripping, construction is
/// infallible; the `unwrap_or_else` branch is unreachable but kept to
/// satisfy the type system without `unwrap`/`expect` (denied by clippy).
fn cstring_from_str(s: &str) -> CString {
    let bytes: Vec<u8> = s.bytes().filter(|b| *b != 0).collect();
    CString::new(bytes).unwrap_or_else(|_| empty_cstring())
}

/// Returns a guaranteed-valid empty `CString`.
fn empty_cstring() -> CString {
    // SAFETY: A single NUL byte is a valid CString representing the empty
    // string (no interior NULs, last byte is NUL).
    unsafe { CString::from_vec_with_nul_unchecked(vec![0u8]) }
}
