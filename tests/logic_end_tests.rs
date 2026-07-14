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
use dyyl::math::CasNumber;
use dyyl::parser::parse_source;
use dyyl::runtime::execute::run_script;
use dyyl::runtime::execute::scan_open_blocks;
use dyyl::runtime::Value;

#[test]
fn scan_open_blocks_finds_matching_end() {
    let src = "logic.if 1, _\n  io.out a\nlogic.end\n";
    let cmds = parse_source(src).expect("parse");
    let map = scan_open_blocks(&cmds);
    // logic.end is the 3rd command → 0-based index 2.
    assert_eq!(map.get(&0), Some(&2));
}

#[test]
fn scan_open_blocks_handles_nesting() {
    let src = "\
logic.while 1, _
  logic.if 1, _
    io.out x
  logic.end
  io.out y
logic.end
";
    let cmds = parse_source(src).expect("parse");
    let map = scan_open_blocks(&cmds);
    assert_eq!(map.get(&0), Some(&(cmds.len() - 1)));
    // inner logic.end is the 4th command → 0-based index 3.
    assert_eq!(map.get(&1), Some(&3));
}

#[test]
fn scan_open_blocks_ignores_explicit_line_counts() {
    let src = "logic.if 1, 1\n  io.out a\n";
    let cmds = parse_source(src).expect("parse");
    let map = scan_open_blocks(&cmds);
    assert!(
        map.is_empty(),
        "explicit line count should not be in open-block map"
    );
}

#[test]
fn scan_open_blocks_mixed_explicit_and_open() {
    let src = "\
logic.if 1, 1
  io.out a
logic.while 1, _
  io.out b
logic.end
";
    let cmds = parse_source(src).expect("parse");
    let map = scan_open_blocks(&cmds);
    // logic.end is the 5th command → 0-based index 4.
    assert_eq!(map.get(&2), Some(&4));
}

// ── Task 15: open-block execution integration tests ────────────────

#[test]
fn logic_if_open_block_executes_body() {
    let v = run_script(
        "create.num x\nset $x, 0\nlogic.if 1, _\n  set $x, 42\nlogic.end\nio.out $x\n",
        false,
    )
    .values;
    assert_eq!(v[3], Value::Num(1), "if true returns 1");
    assert_eq!(v[5], Value::Num(42), "body executed");
}

#[test]
fn logic_if_open_block_false_skips_body() {
    let v = run_script(
        "create.num x\nset $x, 0\nlogic.if 0, _\n  set $x, 99\nlogic.end\nio.out $x\n",
        false,
    )
    .values;
    // if false → 体被跳过（不产生体值），故 if 结果紧随 set 之后位于 v[2]；
    // logic.end 仍被执行（匹配，v[3]=1），io.out 落在 v[4]，x 保持 0。
    assert_eq!(v[2], Value::Num(0), "if false returns 0");
    assert_eq!(v[4], Value::Num(0), "body skipped");
}

#[test]
fn logic_while_open_block_loops() {
    let v = run_script(
        "create.num i\nset $i, 0\nlogic.while logic.less($i, 3), _\n  set $i, math.add($i, 1)\nlogic.end\nio.out $i\n",
        false,
    ).values;
    assert_eq!(v[5], Value::Num(3), "while ran 3 times");
}

#[test]
fn logic_for_open_block_loops() {
    let v = run_script(
        "create.num sum\nset $sum, 0\nlogic.for 3, _\n  set $sum, math.add($sum, 1)\nlogic.end\nio.out $sum\n",
        false,
    ).values;
    assert_eq!(v[5], Value::Num(3), "for ran 3 times");
}

#[test]
fn logic_nested_open_blocks() {
    let v = run_script(
        "\
create.num i
set $i, 0
logic.while logic.less($i, 2), _
  logic.if logic.more($i, 0), _
    set $i, math.add($i, 10)
  logic.end
  set $i, math.add($i, 1)
logic.end
io.out $i
",
        false,
    )
    .values;
    // i=0: if false, i=1; i=1: if true i=11, then i=12; 12>=2 stop
    // 注：用 logic.more 而非 logic.same —— logic.same 走 Value PartialEq，
    // 对 math.add 产生的 Value::Expr 视为不等于 Num 字面量（既有语义，非本任务范围）；
    // logic.more 走数值比较，可精确复现上方注释描述的计划意图。
    // if 体在 iter2 执行 → 多一个体值，io.out 落在 v[11]，且经 math.add 为 Expr。
    assert_eq!(
        v[11],
        Value::Expr(CasNumber::Int(12)),
        "nested open blocks work"
    );
}

#[test]
fn logic_end_without_open_block_returns_sentinel() {
    let v = run_script("logic.end\n", false).values;
    assert_eq!(
        v[0],
        Value::Num(0),
        "logic.end without open block returns 0"
    );
}

#[test]
fn mixed_explicit_and_open_blocks() {
    let v = run_script(
        "\
create.num x
set $x, 1
logic.if 1, 1
  set $x, 2
logic.while logic.less($x, 5), _
  set $x, math.add($x, 1)
logic.end
io.out $x
",
        false,
    )
    .values;
    assert_eq!(v[9], Value::Expr(CasNumber::Int(5)), "mixed blocks work");
}
