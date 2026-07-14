//! Golden fixture / end-to-end CLI snapshot tests for the dyyl interpreter.
//!
//! Each test runs the compiled binary against a fixture file and asserts
//! that stdout (and optionally stderr) match the captured golden output.
//!
//! Fixtures live in `tests/fixtures/` and exercise the full CLI pipeline
//! (arg parsing → file read → lex → parse → execute → display).

use std::process::Command;

/// Path to the compiled dyyl binary, resolved by `cargo test`.
fn dyyl_bin() -> String {
    env!("CARGO_BIN_EXE_dyyl").to_string()
}

/// Path to the repository root (where `tests/fixtures/` lives).
fn repo_root() -> String {
    // In integration tests the working directory is the crate root.
    std::env::current_dir()
        .expect("cargo sets cwd to crate root")
        .to_string_lossy()
        .into_owned()
}

/// Run the dyyl binary with optional `--debug` flag and a fixture path.
/// Returns (exit_code, stdout, stderr).
fn run_dyyl(fixture: &str, debug: bool) -> (i32, String, String) {
    let bin = dyyl_bin();
    let root = repo_root();
    let path = format!("{root}/tests/fixtures/{fixture}");

    let mut cmd = Command::new(&bin);
    if debug {
        cmd.arg("--debug");
    }
    cmd.arg(&path);

    let output = cmd.output().expect("failed to execute dyyl binary");

    let code = output.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();

    (code, stdout, stderr)
}

// ── basic.anything ───────────────────────────────────────────────────

/// Proves the CLI can execute an arbitrary-extension script and produce
/// the expected numeric output.
#[test]
fn golden_basic_anything() {
    let (code, stdout, _stderr) = run_dyyl("basic.anything", false);
    assert_eq!(code, 0, "exit code must be 0");
    assert_eq!(stdout, "42\n", "basic.anything must print 42");
}

// ── arithmetic.dyyl ──────────────────────────────────────────────────

/// Exercises basic arithmetic, CAS display (exact rationals, roots,
/// constants, trig), and char arithmetic.
#[test]
fn golden_arithmetic() {
    let (code, stdout, _stderr) = run_dyyl("arithmetic.dyyl", false);
    assert_eq!(code, 0, "exit code must be 0");

    let expected =
        "3\n2\n6\n3\n8\n2\n0\n1\n\u{2153}\n1\u{2154}\n\u{221A}2\n\u{03C0}\n3.14159265358979\nb\n";
    assert_eq!(stdout, expected, "arithmetic output mismatch");
}

// ── nested-control.dyyl ──────────────────────────────────────────────

/// Nested if/while/for with execution counts.
#[test]
fn golden_nested_control() {
    let (code, stdout, _stderr) = run_dyyl("nested-control.dyyl", false);
    assert_eq!(code, 0, "exit code must be 0");

    let expected = "\
nested_if
while_iter
while_iter
while_iter
for_iter
for_iter
nested_for
nested_for
";
    assert_eq!(stdout, expected, "nested-control output mismatch");
}

// ── containers.dyyl ──────────────────────────────────────────────────

/// Dict and list operations: create, set, get, keys, len, has,
/// append, sort.
#[test]
fn golden_containers() {
    let (code, stdout, _stderr) = run_dyyl("containers.dyyl", false);
    assert_eq!(code, 0, "exit code must be 0");

    let expected = "\
alice
30
2
1
0
3
30
10
20
30
";
    assert_eq!(stdout, expected, "containers output mismatch");
}

// ── io-string.dyyl ───────────────────────────────────────────────────

/// IO output and string operations.
#[test]
fn golden_io_string() {
    let (code, stdout, _stderr) = run_dyyl("io-string.dyyl", false);
    assert_eq!(code, 0, "exit code must be 0");

    let expected = "\
hello world
42
5
HELLO
hello
cba
1
0
";
    assert_eq!(stdout, expected, "io-string output mismatch");
}

// ── mcm-unknown.dyyl ────────────────────────────────────────────────

/// Proves `mcm.*` returns the unknown-command sentinel (-1) with a
/// debug warning, NOT a handler response.
#[test]
fn golden_mcm_unknown() {
    let (code, stdout, stderr) = run_dyyl("mcm-unknown.dyyl", true);
    assert_eq!(code, 0, "exit code must be 0");
    assert_eq!(stdout, "-1\n", "mcm unknown must print sentinel -1");
    assert!(
        stderr.contains("unknown command"),
        "stderr must contain 'unknown command' warning, got: {stderr}"
    );
    assert!(
        stderr.contains("mcm.game.install"),
        "stderr must mention the offending command, got: {stderr}"
    );
}

// ── cli-args.dyyl ────────────────────────────────────────────────────

/// Runs cli-args.dyyl with two args and verifies the script reads them
/// via cli.* commands.
#[test]
fn golden_cli_args() {
    let bin = dyyl_bin();
    let root = repo_root();
    let path = format!("{root}/tests/fixtures/cli-args.dyyl");

    let output = Command::new(&bin)
        .arg(&path)
        .arg("--help")
        .arg("foo")
        .output()
        .expect("failed to execute dyyl binary");

    let code = output.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(code, 0, "exit code must be 0, stderr was: {stderr}");
    // 脚本应打印 count=2 和 name=cli-args.dyyl
    assert!(stdout.contains("count: 2"), "stdout was: {stdout}");
    assert!(stdout.contains("name: cli-args.dyyl"), "stdout was: {stdout}");
}
