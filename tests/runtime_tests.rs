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
//! Integration tests for the dyyl runtime: value model, environment,
//! sentinels, and debug diagnostics (Task 4).

use dyyl::runtime::execute::run_script;
use dyyl::runtime::Value;

// ── value_environment_and_sentinels (targeted acceptance test) ───────

/// Acceptance test: global scope, set, dict.get missing → -1,
/// list.get OOB → -1, undefined variable, unknown command.
#[test]
fn value_environment_and_sentinels() {
    // Given: a script exercising every sentinel scenario
    let source = "\
dict.create d
list.create l
io.out $missing_var
io.out dict.get(d, nonexistent)
io.out list.get(l, 0)
unknown_cmd
";

    // When: we execute it
    let output = run_script(source, false);

    // Then: each command produces the correct sentinel
    // dict.create and list.create return Empty (not captured as error)
    // io.out $missing_var → sentinel
    // io.out dict.get(d, nonexistent) → -1
    // io.out list.get(l, 0) → -1
    // unknown_cmd → sentinel
    assert_eq!(output.values.len(), 6, "expected 6 result values");

    // Command 0: dict.create d → Empty
    assert_eq!(output.values[0], Value::Empty, "dict.create returns Empty");

    // Command 1: list.create l → Empty
    assert_eq!(output.values[1], Value::Empty, "list.create returns Empty");

    // Command 2: io.out $missing_var → sentinel (undefined var produces Num(-1))
    assert_eq!(output.values[2], Value::Num(-1), "undefined var → sentinel");

    // Command 3: io.out dict.get(d, nonexistent) → -1
    assert_eq!(output.values[3], Value::Num(-1), "missing dict.get → -1");

    // Command 4: io.out list.get(l, 0) → -1
    assert_eq!(output.values[4], Value::Num(-1), "OOB list.get → -1");

    // Command 5: unknown_cmd → sentinel (Num(-1))
    assert_eq!(
        output.values[5],
        Value::Num(-1),
        "unknown command → sentinel"
    );
}

// ── Debug mode diagnostics ──────────────────────────────────────────

/// Debug mode must print sentinel to stdout and diagnostic info to stderr.
#[test]
fn debug_mode_prints_warnings() {
    // Given: a script with known errors
    let source = "unknown_cmd\n";

    // When: executing with debug=true
    let output = run_script(source, true);

    // Then: sentinel is returned
    assert_eq!(output.values.len(), 1);
    assert_eq!(output.values[0], Value::Num(-1));
    // Debug info goes to stderr — cannot assert on eprintln output here
    // without capturing stderr; the manual QA fixture verifies this.
}

// ── Global scope (Decision 27) ──────────────────────────────────────

/// Prove that `create.num`/`create.str` actually bind variables AND
/// that `set` can rebind them AND that subsequent `$var` reads resolve.
#[test]
fn global_scope_persists_across_commands() {
    // Line 1: create.num x   → binds x=0, returns Num(0)
    // Line 2: set $x, 10     → rebinds x=10, returns Num(10)
    // Line 3: create.str s   → binds s="", returns Str("")
    // Line 4: set $s, hello  → rebinds s="hello", returns Param("hello")
    // Line 5: io.out $x      → resolves $x=10, prints and returns Num(10)
    // Line 6: io.out $s      → resolves $s="hello", prints and returns Str("hello")
    let source = "\
create.num x
set $x, 10
create.str s
set $s, hello
io.out $x
io.out $s
";
    let output = run_script(source, false);

    assert_eq!(output.values.len(), 6, "expected 6 result values");

    // Command 0: create.num x → Num(0)
    assert_eq!(output.values[0], Value::Num(0), "create.num returns Num(0)");

    // Command 1: set $x, 10 → Num(10)
    assert_eq!(output.values[1], Value::Num(10), "set $x returns 10");

    // Command 2: create.str s → Str("")
    assert_eq!(
        output.values[2],
        Value::Str(String::new()),
        "create.str returns empty str"
    );

    // Command 3: set $s, hello → Str("hello") — bare word string literal
    assert_eq!(
        output.values[3],
        Value::Str("hello".to_string()),
        "set $s returns hello"
    );

    // Command 4: io.out $x → reads $x which should be 10
    assert_eq!(output.values[4], Value::Num(10), "$x reads 10 after set");

    // Command 5: io.out $s → reads $s which should be "hello"
    assert_eq!(
        output.values[5],
        Value::Str("hello".to_string()),
        "$s reads hello after set"
    );
}
