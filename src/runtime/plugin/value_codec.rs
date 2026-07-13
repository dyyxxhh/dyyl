//! Value JSON encoding for the plugin ABI.
//!
//! Encodes dyyl `Value`s to/from JSON for cross-FFI communication.
//! `num` values are encoded as strings (to preserve arbitrary-precision
//! integers and fractions from `CasNumber`).

use serde_json::{json, Value as JsonValue};

use crate::runtime::value::Value;

/// Encode a single Value to its JSON representation.
#[must_use]
pub fn value_to_json(v: &Value) -> String {
    let jv = value_to_json_value(v);
    jv.to_string()
}

/// Encode a slice of Values to a JSON array (used for args).
#[must_use]
pub fn values_to_json_array(values: &[Value]) -> String {
    let arr: Vec<JsonValue> = values.iter().map(value_to_json_value).collect();
    serde_json::to_string(&arr).unwrap_or_else(|_| "[]".to_string())
}

/// Decode a JSON string to a Value.
pub fn value_from_json(s: &str) -> Result<Value, serde_json::Error> {
    let jv: JsonValue = serde_json::from_str(s)?;
    Ok(json_value_to_value(&jv))
}

fn value_to_json_value(v: &Value) -> JsonValue {
    match v {
        Value::Num(n) => json!({"type": "num", "value": n.to_string()}),
        Value::Str(s) => json!({"type": "str", "value": s}),
        Value::Expr(e) => json!({"type": "expr", "value": e.to_string()}),
        Value::Empty => json!({"type": "empty"}),
        Value::List(items) => {
            let arr: Vec<JsonValue> = items.iter().map(value_to_json_value).collect();
            json!({"type": "list", "value": arr})
        }
        Value::Dict(pairs) => {
            let arr: Vec<JsonValue> = pairs
                .iter()
                .map(|(k, v)| json!({"key": value_to_json_value(k), "val": value_to_json_value(v)}))
                .collect();
            json!({"type": "dict", "value": arr})
        }
    }
}

fn json_value_to_value(jv: &JsonValue) -> Value {
    let ty = jv.get("type").and_then(|t| t.as_str()).unwrap_or("empty");
    match ty {
        "num" => {
            let s = jv.get("value").and_then(|v| v.as_str()).unwrap_or("0");
            Value::Num(s.parse().unwrap_or(0))
        }
        "str" => {
            let s = jv.get("value").and_then(|v| v.as_str()).unwrap_or("");
            Value::Str(s.to_string())
        }
        "expr" => {
            // Expr roundtrip is best-effort — parse as num if possible.
            let s = jv.get("value").and_then(|v| v.as_str()).unwrap_or("0");
            Value::Num(s.parse().unwrap_or(0))
        }
        "list" => {
            let arr = jv
                .get("value")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            let items: Vec<Value> = arr.iter().map(json_value_to_value).collect();
            Value::List(items)
        }
        "dict" => {
            let arr = jv
                .get("value")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            let pairs: Vec<(Value, Value)> = arr
                .iter()
                .filter_map(|p| {
                    let k = p.get("key")?;
                    let v = p.get("val")?;
                    Some((json_value_to_value(k), json_value_to_value(v)))
                })
                .collect();
            Value::Dict(pairs)
        }
        _ => Value::Empty,
    }
}
