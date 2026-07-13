use dyyl::runtime::plugin::manifest::RemoteManifest;

#[test]
fn parse_manifest_with_credentials() {
    let json = r#"{
        "name": "migpt",
        "version": "0.1.0",
        "abi_version": 2,
        "dyyl_min": "0.2.0",
        "platforms": [{"platform": "linux-x86_64", "url": "http://x", "sha256": "abc"}],
        "credentials": {
            "fields": [
                {"name": "token", "type": "string", "secret": true, "description": "GitHub PAT"},
                {"name": "repo", "type": "string", "secret": false, "description": "Default repo"}
            ]
        }
    }"#;
    let m: RemoteManifest = serde_json::from_str(json).expect("parse");
    assert_eq!(m.name, "migpt");
    let creds = m.credentials.expect("credentials present");
    assert_eq!(creds.fields.len(), 2);
    assert_eq!(creds.fields[0].name, "token");
    assert!(creds.fields[0].secret);
    assert_eq!(creds.fields[1].name, "repo");
    assert!(!creds.fields[1].secret);
}

#[test]
fn parse_manifest_without_credentials() {
    let json = r#"{
        "name": "simple",
        "version": "0.1.0",
        "abi_version": 2,
        "dyyl_min": "0.2.0",
        "platforms": [{"platform": "linux-x86_64", "url": "http://x", "sha256": "abc"}]
    }"#;
    let m: RemoteManifest = serde_json::from_str(json).expect("parse");
    assert!(m.credentials.is_none());
}
