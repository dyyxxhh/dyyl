//! `math.hash` implementation for dyyl.
//!
//! Supports md5, sha1, and sha256 hashing for numeric and string values.
//! Numeric values are hashed as their dyyl display string.

use md5::Md5;
use sha1::Sha1;
use sha2::digest::Digest;
use sha2::Sha256;

use crate::math::CasNumber;

/// Supported hash algorithms.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HashAlgo {
    Md5,
    Sha1,
    Sha256,
}

/// Parse a hash algorithm name (case-insensitive).
#[must_use]
pub fn parse_algo(name: &str) -> Option<HashAlgo> {
    match name.to_lowercase().as_str() {
        "md5" => Some(HashAlgo::Md5),
        "sha1" => Some(HashAlgo::Sha1),
        "sha256" => Some(HashAlgo::Sha256),
        _ => None,
    }
}

/// Compute a hash digest as a hex string.
///
/// - For `CasNumber` values, the input is the dyyl display string.
/// - For string values, the input is the string itself.
#[must_use]
pub fn hash_value(algo: HashAlgo, input: &str) -> String {
    match algo {
        HashAlgo::Md5 => {
            let mut hasher = Md5::new();
            hasher.update(input.as_bytes());
            hex::encode(hasher.finalize())
        }
        HashAlgo::Sha1 => {
            let mut hasher = Sha1::new();
            hasher.update(input.as_bytes());
            hex::encode(hasher.finalize())
        }
        HashAlgo::Sha256 => {
            let mut hasher = Sha256::new();
            hasher.update(input.as_bytes());
            hex::encode(hasher.finalize())
        }
    }
}

/// Compute a hash of a `CasNumber` (digesting its dyyl display form).
#[must_use]
pub fn hash_cas_number(algo: HashAlgo, val: &CasNumber) -> String {
    let display = val.to_string();
    hash_value(algo, &display)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_algo_cases() {
        assert_eq!(parse_algo("md5"), Some(HashAlgo::Md5));
        assert_eq!(parse_algo("MD5"), Some(HashAlgo::Md5));
        assert_eq!(parse_algo("sha1"), Some(HashAlgo::Sha1));
        assert_eq!(parse_algo("SHA1"), Some(HashAlgo::Sha1));
        assert_eq!(parse_algo("sha256"), Some(HashAlgo::Sha256));
        assert_eq!(parse_algo("SHA256"), Some(HashAlgo::Sha256));
        assert_eq!(parse_algo("sha512"), None);
    }

    #[test]
    fn hash_md5_string() {
        let result = hash_value(HashAlgo::Md5, "hello");
        assert_eq!(result, "5d41402abc4b2a76b9719d911017c592");
    }

    #[test]
    fn hash_sha1_string() {
        let result = hash_value(HashAlgo::Sha1, "hello");
        assert_eq!(result, "aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d");
    }

    #[test]
    fn hash_sha256_string() {
        let result = hash_value(HashAlgo::Sha256, "hello");
        assert_eq!(
            result,
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
    }

    #[test]
    fn hash_md5_int() {
        let result = hash_cas_number(HashAlgo::Md5, &CasNumber::Int(42));
        // "42" hashed with md5
        assert_eq!(result, "a1d0c6e83f027327d8461063f4ac58a6");
    }

    #[test]
    fn hash_md5_string_value() {
        let result = hash_value(HashAlgo::Md5, "hello world");
        assert_eq!(result, "5eb63bbbe01eeed093cb22bb8f5acdc3");
    }
}
