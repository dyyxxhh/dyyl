//! Fetch plugin manifests and libraries from l.dyyapp.com.
//!
//! Uses the same `ureq` agent as `net.rs` for HTTPS. SHA256 verification
//! ensures downloaded libraries match the manifest's checksum.

use std::io::Read;

use sha2::{Digest, Sha256};

use crate::runtime::plugin::manifest::RemoteManifest;

/// Base URL for plugin distribution.
const PLUGIN_BASE_URL: &str = "https://l.dyyapp.com/plugins";

/// Fetch a plugin's remote manifest by name.
///
/// GETs `{base}/{name}/manifest.json`. Returns the parsed manifest or an error.
pub fn fetch_manifest(name: &str) -> Result<RemoteManifest, FetchError> {
    let url = format!("{PLUGIN_BASE_URL}/{name}/manifest.json");
    let agent = ureq::AgentBuilder::new().build();
    let mut body = String::new();
    agent
        .get(&url)
        .call()
        .map_err(|e| FetchError::Http(url.clone(), e.to_string()))?
        .into_reader()
        .read_to_string(&mut body)
        .map_err(|e| FetchError::Read(url.clone(), e.to_string()))?;
    serde_json::from_str(&body).map_err(|e| FetchError::Parse(url, e.to_string()))
}

/// Download a library from `url` and verify its SHA256 matches `expected_sha256`.
///
/// Returns the raw bytes on success.
pub fn download_and_verify(url: &str, expected_sha256: &str) -> Result<Vec<u8>, FetchError> {
    let agent = ureq::AgentBuilder::new().build();
    let mut body = Vec::new();
    agent
        .get(url)
        .call()
        .map_err(|e| FetchError::Http(url.to_string(), e.to_string()))?
        .into_reader()
        .read_to_end(&mut body)
        .map_err(|e| FetchError::Read(url.to_string(), e.to_string()))?;
    if !verify_checksum(&body, expected_sha256) {
        return Err(FetchError::ChecksumMismatch(url.to_string()));
    }
    Ok(body)
}

/// Compute the SHA256 hex digest of a byte slice.
#[must_use]
pub fn sha256_bytes(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let result = hasher.finalize();
    hex::encode(result)
}

/// Verify that a byte slice's SHA256 matches `expected` (hex string).
#[must_use]
pub fn verify_checksum(data: &[u8], expected: &str) -> bool {
    let actual = sha256_bytes(data);
    actual.eq_ignore_ascii_case(expected)
}

/// Error from fetch operations.
#[derive(Debug)]
pub enum FetchError {
    Http(String, String),
    Read(String, String),
    Parse(String, String),
    ChecksumMismatch(String),
}

impl std::fmt::Display for FetchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Http(url, e) => write!(f, "HTTP error fetching {url}: {e}"),
            Self::Read(url, e) => write!(f, "read error from {url}: {e}"),
            Self::Parse(url, e) => write!(f, "parse error for {url}: {e}"),
            Self::ChecksumMismatch(url) => write!(f, "SHA256 mismatch for {url}"),
        }
    }
}

impl std::error::Error for FetchError {}
