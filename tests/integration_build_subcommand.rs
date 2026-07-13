//! Integration test: `dyyl build <file>` subcommand is recognized and
//! dispatched to the prepass build_only path.
//!
//! Before the build subcommand was wired up, `dyyl build nonexistent.dyyl`
//! fell through to normal script execution: "build" was treated as a
//! filename and "nonexistent.dyyl" overwrote it, yielding a read error
//! with exit code 1. After wiring, the build arm dispatches to
//! `prepass::build_only`, which fails to read the missing file and
//! returns exit code 2.

use std::process::Command;

#[test]
fn dyyl_build_subcommand_is_recognized() {
    let output = Command::new("cargo")
        .args(["run", "--", "build", "nonexistent.dyyl"])
        .output()
        .expect("spawn");
    let stderr = String::from_utf8_lossy(&output.stderr);
    // 不应出现 "unknown option"。
    assert!(!stderr.contains("unknown option"), "stderr was: {stderr}");
    // build 子命令应分发到 prepass；文件不存在 → 预扫描失败 → 退出码 2。
    // （修复前：build 被当作文件名，read 失败 → 退出码 1。）
    assert_eq!(output.status.code(), Some(2), "stderr was: {stderr}");
}
