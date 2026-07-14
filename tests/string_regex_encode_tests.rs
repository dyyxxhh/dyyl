#![allow(
    clippy::all,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::panic,
    clippy::expect_used,
    clippy::todo,
    clippy::unimplemented,
    clippy::as_underscore,
    clippy::fn_to_numeric_cast_any,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::redundant_pub_crate,
    clippy::missing_const_for_fn
)]
use dyyl::runtime::execute::run_script;
use dyyl::runtime::Value;

fn eval_one(source: &str) -> String {
    let out = run_script(source, false);
    match out.values.last() {
        Some(v) => v.to_string(),
        None => String::new(),
    }
}

fn eval_list_elements(source: &str) -> Vec<String> {
    let out = run_script(source, false);
    match out.values.last() {
        Some(Value::List(items)) => items.iter().map(Value::to_string).collect(),
        other => vec![other.map_or_else(|| String::new(), Value::to_string)],
    }
}

// ── split ─────────────────────────────────────────────────────────────

#[test]
fn str_split_basic() {
    let vals = eval_list_elements("str.split \"a,b,c\", \",\"");
    assert_eq!(vals, vec!["a", "b", "c"]);
}

#[test]
fn str_split_preserves_empty() {
    let vals = eval_list_elements("str.split \"a,,b\", \",\"");
    assert_eq!(vals, vec!["a", "", "b"]);
}

#[test]
fn str_split_empty_string() {
    let vals = eval_list_elements("create.str e\nstr.split $e, \",\"");
    assert_eq!(vals, vec![""]);
}

// ── join ──────────────────────────────────────────────────────────────

#[test]
fn str_join_with_list() {
    let source = "list.create mylist\n\
                   list.append mylist, \"a\"\n\
                   list.append mylist, \"b\"\n\
                   list.append mylist, \"c\"\n\
                   str.join \", \", $mylist";
    assert_eq!(eval_one(source), "a, b, c");
}

// ── start / end / contains / index ────────────────────────────────────

#[test]
fn str_start_true() {
    assert_eq!(eval_one("str.start \"hello\", \"hel\""), "1");
}

#[test]
fn str_start_false() {
    assert_eq!(eval_one("str.start \"hello\", \"xyz\""), "0");
}

#[test]
fn str_end_true() {
    assert_eq!(eval_one("str.end \"hello\", \"llo\""), "1");
}

#[test]
fn str_end_false() {
    assert_eq!(eval_one("str.end \"hello\", \"xyz\""), "0");
}

#[test]
fn str_contains_true() {
    assert_eq!(eval_one("str.contains \"hello world\", \"world\""), "1");
}

#[test]
fn str_contains_false() {
    assert_eq!(eval_one("str.contains \"hello\", \"xyz\""), "0");
}

#[test]
fn str_index_found() {
    assert_eq!(eval_one("str.index \"hello world\", \"world\""), "6");
}

#[test]
fn str_index_not_found() {
    assert_eq!(eval_one("str.index \"hello\", \"xyz\""), "-1");
}

// ── match / extract / replace.regex ───────────────────────────────────

#[test]
fn str_match_yes() {
    assert_eq!(eval_one("str.match \"hello123\", \"[0-9]+\""), "1");
}

#[test]
fn str_match_no() {
    assert_eq!(eval_one("str.match \"hello\", \"^[0-9]+$\""), "0");
}

#[test]
fn str_extract_found() {
    assert_eq!(eval_one("str.extract \"hello123world\", \"[0-9]+\""), "123");
}

#[test]
fn str_extract_not_found() {
    assert_eq!(eval_one("str.extract \"hello\", \"[0-9]+\""), "");
}

#[test]
fn str_replace_regex() {
    assert_eq!(
        eval_one("str.replace.regex \"hello123world\", \"[0-9]+\", \"X\""),
        "helloXworld"
    );
}

#[test]
fn str_match_invalid_regex_sentinel() {
    assert_eq!(eval_one("str.match \"abc\", \"(\""), "");
}

// ── escape / unescape ─────────────────────────────────────────────────

#[test]
fn str_escape() {
    assert_eq!(eval_one(r#"str.escape "a.b*c""#), r"a\.b\*c");
}

#[test]
fn str_unescape() {
    // a\.b\*c = 0x61 0x5c 0x2e 0x62 0x5c 0x2a 0x63
    assert_eq!(
        eval_one(r#"str.unescape str.decode("615c2e625c2a63", "hex")"#),
        "a.b*c"
    );
}

// ── encode / decode ───────────────────────────────────────────────────

#[test]
fn str_encode_base64() {
    assert_eq!(eval_one("str.encode \"hello\", \"base64\""), "aGVsbG8=");
}

#[test]
fn str_decode_base64() {
    assert_eq!(eval_one("str.decode \"aGVsbG8=\", \"base64\""), "hello");
}

#[test]
fn str_encode_hex() {
    assert_eq!(eval_one("str.encode \"hello\", \"hex\""), "68656c6c6f");
}

#[test]
fn str_decode_hex() {
    assert_eq!(eval_one("str.decode \"68656c6c6f\", \"hex\""), "hello");
}

#[test]
fn str_encode_url() {
    assert_eq!(
        eval_one("str.encode \"hello world\", \"url\""),
        "hello%20world"
    );
}

#[test]
fn str_decode_url() {
    assert_eq!(
        eval_one("str.decode \"hello%20world\", \"url\""),
        "hello world"
    );
}

// ── format ────────────────────────────────────────────────────────────

#[test]
fn str_format_basic() {
    let source = "create.str name\n\
                   set $name, \"Alice\"\n\
                   list.create vals\n\
                   list.append vals, $name\n\
                   list.append vals, 25\n\
                   str.format \"Hello {0}, you are {1}\", $vals";
    assert_eq!(eval_one(source), "Hello Alice, you are 25");
}

#[test]
fn str_format_single_value() {
    let source = "list.create vals\n\
                   list.append vals, \"World\"\n\
                   str.format \"Hello {0}\", $vals";
    assert_eq!(eval_one(source), "Hello World");
}

// ── to.num / from.num ─────────────────────────────────────────────────

#[test]
fn str_to_num_valid() {
    assert_eq!(eval_one("str.to.num \"42\""), "42");
}

#[test]
fn str_to_num_negative() {
    assert_eq!(eval_one("str.to.num \"-7\""), "-7");
}

#[test]
fn str_to_num_invalid() {
    assert_eq!(eval_one("str.to.num \"abc\""), "-1");
}

#[test]
fn str_from_num() {
    assert_eq!(eval_one("str.from.num 42"), "42");
}

#[test]
fn str_from_num_negative() {
    assert_eq!(eval_one("str.from.num -7"), "-7");
}
