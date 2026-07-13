use dyyl::runtime::plugin::manifest::{RemoteManifest, LocalPluginToml};

#[test]
fn parse_remote_manifest() {
    let json = r#"{
        "name": "migpt",
        "version": "0.1.0",
        "abi_version": 1,
        "dyyl_min": "0.2.0",
        "panic_mode": "abort",
        "commands": [
            {"name": "greet", "arity": 1, "brief": "Send a greeting"},
            {"name": "user.login", "arity": 2, "brief": "Login"}
        ],
        "platforms": [
            {"platform": "linux-x86_64", "url": "https://l.dyyapp.com/p/migpt/0.1.0/linux-x86_64/libmigpt.so", "sha256": "abc123"}
        ]
    }"#;
    let m: RemoteManifest = serde_json::from_str(json).unwrap();
    assert_eq!(m.name, "migpt");
    assert_eq!(m.version, "0.1.0");
    assert_eq!(m.abi_version, 1);
    assert_eq!(m.dyyl_min, "0.2.0");
    assert_eq!(m.panic_mode, "abort");
    assert_eq!(m.commands.len(), 2);
    assert_eq!(m.commands[0].name, "greet");
    assert_eq!(m.commands[0].arity, 1);
    assert_eq!(m.commands[1].name, "user.login");
    assert_eq!(m.platforms.len(), 1);
    assert_eq!(m.platforms[0].platform, "linux-x86_64");
    assert_eq!(m.platforms[0].sha256, "abc123");
}

#[test]
fn parse_local_plugin_toml() {
    let toml = r#"
name = "migpt"
version = "0.1.0"
abi_version = 1
dyyl_min = "0.2.0"
panic_mode = "abort"

[[commands]]
name = "greet"
arity = 1
brief = "Send a greeting"

[installed]
source_url = "https://l.dyyapp.com/p/migpt.so"
sha256 = "abc123"
installed_at = "2026-07-13T10:30:00Z"
dyyl_version = "0.2.0"
"#;
    let t: LocalPluginToml = toml::from_str(toml).unwrap();
    assert_eq!(t.name, "migpt");
    assert_eq!(t.version, "0.1.0");
    assert_eq!(t.abi_version, 1);
    assert_eq!(t.commands.len(), 1);
    assert_eq!(t.installed.sha256, "abc123");
    assert_eq!(t.installed.dyyl_version, "0.2.0");
}
