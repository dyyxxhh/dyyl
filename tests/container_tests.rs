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
//! Integration tests for dict commands and the combined acceptance test (Task 9).

use dyyl::runtime::execute::run_script;
use dyyl::runtime::Value;

fn eval_values(source: &str) -> Vec<Value> {
    let out = run_script(source, false);
    out.values
}

#[test]
fn containers_all_dict_list_commands() {
    let source = "\
dict.create d
dict.set d, \"a\", 1
dict.set d, \"b\", 2
dict.set d, \"c\", 3
dict.get d, \"a\"
dict.get d, \"b\"
dict.has d, \"a\"
dict.has d, \"z\"
dict.del d, \"b\"
dict.has d, \"b\"
dict.len d
dict.set d, \"b\", 10
dict.get d, \"b\"
dict.keys d
dict.vals d
list.create l
list.append l, 10
list.append l, 20
list.append l, 30
list.len l
list.get l, 0
list.get l, 1
list.get l, 2
list.insert l, 1, 15
list.get l, 0
list.get l, 1
list.get l, 2
list.len l
list.remove l, 0
list.get l, 0
list.contains l, 15
list.contains l, 99
list.index l, 15
list.index l, 99
list.join l, \",\"
list.reverse l
list.get l, 0
list.get l, 1
list.sort l
list.slice l, 0, 2";

    let vals = eval_values(source);
    let expected: Vec<&dyn std::fmt::Display> = vec![
        &"empty",
        &"empty",
        &"empty",
        &"empty",
        &"1",
        &"2",
        &"1",
        &"0",
        &"empty",
        &"0",
        &"2",
        &"empty",
        &"10",
        &"[a, c, b]",
        &"[1, 3, 10]",
        &"empty",
        &"0",
        &"1",
        &"2",
        &"3",
        &"10",
        &"20",
        &"30",
        &"empty",
        &"10",
        &"15",
        &"20",
        &"4",
        &"10",
        &"15",
        &"1",
        &"0",
        &"0",
        &"-1",
        &"15,20,30",
        &"empty",
        &"30",
        &"20",
        &"empty",
        &"[15, 20]",
    ];

    assert_eq!(
        vals.len(),
        expected.len(),
        "expected {} results, got {} values",
        expected.len(),
        vals.len()
    );
    for (i, exp) in expected.iter().enumerate() {
        let exp_str = exp.to_string();
        assert_eq!(
            vals[i].to_string(),
            exp_str,
            "mismatch at index {i}: expected {exp_str}"
        );
    }
}

#[test]
fn dict_string_keys() {
    let source = "\
dict.create d
dict.set d, \"x\", 42
dict.get d, \"x\"";
    let vals = eval_values(source);
    assert_eq!(vals[2], Value::Num(42));
}

#[test]
fn dict_numeric_keys() {
    let source = "\
dict.create d
dict.set d, 1, 100
dict.get d, 1
dict.get d, 2";
    let vals = eval_values(source);
    assert_eq!(vals[2], Value::Num(100));
    assert_eq!(vals[3], Value::Num(-1));
}

#[test]
fn dict_keys_order() {
    let source = "\
dict.create d
dict.set d, \"c\", 3
dict.set d, \"a\", 1
dict.set d, \"b\", 2
dict.keys d";
    let vals = eval_values(source);
    assert_eq!(vals[4].to_string(), "[c, a, b]");
}

#[test]
fn dict_vals_match_keys() {
    let source = "\
dict.create d
dict.set d, \"x\", 10
dict.set d, \"y\", 20
dict.vals d";
    let vals = eval_values(source);
    assert_eq!(vals[3].to_string(), "[10, 20]");
}

#[test]
fn dict_len_after_set_del() {
    let source = "\
dict.create d
dict.set d, \"a\", 1
dict.set d, \"b\", 2
dict.len d
dict.del d, \"a\"
dict.len d
dict.del d, \"b\"
dict.len d";
    let vals = eval_values(source);
    assert_eq!(vals[3], Value::Num(2));
    assert_eq!(vals[5], Value::Num(1));
    assert_eq!(vals[7], Value::Num(0));
}

#[test]
fn dict_set_overwrites_existing() {
    let source = "\
dict.create d
dict.set d, \"k\", 1
dict.set d, \"k\", 99
dict.get d, \"k\"";
    let vals = eval_values(source);
    assert_eq!(vals[3], Value::Num(99));
}

#[test]
fn dict_get_missing_returns_minus_one() {
    let source = "\
dict.create d
dict.get d, \"nope\"";
    let vals = eval_values(source);
    assert_eq!(vals[1], Value::Num(-1));
}
