//! Integration tests for simple (non-control-flow) logic commands.
//! Control flow tests are in logic_control_flow_tests.rs.

use dyyl::runtime::execute::run_script;
use dyyl::runtime::Value;

fn exec_one(source: &str) -> Value {
    let output = run_script(source, false);
    assert_eq!(output.values.len(), 1);
    output.values.into_iter().next().unwrap()
}

fn assert_one(source: &str, expected: Value) {
    assert_eq!(exec_one(source), expected, "source: {source:?}");
}

#[test]
fn logic_un_zero() {
    assert_one("logic.un 0", Value::Num(1));
}
#[test]
fn logic_un_one() {
    assert_one("logic.un 1", Value::Num(0));
}
#[test]
fn logic_un_nonzero() {
    assert_one("logic.un 5", Value::Num(0));
}

#[test]
fn logic_and_both_true() {
    assert_one("logic.and 1, 1", Value::Num(1));
}
#[test]
fn logic_and_one_false() {
    assert_one("logic.and 1, 0", Value::Num(0));
}
#[test]
fn logic_and_both_false() {
    assert_one("logic.and 0, 0", Value::Num(0));
}

#[test]
fn logic_or_both_false() {
    assert_one("logic.or 0, 0", Value::Num(0));
}
#[test]
fn logic_or_one_true() {
    assert_one("logic.or 1, 0", Value::Num(1));
}
#[test]
fn logic_or_both_true() {
    assert_one("logic.or 1, 1", Value::Num(1));
}

#[test]
fn logic_same_equal() {
    assert_one("logic.same 3, 3", Value::Num(1));
}
#[test]
fn logic_same_not_equal() {
    assert_one("logic.same 3, 4", Value::Num(0));
}

#[test]
fn logic_not_same_equal() {
    assert_one("logic.not.same 3, 3", Value::Num(0));
}
#[test]
fn logic_not_same_not_equal() {
    assert_one("logic.not.same 3, 4", Value::Num(1));
}

#[test]
fn logic_more_true() {
    assert_one("logic.more 5, 3", Value::Num(1));
}
#[test]
fn logic_more_false() {
    assert_one("logic.more 3, 5", Value::Num(0));
}
#[test]
fn logic_more_equal_false() {
    assert_one("logic.more 5, 5", Value::Num(0));
}

#[test]
fn logic_less_true() {
    assert_one("logic.less 3, 5", Value::Num(1));
}
#[test]
fn logic_less_false() {
    assert_one("logic.less 5, 3", Value::Num(0));
}

#[test]
fn logic_more_same_true_gt() {
    assert_one("logic.more.same 5, 3", Value::Num(1));
}
#[test]
fn logic_more_same_true_eq() {
    assert_one("logic.more.same 5, 5", Value::Num(1));
}
#[test]
fn logic_more_same_false() {
    assert_one("logic.more.same 3, 5", Value::Num(0));
}

#[test]
fn logic_less_same_true_lt() {
    assert_one("logic.less.same 3, 5", Value::Num(1));
}
#[test]
fn logic_less_same_true_eq() {
    assert_one("logic.less.same 5, 5", Value::Num(1));
}
#[test]
fn logic_less_same_false() {
    assert_one("logic.less.same 5, 3", Value::Num(0));
}

#[test]
fn logic_max_first() {
    assert_one("logic.max 10, 3", Value::Num(10));
}
#[test]
fn logic_max_second() {
    assert_one("logic.max 3, 10", Value::Num(10));
}
#[test]
fn logic_min_first() {
    assert_one("logic.min 3, 10", Value::Num(3));
}
#[test]
fn logic_min_second() {
    assert_one("logic.min 10, 3", Value::Num(3));
}

#[test]
fn logic_between_inside() {
    assert_one("logic.between 5, 1, 10", Value::Num(1));
}
#[test]
fn logic_between_below() {
    assert_one("logic.between 0, 1, 10", Value::Num(0));
}
#[test]
fn logic_between_above() {
    assert_one("logic.between 15, 1, 10", Value::Num(0));
}
#[test]
fn logic_between_lower() {
    assert_one("logic.between 1, 1, 10", Value::Num(1));
}
#[test]
fn logic_between_upper() {
    assert_one("logic.between 10, 1, 10", Value::Num(1));
}

#[test]
fn logic_clamp_below() {
    assert_one("logic.clamp 0, 1, 10", Value::Num(1));
}
#[test]
fn logic_clamp_above() {
    assert_one("logic.clamp 15, 1, 10", Value::Num(10));
}
#[test]
fn logic_clamp_inside() {
    assert_one("logic.clamp 5, 1, 10", Value::Num(5));
}

#[test]
fn logic_is_num_num() {
    assert_one("logic.is.num 42", Value::Num(1));
}
#[test]
fn logic_is_num_str() {
    assert_one("logic.is.num hello", Value::Num(0));
}

#[test]
fn logic_is_str_str() {
    assert_one("logic.is.str hello", Value::Num(1));
}
#[test]
fn logic_is_str_num() {
    assert_one("logic.is.str 42", Value::Num(0));
}

#[test]
fn logic_is_empty_zero() {
    assert_one("logic.is.empty 0", Value::Num(1));
}
#[test]
fn logic_is_empty_one() {
    assert_one("logic.is.empty 1", Value::Num(0));
}
#[test]
fn logic_is_empty_str_nonempty() {
    assert_one("logic.is.empty hello", Value::Num(0));
}
