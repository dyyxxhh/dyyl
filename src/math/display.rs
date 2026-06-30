//! dyyl CAS display formatting.
//!
//! The Display impl for `CasNumber` follows the dyyl spec output priority:
//! mixed fraction > improper fraction > irrational expression > decimal.
//! Unicode fraction characters are used for common fractions when possible.

use std::fmt;

use crate::math::CasNumber;

/// Unicode vulgar fraction numerator characters.
const VULGAR_NUM: &[char] = &[
    '\u{2070}', '\u{00B9}', '\u{00B2}', '\u{00B3}', '\u{2074}', '\u{2075}', '\u{2076}', '\u{2077}',
    '\u{2078}', '\u{2079}',
];

/// Unicode vulgar fraction denominator characters.
const VULGAR_DEN: &[char] = &[
    '\u{2080}', '\u{2081}', '\u{2082}', '\u{2083}', '\u{2084}', '\u{2085}', '\u{2086}', '\u{2087}',
    '\u{2088}', '\u{2089}',
];

/// Known Unicode vulgar fraction mappings for single-character display.
/// Maps `(numerator, denominator)` → Unicode char.
fn vulgar_fraction_char(n: u32, d: u32) -> Option<char> {
    match (n, d) {
        (1, 2) => Some('\u{00BD}'),  // ½
        (1, 3) => Some('\u{2153}'),  // ⅓
        (2, 3) => Some('\u{2154}'),  // ⅔
        (1, 4) => Some('\u{00BC}'),  // ¼
        (3, 4) => Some('\u{00BE}'),  // ¾
        (1, 5) => Some('\u{2155}'),  // ⅕
        (2, 5) => Some('\u{2156}'),  // ⅖
        (3, 5) => Some('\u{2157}'),  // ⅗
        (4, 5) => Some('\u{2158}'),  // ⅘
        (1, 6) => Some('\u{2159}'),  // ⅙
        (5, 6) => Some('\u{215A}'),  // ⅚
        (1, 7) => Some('\u{2150}'),  // ⅐
        (1, 8) => Some('\u{215B}'),  // ⅛
        (3, 8) => Some('\u{215C}'),  // ⅜
        (5, 8) => Some('\u{215D}'),  // ⅝
        (7, 8) => Some('\u{215E}'),  // ⅞
        (1, 9) => Some('\u{2151}'),  // ⅑
        (1, 10) => Some('\u{2152}'), // ⅒
        _ => None,
    }
}

/// Format a proper fraction `n/d` (where `0 < n < d`) as Unicode fraction.
fn format_proper_fraction(n: i64, d: i64) -> String {
    assert!(n > 0 && d > 0 && n < d);
    let nu = n.unsigned_abs() as u32;
    let du = d.unsigned_abs() as u32;
    if let Some(ch) = vulgar_fraction_char(nu, du) {
        return ch.to_string();
    }
    // Fall back to superscript/subscript digits.
    let num_str: String = nu
        .to_string()
        .chars()
        .map(|c| match c.to_digit(10) {
            Some(d) => VULGAR_NUM[d as usize],
            None => '?',
        })
        .collect();
    let den_str: String = du
        .to_string()
        .chars()
        .map(|c| match c.to_digit(10) {
            Some(d) => VULGAR_DEN[d as usize],
            None => '?',
        })
        .collect();
    format!("{num_str}\u{2044}{den_str}") // ⁄
}

/// Check if a fraction can use a direct Unicode fraction character.
fn is_vulgar_available(n: i64, d: i64) -> bool {
    if n <= 0 || d <= 0 || n >= d {
        return false;
    }
    vulgar_fraction_char(n.unsigned_abs() as u32, d.unsigned_abs() as u32).is_some()
}

impl fmt::Display for CasNumber {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Int(n) => write!(f, "{n}"),
            Self::Rat(n, d) => {
                let (num, den) = if *d < 0 { (-n, -d) } else { (*n, *d) };
                if den == 0 {
                    return write!(f, "err");
                }
                if num == 0 {
                    return write!(f, "0");
                }
                // Check for mixed number: |num| >= den
                let abs_num = num.unsigned_abs() as i64;
                if abs_num >= den {
                    let whole = num / den;
                    let rem = abs_num % den;
                    if rem == 0 {
                        return write!(f, "{whole}");
                    }
                    // Mixed number: whole + proper fraction
                    let proper = if whole < 0 {
                        // Negative mixed: -whole rem/den
                        format!("{}{}", whole, format_proper_fraction(rem, den))
                    } else {
                        format!("{}{}", whole, format_proper_fraction(rem, den))
                    };
                    return write!(f, "{proper}");
                }
                // Proper fraction
                if is_vulgar_available(num, den) {
                    write!(f, "{}", format_proper_fraction(num, den))
                } else {
                    write!(f, "{}", format_proper_fraction(num, den))
                }
            }
            Self::Sqrt(inner) => {
                let inner_str = inner.to_string();
                // If inner needs parentheses (compound expression), add them.
                let needs_paren = matches!(inner.as_ref(), Self::Sum(_, _) | Self::Prod(_, _));
                if needs_paren {
                    write!(f, "\u{221A}({inner_str})") // √(...)
                } else {
                    write!(f, "\u{221A}{inner_str}") // √x
                }
            }
            Self::Const(c) => write!(f, "{c}"),
            Self::Sum(a, b) => {
                let a_str = fmt_term(a, false);
                let b_str = fmt_term(b, true);
                write!(f, "{a_str} + {b_str}")
            }
            Self::Prod(a, b) => {
                // Handle special case: rat * sqrt → (√x)/y
                if let Self::Rat(n, d) = a.as_ref() {
                    if let Self::Sqrt(_) = b.as_ref() {
                        let sqrt_str = b.to_string();
                        if *n == 1 {
                            return write!(f, "({sqrt_str})/{d}");
                        }
                        if *d == 1 {
                            return write!(f, "{n}{sqrt_str}");
                        }
                        if *n == -1 {
                            return write!(f, "-({sqrt_str})/{d}");
                        }
                        return write!(f, "({n}{sqrt_str})/{d}");
                    }
                }
                // Default product formatting
                let a_str = fmt_term(a, false);
                let b_str = fmt_term(b, false);
                // Use × for multiplication display
                write!(f, "{a_str} \u{00D7} {b_str}")
            }
            Self::Neg(a) => {
                let inner = a.to_string();
                if matches!(a.as_ref(), Self::Int(_) | Self::Const(_) | Self::Sqrt(_)) {
                    write!(f, "-{inner}")
                } else {
                    write!(f, "-({inner})")
                }
            }
        }
    }
}

