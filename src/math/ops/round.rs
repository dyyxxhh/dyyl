//! Rounding operations for CasNumber.

use crate::math::CasNumber;

/// Round half away from zero.
pub fn round(a: &CasNumber) -> CasNumber {
    match a {
        CasNumber::Int(_) => a.clone(),
        CasNumber::Rat(n, d) => {
            if *d == 0 {
                return CasNumber::Int(0);
            }
            let abs_n = n.unsigned_abs() as i64;
            let half = *d / 2;
            let abs_r = (abs_n + half) / *d;
            if *n < 0 {
                CasNumber::Int(-abs_r)
            } else {
                CasNumber::Int(abs_r)
            }
        }
        _ => CasNumber::Int(a.to_f64().round() as i64),
    }
}

/// Floor (toward negative infinity).
pub fn floor(a: &CasNumber) -> CasNumber {
    match a {
        CasNumber::Int(_) => a.clone(),
        CasNumber::Rat(n, d) => {
            if *d == 0 {
                return CasNumber::Int(0);
            }
            if *n >= 0 {
                CasNumber::Int(*n / *d)
            } else {
                let d2 = n / d;
                if n % d == 0 {
                    CasNumber::Int(d2)
                } else {
                    CasNumber::Int(d2 - 1)
                }
            }
        }
        _ => CasNumber::Int(a.to_f64().floor() as i64),
    }
}

/// Ceil (toward positive infinity).
pub fn ceil(a: &CasNumber) -> CasNumber {
    match a {
        CasNumber::Int(_) => a.clone(),
        CasNumber::Rat(n, d) => {
            if *d == 0 {
                return CasNumber::Int(0);
            }
            if *n > 0 {
                CasNumber::Int(*n / *d + 1)
            } else {
                CasNumber::Int(*n / *d)
            }
        }
        _ => CasNumber::Int(a.to_f64().ceil() as i64),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_neg_half() {
        assert_eq!(round(&CasNumber::Rat(-1, 2)), CasNumber::Int(-1));
    }
    #[test]
    fn round_pos_half() {
        assert_eq!(round(&CasNumber::Rat(1, 2)), CasNumber::Int(1));
    }
    #[test]
    fn floor_pos() {
        assert_eq!(floor(&CasNumber::Rat(7, 3)), CasNumber::Int(2));
    }
    #[test]
    fn floor_neg() {
        assert_eq!(floor(&CasNumber::Rat(-7, 3)), CasNumber::Int(-3));
    }
    #[test]
    fn ceil_pos() {
        assert_eq!(ceil(&CasNumber::Rat(7, 3)), CasNumber::Int(3));
    }
    #[test]
    fn ceil_neg() {
        assert_eq!(ceil(&CasNumber::Rat(-7, 3)), CasNumber::Int(-2));
    }
}
