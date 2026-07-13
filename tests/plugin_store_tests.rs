use dyyl::runtime::plugin::store;
use std::path::PathBuf;

#[test]
fn plugin_dir_is_under_xdg_data() {
    let dir = store::plugin_dir();
    // Should end with dyyl/plugins
    assert!(
        dir.ends_with("dyyl/plugins") || dir.ends_with("dyyl\\plugins"),
        "plugin_dir was: {}",
        dir.display()
    );
}

#[test]
fn plugin_version_dir_includes_name_and_version() {
    let dir = store::plugin_version_dir("migpt", "0.1.0");
    assert!(dir.to_string_lossy().contains("migpt"));
    assert!(dir.to_string_lossy().contains("0.1.0"));
}

#[test]
fn lib_path_ends_with_platform_suffix() {
    let path = store::lib_path("migpt", "0.1.0");
    let s = path.to_string_lossy();
    // On linux .so, macos .dylib, windows .dll
    assert!(
        s.ends_with(".so") || s.ends_with(".dylib") || s.ends_with(".dll"),
        "lib_path was: {s}"
    );
}

#[test]
fn plugin_toml_path_in_same_dir_as_lib() {
    let toml_path = store::plugin_toml_path("migpt", "0.1.0");
    assert!(toml_path.to_string_lossy().contains("migpt"));
    assert!(toml_path.to_string_lossy().ends_with("plugin.toml"));
}
