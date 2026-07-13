//! End-to-end plugin tests — load the example fixture directly (bypassing
//! fetch) and verify dispatch works.

use dyyl::runtime::plugin::loader::PluginLoader;
use std::path::PathBuf;

fn fixture_lib_path() -> PathBuf {
    // The fixture is built as a cdylib in tests/fixtures/example-plugin/.
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/example-plugin/target/release")
        .join(if cfg!(target_os = "macos") {
            "libexample.dylib"
        } else if cfg!(target_os = "windows") {
            "example.dll"
        } else {
            "libexample.so"
        })
}

#[test]
fn load_and_call_greet() {
    let lib_path = fixture_lib_path();
    if !lib_path.exists() {
        eprintln!("skipping: fixture not built at {}", lib_path.display());
        return;
    }
    let loader = PluginLoader::load(&lib_path, "example", None).expect("load failed");

    // List commands.
    let cmds = loader.list_commands().expect("list_commands failed");
    assert!(cmds.contains("greet"));
    assert!(cmds.contains("math.double"));

    // Call greet.
    let args = r#"[{"type":"str","value":"World"}]"#;
    let result = loader.handle_command("greet", args).expect("greet failed");
    assert!(result.contains("Hello, World!"));
}

#[test]
fn load_and_call_math_double() {
    let lib_path = fixture_lib_path();
    if !lib_path.exists() {
        eprintln!("skipping: fixture not built at {}", lib_path.display());
        return;
    }
    let loader = PluginLoader::load(&lib_path, "example", None).expect("load failed");

    let args = r#"[{"type":"num","value":"21"}]"#;
    let result = loader
        .handle_command("math.double", args)
        .expect("math.double failed");
    assert!(result.contains("42"));
}

#[test]
fn load_and_call_unknown_command() {
    let lib_path = fixture_lib_path();
    if !lib_path.exists() {
        eprintln!("skipping: fixture not built at {}", lib_path.display());
        return;
    }
    let loader = PluginLoader::load(&lib_path, "example", None).expect("load failed");

    let args = "[]";
    let result = loader.handle_command("nonexistent", args);
    assert!(result.is_err(), "unknown command should fail");
}
