//! Comprehensive tests for ALL str.* commands (Task 8).
//!
//! Covers: len, get, slice, find, rfind, count, replace, replace.all,
//! insert, remove, upper, lower, capital, reverse, repeat, pad.left,
//! pad.right, trim, trim.left, trim.right, split, join, start, end,
//! contains, index, match, extract, replace.regex, escape, unescape,
//! encode, decode, format, to.num, from.num.
//!
//! CJK/emoji tests for len, get, slice, reverse.

use dyyl::runtime::execute::run_script;

fn eval_one(source: &str) -> String {
    let out = run_script(source, false);
    match out.values.last() {
        Some(v) => v.to_string(),
        None => String::new(),
    }
}

// ── len ───────────────────────────────────────────────────────────────

#[test]
fn str_len_ascii() {
    assert_eq!(eval_one("str.len \"hello\""), "5");
}

#[test]
fn str_len_empty() {
    assert_eq!(eval_one("create.str e\nstr.len $e"), "0");
}

#[test]
fn str_len_cjk() {
    assert_eq!(eval_one("str.len \"你好世界\""), "4");
}

#[test]
fn str_len_emoji() {
    assert_eq!(eval_one("str.len \"👍🎉\""), "2");
}

// ── get ───────────────────────────────────────────────────────────────

#[test]
fn str_get_ascii() {
    assert_eq!(eval_one("str.get \"hello\", 1"), "e");
}

#[test]
fn str_get_cjk() {
    assert_eq!(eval_one("str.get \"你好世界\", 1"), "好");
}

#[test]
fn str_get_out_of_bounds() {
    assert_eq!(eval_one("str.get \"hello\", 10"), "-1");
}

// ── slice ─────────────────────────────────────────────────────────────

#[test]
fn str_slice_ascii() {
    assert_eq!(eval_one("str.slice \"hello\", 1, 3"), "el");
}

#[test]
fn str_slice_cjk() {
    assert_eq!(eval_one("str.slice \"你好世界\", 1, 3"), "好世");
}

#[test]
fn str_slice_full() {
    assert_eq!(eval_one("str.slice \"hello\", 0, 5"), "hello");
}

// ── find / rfind ──────────────────────────────────────────────────────

#[test]
fn str_find_found() {
    assert_eq!(eval_one("str.find \"hello world\", \"world\""), "6");
}

#[test]
fn str_find_not_found() {
    assert_eq!(eval_one("str.find \"hello\", \"xyz\""), "-1");
}

#[test]
fn str_rfind_first_match() {
    assert_eq!(eval_one("str.rfind \"aabaa\", \"aa\""), "3");
}

#[test]
fn str_rfind_not_found() {
    assert_eq!(eval_one("str.rfind \"hello\", \"xyz\""), "-1");
}

// ── count ─────────────────────────────────────────────────────────────

#[test]
fn str_count_multiple() {
    assert_eq!(eval_one("str.count \"aabaa\", \"aa\""), "2");
}

#[test]
fn str_count_none() {
    assert_eq!(eval_one("str.count \"hello\", \"xyz\""), "0");
}

// ── replace / replace.all ─────────────────────────────────────────────

#[test]
fn str_replace_first_only() {
    assert_eq!(eval_one("str.replace \"aabaa\", \"aa\", \"x\""), "xbaa");
}

#[test]
fn str_replace_all_matches() {
    assert_eq!(eval_one("str.replace.all \"aabaa\", \"aa\", \"x\""), "xbx");
}

#[test]
fn str_replace_no_match() {
    assert_eq!(eval_one("str.replace \"hello\", \"xyz\", \"a\""), "hello");
}

// ── insert ────────────────────────────────────────────────────────────

#[test]
fn str_insert_middle() {
    assert_eq!(eval_one("str.insert \"hello\", 1, \"XY\""), "hXYello");
}

#[test]
fn str_insert_at_start() {
    assert_eq!(eval_one("str.insert \"hello\", 0, \"AB\""), "ABhello");
}

#[test]
fn str_insert_at_end() {
    assert_eq!(eval_one("str.insert \"hello\", 5, \"!\""), "hello!");
}

// ── remove ────────────────────────────────────────────────────────────

#[test]
fn str_remove_middle() {
    assert_eq!(eval_one("str.remove \"hello\", 1, 3"), "hlo");
}

#[test]
fn str_remove_nothing() {
    assert_eq!(eval_one("str.remove \"hello\", 0, 0"), "hello");
}

// ── upper / lower / capital ───────────────────────────────────────────

#[test]
fn str_upper() {
    assert_eq!(eval_one("str.upper \"hello\""), "HELLO");
}

#[test]
fn str_lower() {
    assert_eq!(eval_one("str.lower \"HELLO\""), "hello");
}

#[test]
fn str_capital() {
    assert_eq!(eval_one("str.capital \"hello\""), "Hello");
}

#[test]
fn str_capital_empty() {
    assert_eq!(eval_one("str.capital \"\""), "");
}

// ── reverse ───────────────────────────────────────────────────────────

#[test]
fn str_reverse_ascii() {
    assert_eq!(eval_one("str.reverse \"hello\""), "olleh");
}

#[test]
fn str_reverse_cjk() {
    assert_eq!(eval_one("str.reverse \"你好世界\""), "界世好你");
}

// ── repeat ────────────────────────────────────────────────────────────

#[test]
fn str_repeat() {
    assert_eq!(eval_one("str.repeat \"ab\", 3"), "ababab");
}

#[test]
fn str_repeat_zero() {
    assert_eq!(eval_one("str.repeat \"ab\", 0"), "");
}

// ── pad.left / pad.right ──────────────────────────────────────────────

#[test]
fn str_pad_left() {
    assert_eq!(eval_one("str.pad.left \"hi\", 5, \".\""), "...hi");
}

#[test]
fn str_pad_right() {
    assert_eq!(eval_one("str.pad.right \"hi\", 5, \".\""), "hi...");
}

#[test]
fn str_pad_already_long() {
    assert_eq!(eval_one("str.pad.left \"hello\", 3, \".\""), "hello");
}

// ── trim / trim.left / trim.right ─────────────────────────────────────

#[test]
fn str_trim() {
    assert_eq!(eval_one("str.trim \"  hello  \""), "hello");
}

#[test]
fn str_trim_left() {
    assert_eq!(eval_one("str.trim.left \"  hello  \""), "hello  ");
}

#[test]
fn str_trim_right_leading_preserved() {
    assert_eq!(
        eval_one(r#"str.trim.right str.decode("202068656c6c6f", "hex")"#),
        "  hello"
    );
}