/// Format a term, optionally inserting a space before negative values.
fn fmt_term(val: &CasNumber, is_rhs: bool) -> String {
    let s = val.to_string();
    if is_rhs && val.is_neg() {
        // Already has "-" prefix from Display
        s
    } else {
        s
    }
}

// Helper to check if value is negative (for display context).
// We cannot make it pub on CasNumber easily without circular dep, so local.
trait IsNeg {
    fn is_neg(&self) -> bool;
}

impl IsNeg for CasNumber {
    fn is_neg(&self) -> bool {
        match self {
            CasNumber::Int(n) => *n < 0,
            CasNumber::Rat(n, _) => *n < 0,
            CasNumber::Neg(_) => true,
            CasNumber::Sqrt(_) => false,
            CasNumber::Const(_) => false,
            CasNumber::Sum(a, b) => a.is_neg() && b.is_neg(),
            CasNumber::Prod(a, b) => a.is_neg() != b.is_neg(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::SymConstant;

    #[test]
    fn display_int() {
        assert_eq!(CasNumber::Int(42).to_string(), "42");
        assert_eq!(CasNumber::Int(-5).to_string(), "-5");
        assert_eq!(CasNumber::Int(0).to_string(), "0");
    }

    #[test]
    fn display_proper_fraction_vulgar() {
        assert_eq!(CasNumber::reduce(1, 2).to_string(), "\u{00BD}"); // ½
        assert_eq!(CasNumber::reduce(1, 3).to_string(), "\u{2153}"); // ⅓
        assert_eq!(CasNumber::reduce(2, 3).to_string(), "\u{2154}"); // ⅔
        assert_eq!(CasNumber::reduce(1, 4).to_string(), "\u{00BC}"); // ¼
    }

    #[test]
    fn display_proper_fraction_fallback() {
        // 1/7 has a Unicode char
        assert_eq!(CasNumber::reduce(1, 7).to_string(), "\u{2150}");
        // 2/7 does not → fallback to "²/₇"
        let s = CasNumber::reduce(2, 7).to_string();
        // Should use superscript 2 and subscript 7
        assert!(s.contains('\u{00B2}')); // ²
        assert!(s.contains('\u{2087}')); // ₇
    }

    #[test]
    fn display_mixed_number() {
        assert_eq!(CasNumber::reduce(5, 3).to_string(), "1\u{2154}"); // 1⅔
        assert_eq!(CasNumber::reduce(7, 3).to_string(), "2\u{2153}"); // 2⅓
    }

    #[test]
    fn display_sqrt() {
        assert_eq!(
            CasNumber::Sqrt(Box::new(CasNumber::Int(2))).to_string(),
            "\u{221A}2" // √2
        );
    }

    #[test]
    fn display_sqrt_over_denominator() {
        // (√2)/2
        let half = CasNumber::Rat(1, 2);
        let sqrt2 = CasNumber::Sqrt(Box::new(CasNumber::Int(2)));
        let prod = CasNumber::Prod(Box::new(half), Box::new(sqrt2));
        assert_eq!(prod.to_string(), "(\u{221A}2)/2");
    }

    #[test]
    fn display_constants() {
        assert_eq!(CasNumber::Const(SymConstant::Pi).to_string(), "\u{03C0}");
        assert_eq!(CasNumber::Const(SymConstant::E).to_string(), "e");
        assert_eq!(CasNumber::Const(SymConstant::Tau).to_string(), "\u{03C4}");
    }

    #[test]
    fn display_sum() {
        let sum = CasNumber::Sum(
            Box::new(CasNumber::Rat(1, 3)),
            Box::new(CasNumber::Const(SymConstant::Pi)),
        );
        let s = sum.to_string();
        assert!(s.contains('\u{2153}')); // ⅓
        assert!(s.contains('\u{03C0}')); // π
        assert!(s.contains('+'));
    }

    #[test]
    fn display_neg_int() {
        assert_eq!(
            CasNumber::Neg(Box::new(CasNumber::Int(5))).to_string(),
            "-5"
        );
    }

    #[test]
    fn display_neg_sum() {
        let neg = CasNumber::Neg(Box::new(CasNumber::Sum(
            Box::new(CasNumber::Int(1)),
            Box::new(CasNumber::Int(2)),
        )));
        assert_eq!(neg.to_string(), "-(1 + 2)");
    }

    #[test]
    fn display_mixed_negative() {
        // -5/3 should display as -1⅔
        let r = CasNumber::reduce(-5, 3);
        assert_eq!(r.to_string(), "-1\u{2154}");
    }

    #[test]
    fn display_int_from_rat() {
        assert_eq!(CasNumber::reduce(6, 3).to_string(), "2");
        assert_eq!(CasNumber::reduce(0, 5).to_string(), "0");
    }
}
