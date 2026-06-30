//! Conversion operations: encode, decode, format, to.num, from.num.

use super::str::{resolve_str_arg, resolve_val, StrCtx};
use crate::i18n;
use crate::runtime::error::RuntimeError;
use crate::runtime::value::Value;

pub(super) fn dispatch_convert(sub: &str, sc: &mut StrCtx) -> Result<Value, RuntimeError> {
    match sub {
        "encode" => do_encode(sc),
        "decode" => do_decode(sc),
        "format" => do_format(sc),
        "to.num" => do_to_num(sc),
        "from.num" => do_from_num(sc),
        _ => Err(RuntimeError::new(
            sc.exec.line,
            &sc.call.command,
            i18n::unknown_command(sc.exec.lang.get(), "str.convert", sub),
        )),
    }
}

fn do_encode(sc: &mut StrCtx) -> Result<Value, RuntimeError> {
    let s = resolve_str_arg(sc, 0)?;
    let encoding = resolve_str_arg(sc, 1)?;
    match encoding.as_str() {
        "base64" => Ok(Value::Str(base64_encode(s.as_bytes()))),
        "hex" => Ok(Value::Str(hex::encode(s.as_bytes()))),
        "url" => Ok(Value::Str(url_encode(&s))),
        _ => Ok(Value::sentinel_str()),
    }
}

fn do_decode(sc: &mut StrCtx) -> Result<Value, RuntimeError> {
    let s = resolve_str_arg(sc, 0)?;
    let encoding = resolve_str_arg(sc, 1)?;
    match encoding.as_str() {
        "base64" => match base64_decode(&s) {
            Ok(bytes) => match String::from_utf8(bytes) {
                Ok(valid) => Ok(Value::Str(valid)),
                Err(_) => Ok(Value::sentinel_str()),
            },
            Err(_) => Ok(Value::sentinel_str()),
        },
        "hex" => match hex::decode(&s) {
            Ok(bytes) => match String::from_utf8(bytes) {
                Ok(valid) => Ok(Value::Str(valid)),
                Err(_) => Ok(Value::sentinel_str()),
            },
            Err(_) => Ok(Value::sentinel_str()),
        },
        "url" => Ok(Value::Str(url_decode(&s))),
        _ => Ok(Value::sentinel_str()),
    }
}

fn do_format(sc: &mut StrCtx) -> Result<Value, RuntimeError> {
    let template = resolve_str_arg(sc, 0)?;
    let val = resolve_val(sc, 1)?;
    let values: Vec<String> = match val {
        Value::List(items) => items.iter().map(|v| v.to_string()).collect(),
        other => vec![other.to_string()],
    };
    let mut result = template;
    for (idx, val) in values.iter().enumerate() {
        let placeholder = format!("{{{idx}}}");
        result = result.replace(&placeholder, val);
    }
    Ok(Value::Str(result))
}

fn do_to_num(sc: &mut StrCtx) -> Result<Value, RuntimeError> {
    let s = resolve_str_arg(sc, 0)?;
    match s.trim().parse::<i64>() {
        Ok(n) => Ok(Value::Num(n)),
        Err(_) => Ok(Value::Num(-1)),
    }
}

fn do_from_num(sc: &mut StrCtx) -> Result<Value, RuntimeError> {
    let val = resolve_val(sc, 0)?;
    Ok(Value::Str(val.to_string()))
}

// ── Base64 (minimal implementation) ───────────────────────────────────

const B64_TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn base64_encode(data: &[u8]) -> String {
    let mut result = String::with_capacity((data.len() + 2) / 3 * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;
        result.push(B64_TABLE[((triple >> 18) & 0x3F) as usize] as char);
        result.push(B64_TABLE[((triple >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            result.push(B64_TABLE[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
        if chunk.len() > 2 {
            result.push(B64_TABLE[(triple & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
    }
    result
}

fn base64_decode(s: &str) -> Result<Vec<u8>, ()> {
    let s: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    if s.len() % 4 != 0 {
        return Err(());
    }
    let mut result = Vec::with_capacity(s.len() * 3 / 4);
    for chunk in s.as_bytes().chunks(4) {
        let vals: Vec<u32> = chunk
            .iter()
            .map(|&b| match b {
                b'A'..=b'Z' => Ok((b - b'A') as u32),
                b'a'..=b'z' => Ok((b - b'a' + 26) as u32),
                b'0'..=b'9' => Ok((b - b'0' + 52) as u32),
                b'+' => Ok(62),
                b'/' => Ok(63),
                b'=' => Ok(0),
                _ => Err(()),
            })
            .collect::<Result<Vec<u32>, ()>>()?;
        if vals.len() != 4 {
            return Err(());
        }
        let triple = (vals[0] << 18) | (vals[1] << 12) | (vals[2] << 6) | vals[3];
        result.push((triple >> 16) as u8);
        if chunk[2] != b'=' {
            result.push((triple >> 8) as u8);
        }
        if chunk[3] != b'=' {
            result.push(triple as u8);
        }
    }
    Ok(result)
}

// ── URL encoding (percent encoding) ───────────────────────────────────

fn url_encode(s: &str) -> String {
    let mut result = String::with_capacity(s.len() * 3);
    for byte in s.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                result.push(byte as char);
            }
            _ => {
                result.push('%');
                result.push_str(&format!("{byte:02X}"));
            }
        }
    }
    result
}

fn url_decode(s: &str) -> String {
    let mut decoded = Vec::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => {
                let hi = hex_digit(bytes[i + 1]);
                let lo = hex_digit(bytes[i + 2]);
                if let (Some(h), Some(l)) = (hi, lo) {
                    decoded.push(h * 16 + l);
                    i += 3;
                } else {
                    decoded.push(b'%');
                    i += 1;
                }
            }
            b'+' => {
                decoded.push(b' ');
                i += 1;
            }
            b => {
                decoded.push(b);
                i += 1;
            }
        }
    }
    match String::from_utf8(decoded) {
        Ok(s) => s,
        Err(e) => String::from_utf8_lossy(e.as_bytes()).into_owned(),
    }
}

fn hex_digit(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}
