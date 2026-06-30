//! Comprehensive ordered-acceptance test for ALL math.* commands (Task 5).

use dyyl::runtime::execute::run_script;
use dyyl::runtime::Value;

const PI: &str = "\u{03C0}";
const TAU: &str = "\u{03C4}";
const SQRT: &str = "\u{221A}";
const ONE_HALF: &str = "\u{00BD}";
const ONE_THIRD: &str = "\u{2153}";

#[test]
fn math_all_commands_cas_display_and_char_arithmetic() {
    let source = format!(
        "math.add 3, 4\n\
         math.sub 10, 3\n\
         math.multi 6, 7\n\
         math.div 10, 3\n\
         math.strike -7, 2\n\
         math.surplus -7, 2\n\
         math.pow 2, 3\n\
         math.pow 2, 1/2\n\
         math.sqrt 9\n\
         math.abs -5\n\
         math.sin math.div {PI}, 6\n\
         math.cos 0\n\
         math.tan math.div {PI}, 4\n\
         math.asin 1\n\
         math.acos 1\n\
         math.acos 0\n\
         math.atan 1\n\
         math.ln math.e\n\
         math.lg 100\n\
         math.log 8, 2\n\
         math.exp 0\n\
         math.round -1/2\n\
         math.floor 7/3\n\
         math.ceil -7/3\n\
         math.pi\n\
         math.e\n\
         math.tau\n\
         math.hash hello, md5\n"
    );
    let output = run_script(&source, false);
    let strings: Vec<String> = output.values.iter().map(Value::to_string).collect();
    assert_eq!(strings.len(), 28, "expected 28 results, got {strings:?}");

    // 0: math.add 3, 4 → "7"
    assert_eq!(strings[0], "7");
    // 1: math.sub 10, 3 → "7"
    assert_eq!(strings[1], "7");
    // 2: math.multi 6, 7 → "42"
    assert_eq!(strings[2], "42");
    // 3: math.div 10, 3 → "3⅓"
    assert_eq!(strings[3], format!("3{ONE_THIRD}"));
    // 4: math.strike -7, 2 → "-3"
    assert_eq!(strings[4], "-3");
    // 5: math.surplus -7, 2 → "-1"
    assert_eq!(strings[5], "-1");

    // 6: math.pow 2, 3 → "8"
    assert_eq!(strings[6], "8");
    // 7: math.pow 2, 1/2 → "√2"
    assert_eq!(strings[7], format!("{SQRT}2"));
    // 8: math.sqrt 9 → "3"
    assert_eq!(strings[8], "3");
    // 9: math.abs -5 → "5"
    assert_eq!(strings[9], "5");

    // 10: sin(π/6) → "½"
    assert_eq!(strings[10], ONE_HALF);
    // 11: cos(0) → "1"
    assert_eq!(strings[11], "1");
    // 12: tan(π/4) → "1"
    assert_eq!(strings[12], "1");
    // 13: asin(1) → "½ × π"
    assert_eq!(strings[13], format!("{ONE_HALF} \u{00D7} {PI}"));
    // 14: acos(1) → "0"
    assert_eq!(strings[14], "0");
    // 15: acos(0) → "½ × π"
    assert_eq!(strings[15], format!("{ONE_HALF} \u{00D7} {PI}"));
    // 16: atan(1) → "¼ × π"
    assert_eq!(strings[16], format!("\u{00BC} \u{00D7} {PI}"));

    // 17: ln(e) → "1"
    assert_eq!(strings[17], "1");
    // 18: lg(100) → "2"
    assert_eq!(strings[18], "2");
    // 19: log(8, 2) → "3"
    assert_eq!(strings[19], "3");
    // 20: exp(0) → "1"
    assert_eq!(strings[20], "1");

    // 21: round(-1/2) → "-1"
    assert_eq!(strings[21], "-1");
    // 22: floor(7/3) → "2"
    assert_eq!(strings[22], "2");
    // 23: ceil(-7/3) → "-2"
    assert_eq!(strings[23], "-2");

    // 24: math.pi → "π"
    assert_eq!(strings[24], PI);
    // 25: math.e → "e"
    assert_eq!(strings[25], "e");
    // 26: math.tau → "τ"
    assert_eq!(strings[26], TAU);

    // 27: math.hash hello, md5
    assert_eq!(strings[27], "5d41402abc4b2a76b9719d911017c592");
}
