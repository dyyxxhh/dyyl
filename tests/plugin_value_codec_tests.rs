use dyyl::runtime::plugin::value_codec::{value_from_json, value_to_json, values_to_json_array};
use dyyl::runtime::value::Value;

#[test]
fn encode_num() {
    let json = value_to_json(&Value::Num(42));
    assert_eq!(json, r#"{"type":"num","value":"42"}"#);
}

#[test]
fn encode_str() {
    let json = value_to_json(&Value::Str("hello".to_string()));
    assert_eq!(json, r#"{"type":"str","value":"hello"}"#);
}

#[test]
fn encode_empty() {
    let json = value_to_json(&Value::Empty);
    assert_eq!(json, r#"{"type":"empty"}"#);
}

#[test]
fn encode_list() {
    let json = value_to_json(&Value::List(vec![
        Value::Num(1),
        Value::Str("a".to_string()),
    ]));
    assert_eq!(
        json,
        r#"{"type":"list","value":[{"type":"num","value":"1"},{"type":"str","value":"a"}]}"#
    );
}

#[test]
fn encode_args_array() {
    let args = vec![Value::Num(3), Value::Str("hi".to_string())];
    let json = values_to_json_array(&args);
    assert_eq!(
        json,
        r#"[{"type":"num","value":"3"},{"type":"str","value":"hi"}]"#
    );
}

#[test]
fn decode_str() {
    let v = value_from_json(r#"{"type":"str","value":"hello"}"#).unwrap();
    assert_eq!(v, Value::Str("hello".to_string()));
}

#[test]
fn decode_num() {
    let v = value_from_json(r#"{"type":"num","value":"42"}"#).unwrap();
    assert_eq!(v, Value::Num(42));
}

#[test]
fn decode_empty() {
    let v = value_from_json(r#"{"type":"empty"}"#).unwrap();
    assert_eq!(v, Value::Empty);
}

#[test]
fn decode_list() {
    let v = value_from_json(r#"{"type":"list","value":[{"type":"num","value":"1"}]}"#).unwrap();
    assert_eq!(v, Value::List(vec![Value::Num(1)]));
}

#[test]
fn roundtrip_str() {
    let original = Value::Str("test".to_string());
    let json = value_to_json(&original);
    let decoded = value_from_json(&json).unwrap();
    assert_eq!(original, decoded);
}
