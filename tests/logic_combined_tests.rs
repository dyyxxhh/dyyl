//! Combined acceptance test for all logic commands and control flow.

use dyyl::runtime::execute::run_script;
use dyyl::runtime::Value;

fn exec_multi(source: &str) -> Vec<Value> {
    run_script(source, false).values
}

#[test]
fn logic_all_commands_and_control_flow() {
    let v = exec_multi(
        "\
logic.un 0
logic.and 1, 1
logic.or 0, 1
logic.same 3, 3
logic.not.same 3, 4
logic.more 5, 3
logic.less 3, 5
logic.more.same 5, 5
logic.less.same 3, 3
logic.max 3, 10
logic.min 3, 10
logic.between 5, 1, 10
logic.clamp 15, 1, 10
logic.is.num 42
logic.is.str hello
logic.is.empty 0
create.num result
logic.if 1, 1
  set $result, 1
logic.else 1, 1
  set $result, 2
create.num counter
logic.while logic.less($counter, 3), 1
  set $counter, math.add($counter, 1)
create.num acc
logic.for 2, 1
  set $acc, math.add($acc, 10)
",
    );
    assert_eq!(v[0], Value::Num(1), "un 0");
    assert_eq!(v[1], Value::Num(1), "and");
    assert_eq!(v[2], Value::Num(1), "or");
    assert_eq!(v[3], Value::Num(1), "same");
    assert_eq!(v[4], Value::Num(1), "not.same");
    assert_eq!(v[5], Value::Num(1), "more");
    assert_eq!(v[6], Value::Num(1), "less");
    assert_eq!(v[7], Value::Num(1), "more.same");
    assert_eq!(v[8], Value::Num(1), "less.same");
    assert_eq!(v[9], Value::Num(10), "max");
    assert_eq!(v[10], Value::Num(3), "min");
    assert_eq!(v[11], Value::Num(1), "between");
    assert_eq!(v[12], Value::Num(10), "clamp");
    assert_eq!(v[13], Value::Num(1), "is.num");
    assert_eq!(v[14], Value::Num(1), "is.str");
    assert_eq!(v[15], Value::Num(1), "is.empty");
    assert_eq!(v[16], Value::Num(0), "create.num result");
    assert_eq!(v[17], Value::Num(1), "if body: set $result, 1");
    assert_eq!(v[18], Value::Num(1), "if true returns 1");
    assert_eq!(v[19], Value::Num(0), "else does not fire → 0");
    assert_eq!(v[20], Value::Num(0), "create.num counter");
    assert_eq!(v[24], Value::Num(3), "while 3 iterations");
    assert_eq!(v[25], Value::Num(0), "create.num acc");
    assert_eq!(v[28], Value::Num(2), "for 2 iterations");
}
