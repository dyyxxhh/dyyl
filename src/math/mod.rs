//! dyyl CAS numeric layer — exact rationals, symbolic constants, roots.
//!
//! This module implements the fallback-custom CAS backend chosen in Task 1.
//! `mathcore` was rejected because its `Expr::Number` stores `f64`, which
//! cannot represent exact rationals, symbolic π/e/τ, symbolic sqrt, or trig
//! special-value simplification.

pub mod approx;
pub mod display;
pub mod hash;
pub mod ops;
pub mod trig;

use std::cmp::Ordering;
use std::fmt;

/// A dyyl exact / symbolic numeric value.
#[derive(Debug, Clone, PartialEq)]
pub enum CasNumber {
    /// Exact integer.
    Int(i64),
    /// Exact reduced rational `n/d` with `d > 0` and `gcd(|n|, d) = 1`.
    Rat(i64, i64),
    /// Square root of a contained CasNumber.
    Sqrt(Box<CasNumber>),
    /// Symbolic constant.
    Const(SymConstant),
    /// Sum of two expressions: `a + b`.
    Sum(Box<CasNumber>, Box<CasNumber>),
    /// Product of two expressions: `a * b`.
    Prod(Box<CasNumber>, Box<CasNumber>),
    /// Negation: `-a`.
    Neg(Box<CasNumber>),
}

/// Symbolic mathematical constants.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SymConstant {
    Pi,
    E,
    Tau,
}

impl CasNumber {
    /// The integer `0`.
    #[must_use]
    pub fn zero() -> Self {
        Self::Int(0)
    }

    /// The integer `1`.
    #[must_use]
    pub fn one() -> Self {
        Self::Int(1)
    }

    /// Return `true` if this value is exactly zero.
    #[must_use]
    pub fn is_zero(&self) -> bool {
        matches!(self, Self::Int(0))
    }

    /// Return `true` if this value is exactly one.
    #[must_use]
    pub fn is_one(&self) -> bool {
        matches!(self, Self::Int(1))
    }

    /// Return `true` if this value is exactly minus one.
    #[must_use]
    pub fn is_neg_one(&self) -> bool {
        matches!(self, Self::Int(-1))
    }

    /// Reduce a rational `n/d` to lowest terms.
    /// Returns `Int(n/d)` if `d` divides `n` evenly.
    #[must_use]
    pub fn reduce(n: i64, d: i64) -> Self {
        assert!(d != 0, "denominator must not be zero");
        if d < 0 {
            return Self::reduce(-n, -d);
        }
        if n == 0 {
            return Self::Int(0);
        }
        let g = gcd(n.unsigned_abs(), d as u64) as i64;
        let num = n / g;
        let den = d / g;
        if den == 1 {
            Self::Int(num)
        } else {
            Self::Rat(num, den)
        }
    }

    /// Convert `i64` to `CasNumber`.
    #[must_use]
    pub fn from_int(n: i64) -> Self {
        Self::Int(n)
    }

    /// Extract the integer value if this is `Int`, otherwise `None`.
    #[must_use]
    pub fn as_int(&self) -> Option<i64> {
        if let Self::Int(n) = self {
            Some(*n)
        } else {
            None
        }
    }

    /// Return `true` if this is an integer value.
    #[must_use]
    pub fn is_int(&self) -> bool {
        matches!(self, Self::Int(_))
    }

    /// Approximate this value as `f64`.
    #[must_use]
    pub fn to_f64(&self) -> f64 {
        match self {
            Self::Int(n) => *n as f64,
            Self::Rat(a, b) => *a as f64 / *b as f64,
            Self::Sqrt(inner) => inner.to_f64().sqrt(),
            Self::Const(c) => c.to_f64(),
            Self::Sum(a, b) => a.to_f64() + b.to_f64(),
            Self::Prod(a, b) => a.to_f64() * b.to_f64(),
            Self::Neg(a) => -a.to_f64(),
        }
    }
}

impl SymConstant {
    /// Approximate this constant as `f64`.
    #[must_use]
    pub fn to_f64(self) -> f64 {
        match self {
            Self::Pi => std::f64::consts::PI,
            Self::E => std::f64::consts::E,
            Self::Tau => std::f64::consts::TAU,
        }
    }
}

// ── Helpers ────────────────────────────────────────────────────────────

pub(crate) fn gcd(a: u64, b: u64) -> u64 {
    if b == 0 {
        a
    } else {
        gcd(b, a % b)
    }
}

/// Compare two `CasNumber` values ordinally (by f64 approximation, for
/// `Ord`/`Eq` on integer-only values).
#[must_use]
pub fn compare(a: &CasNumber, b: &CasNumber) -> Ordering {
    // For exact integers, compare directly.
    match (a, b) {
        (CasNumber::Int(x), CasNumber::Int(y)) => x.cmp(y),
        _ => match a.to_f64().partial_cmp(&b.to_f64()) {
            Some(ord) => ord,
            None => Ordering::Equal,
        },
    }
}

// The Display impl is in `display.rs`.

impl fmt::Display for SymConstant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pi => write!(f, "\u{03C0}"), // π
            Self::E => write!(f, "e"),
            Self::Tau => write!(f, "\u{03C4}"), // τ
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reduce_simple() {
        assert_eq!(CasNumber::reduce(6, 8), CasNumber::Rat(3, 4));
    }

    #[test]
    fn reduce_to_int() {
        assert_eq!(CasNumber::reduce(4, 2), CasNumber::Int(2));
    }

    #[test]
    fn reduce_zero() {
        assert_eq!(CasNumber::reduce(0, 5), CasNumber::Int(0));
    }

    #[test]
    fn reduce_neg_denominator() {
        assert_eq!(CasNumber::reduce(3, -4), CasNumber::Rat(-3, 4));
    }

    #[test]
    fn constant_display() {
        assert_eq!(SymConstant::Pi.to_string(), "\u{03C0}");
        assert_eq!(SymConstant::E.to_string(), "e");
        assert_eq!(SymConstant::Tau.to_string(), "\u{03C4}");
    }

    #[test]
    fn to_f64_int() {
        let val = (CasNumber::Int(42)).to_f64();
        assert!((val - 42.0).abs() < 1e-12);
    }

    #[test]
    fn to_f64_rat() {
        let val = (CasNumber::Rat(1, 3)).to_f64();
        assert!((val - 1.0 / 3.0).abs() < 1e-12);
    }

    #[test]
    fn to_f64_const() {
        let val = (CasNumber::Const(SymConstant::Pi)).to_f64();
        assert!((val - std::f64::consts::PI).abs() < 1e-12);
    }

    #[test]
    fn is_zero_one_neg_one() {
        assert!(CasNumber::zero().is_zero());
        assert!(CasNumber::one().is_one());
        assert!(CasNumber::Int(-1).is_neg_one());
        assert!(!CasNumber::Int(2).is_one());
    }
}
