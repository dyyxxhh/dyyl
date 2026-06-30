//! Task 8 regression guards — str.join and str.format with list values (Task 9).

use dyyl::runtime::execute::run_script;
use dyyl::runtime::Value;

fn eval_values(source: &str) -> Vec<Value> {
    let out = run_script(source, false);
    out.values
}

#[test]
fn str_join_with_list_still_works() {
    let source = "\
list.create l
list.append l, a
list.append l, b
list.append l, c
str.join \",\", $l";
    let vals = eval_values(source);
    assert_eq!(vals[4], Value::Str("a,b,c".into()));
}

#[test]
fn str_format_with_list_still_works() {
    let source = "\
list.create l
list.append l, world
str.format \"hello {0}\", $l";
    let vals = eval_values(source);
    assert_eq!(vals[2], Value::Str("hello world".into()));
}
