use dyyl::runtime::plugin::fetch;

#[test]
fn sha256_of_known_bytes() {
    // SHA256 of empty string
    let hash = fetch::sha256_bytes(b"");
    assert_eq!(
        hash,
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
}

#[test]
fn sha256_of_hello() {
    let hash = fetch::sha256_bytes(b"hello");
    assert_eq!(
        hash,
        "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
    );
}

#[test]
fn verify_checksum_matches() {
    let data = b"hello";
    let expected = "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824";
    assert!(fetch::verify_checksum(data, expected));
}

#[test]
fn verify_checksum_mismatches() {
    let data = b"hello";
    let wrong = "0000000000000000000000000000000000000000000000000000000000000000";
    assert!(!fetch::verify_checksum(data, wrong));
}
