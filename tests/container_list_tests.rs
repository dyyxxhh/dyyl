//! List command unit tests (Task 9).

use dyyl::runtime::execute::run_script;
use dyyl::runtime::Value;

fn eval_values(source: &str) -> Vec<Value> {
    let out = run_script(source, false);
    out.values
}

#[test]
fn list_append_persists_in_env() {
    let source = "\
list.create l
list.append l, 5
list.append l, 6
list.get l, 0
list.get l, 1
list.len l";
    let vals = eval_values(source);
    assert_eq!(vals[3], Value::Num(5));
    assert_eq!(vals[4], Value::Num(6));
    assert_eq!(vals[5], Value::Num(2));
}

#[test]
fn list_insert_at_middle() {
    let source = "\
list.create l
list.append l, 1
list.append l, 3
list.insert l, 1, 2
list.get l, 0
list.get l, 1
list.get l, 2";
    let vals = eval_values(source);
    assert_eq!(vals[4], Value::Num(1));
    assert_eq!(vals[5], Value::Num(2));
    assert_eq!(vals[6], Value::Num(3));
}

#[test]
fn list_remove_returns_removed_element() {
    let source = "\
list.create l
list.append l, 10
list.append l, 20
list.remove l, 0";
    let vals = eval_values(source);
    assert_eq!(vals[3], Value::Num(10));
}

#[test]
fn list_remove_oob_returns_minus_one() {
    let source = "\
list.create l
list.append l, 1
list.remove l, 5";
    let vals = eval_values(source);
    assert_eq!(vals[2], Value::Num(-1));
}

#[test]
fn list_join_with_separator() {
    let source = "\
list.create l
list.append l, 1
list.append l, 2
list.append l, 3
list.join l, \"-\"";
    let vals = eval_values(source);
    assert_eq!(vals[4], Value::Str("1-2-3".to_string()));
}

#[test]
fn list_contains_found_and_not_found() {
    let source = "\
list.create l
list.append l, 10
list.append l, 20
list.contains l, 10
list.contains l, 30";
    let vals = eval_values(source);
    assert_eq!(vals[3], Value::Num(1));
    assert_eq!(vals[4], Value::Num(0));
}

#[test]
fn list_index_found_and_not_found() {
    let source = "\
list.create l
list.append l, 10
list.append l, 20
list.index l, 20
list.index l, 30";
    let vals = eval_values(source);
    assert_eq!(vals[3], Value::Num(1));
    assert_eq!(vals[4], Value::Num(-1));
}

#[test]
fn list_reverse_in_place() {
    let source = "\
list.create l
list.append l, 1
list.append l, 2
list.append l, 3
list.reverse l
list.get l, 0
list.get l, 1
list.get l, 2";
    let vals = eval_values(source);
    assert_eq!(vals[5], Value::Num(3));
    assert_eq!(vals[6], Value::Num(2));
    assert_eq!(vals[7], Value::Num(1));
}

#[test]
fn list_sort_all_numbers() {
    let source = "\
list.create l
list.append l, 30
list.append l, 10
list.append l, 20
list.sort l
list.get l, 0
list.get l, 1
list.get l, 2";
    let vals = eval_values(source);
    assert_eq!(vals[5], Value::Num(10));
    assert_eq!(vals[6], Value::Num(20));
    assert_eq!(vals[7], Value::Num(30));
}

#[test]
fn list_sort_all_strings() {
    let source = "\
list.create l
list.append l, cherry
list.append l, apple
list.append l, banana
list.sort l
list.get l, 0
list.get l, 1
list.get l, 2";
    let vals = eval_values(source);
    assert_eq!(vals[5], Value::Str("apple".into()));
    assert_eq!(vals[6], Value::Str("banana".into()));
    assert_eq!(vals[7], Value::Str("cherry".into()));
}

#[test]
fn list_sort_mixed_numbers_and_strings() {
    let source = "\
list.create l
list.append l, banana
list.append l, 3
list.append l, apple
list.append l, 1
list.sort l
list.get l, 0
list.get l, 1
list.get l, 2
list.get l, 3";
    let vals = eval_values(source);
    assert_eq!(vals[6], Value::Num(1));
    assert_eq!(vals[7], Value::Num(3));
    assert_eq!(vals[8], Value::Str("apple".into()));
    assert_eq!(vals[9], Value::Str("banana".into()));
}

#[test]
fn list_slice_basic() {
    let source = "\
list.create l
list.append l, 10
list.append l, 20
list.append l, 30
list.append l, 40
list.slice l, 1, 3";
    let vals = eval_values(source);
    assert_eq!(vals[5].to_string(), "[20, 30]");
}

#[test]
fn list_slice_empty_result() {
    let source = "\
list.create l
list.append l, 10
list.slice l, 0, 0";
    let vals = eval_values(source);
    assert_eq!(vals[2].to_string(), "[]");
}

#[test]
fn list_get_oob_returns_minus_one() {
    let source = "\
list.create l
list.append l, 1
list.get l, 5";
    let vals = eval_values(source);
    assert_eq!(vals[2], Value::Num(-1));
}

#[test]
fn list_get_negative_index_returns_minus_one() {
    let source = "\
list.create l
list.append l, 1
list.get l, -1";
    let vals = eval_values(source);
    assert_eq!(vals[2], Value::Num(-1));
}
