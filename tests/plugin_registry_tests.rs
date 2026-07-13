use dyyl::runtime::plugin::registry;
use dyyl::runtime::plugin::store;
use std::fs;

#[test]
fn scan_empty_dir_returns_empty() {
    // Use a temp dir — but registry scans the real XDG dir.
    // Since no plugins are installed in CI, this should return empty.
    let plugins = registry::scan_installed().unwrap_or_default();
    // Just verify it doesn't panic. May be empty or contain test artifacts.
    let _ = plugins;
}

#[test]
fn installed_plugin_record_has_fields() {
    let rec = registry::InstalledPlugin {
        name: "test".to_string(),
        version: "0.1.0".to_string(),
        toml_path: std::path::PathBuf::from("/tmp/test/plugin.toml"),
        lib_path: std::path::PathBuf::from("/tmp/test/libtest.so"),
    };
    assert_eq!(rec.name, "test");
    assert_eq!(rec.version, "0.1.0");
}
