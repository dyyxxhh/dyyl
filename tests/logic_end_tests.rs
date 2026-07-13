use dyyl::runtime::execute::scan_open_blocks;
use dyyl::parser::parse_source;

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
    assert!(map.is_empty(), "explicit line count should not be in open-block map");
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
