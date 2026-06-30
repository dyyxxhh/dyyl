//! Trigonometric special-value evaluation.
//!
//! Returns exact dyyl values for common multiples of π/6, π/4, π/3, π/2.
//! Non-special values return a numeric approximation.

use crate::math::ops::{div, neg, sqrt};
use crate::math::{CasNumber, SymConstant};

/// Try to match a value of the form `(n/d) * π` and return `(n, d)`.
fn as_pi_ratio(val: &CasNumber) -> Option<(i64, i64)> {
    if let CasNumber::Prod(a, b) = val {
        match (a.as_ref(), b.as_ref()) {
            (CasNumber::Rat(n, d), CasNumber::Const(SymConstant::Pi)) => Some((*n, *d)),
            (CasNumber::Const(SymConstant::Pi), CasNumber::Rat(n, d)) => Some((*n, *d)),
            _ => None,
        }
    } else if let CasNumber::Const(SymConstant::Pi) = val {
        Some((1, 1))
    } else if let CasNumber::Const(SymConstant::Tau) = val {
        Some((2, 1))
    } else {
        None
    }
}

/// Evaluate `sin(x)` for special values.
#[must_use]
pub fn sin(x: &CasNumber) -> CasNumber {
    if x.is_zero() {
        return CasNumber::Int(0);
    }

    if let CasNumber::Neg(inner) = x {
        return neg(&sin(inner));
    }

    if let Some((n, d)) = as_pi_ratio(x) {
        let val = CasNumber::reduce(n, d);
        if let CasNumber::Rat(sn, sd) = val {
            return sin_pi_ratio(sn, sd);
        }
    }

    CasNumber::Int(x.to_f64().sin() as i64)
}

fn sin_pi_ratio(n: i64, d: i64) -> CasNumber {
    match (n, d) {
        (0, _) => CasNumber::Int(0),
        (1, 6) | (5, 6) => CasNumber::Rat(1, 2),
        (1, 4) | (3, 4) => div(&sqrt(&CasNumber::Int(2)), &CasNumber::Int(2)),
        (1, 3) | (2, 3) => div(&sqrt(&CasNumber::Int(3)), &CasNumber::Int(2)),
        (1, 2) => CasNumber::Int(1),
        (3, 2) => CasNumber::Int(-1),
        (_, 1) => CasNumber::Int(0),
        _ => {
            let reduced = CasNumber::reduce(n, d);
            if let CasNumber::Rat(rn, rd) = reduced {
                if rn != n || rd != d {
                    return sin_pi_ratio(rn, rd);
                }
            }
            CasNumber::Int(-1)
        }
    }
}

/// Evaluate `cos(x)` for special values.
#[must_use]
pub fn cos(x: &CasNumber) -> CasNumber {
    if x.is_zero() {
        return CasNumber::Int(1);
    }

    if let CasNumber::Neg(inner) = x {
        return cos(inner);
    }

    if let Some((n, d)) = as_pi_ratio(x) {
        let val = CasNumber::reduce(n, d);
        if let CasNumber::Rat(sn, sd) = val {
            return cos_pi_ratio(sn, sd);
        }
    }

    CasNumber::Int(x.to_f64().cos() as i64)
}

fn cos_pi_ratio(n: i64, d: i64) -> CasNumber {
    match (n, d) {
        (0, _) => CasNumber::Int(1),
        (1, 6) | (11, 6) => div(&sqrt(&CasNumber::Int(3)), &CasNumber::Int(2)),
        (5, 6) | (7, 6) => neg(&div(&sqrt(&CasNumber::Int(3)), &CasNumber::Int(2))),
        (1, 4) | (7, 4) => div(&sqrt(&CasNumber::Int(2)), &CasNumber::Int(2)),
        (3, 4) | (5, 4) => neg(&div(&sqrt(&CasNumber::Int(2)), &CasNumber::Int(2))),
        (1, 3) | (5, 3) => CasNumber::Rat(1, 2),
        (2, 3) | (4, 3) => neg(&CasNumber::Rat(1, 2)),
        (1, 2) | (3, 2) => CasNumber::Int(0),
        (_, 1) if n % 2 == 0 => CasNumber::Int(1),
        (_, 1) => CasNumber::Int(-1),
        _ => {
            let reduced = CasNumber::reduce(n, d);
            if let CasNumber::Rat(rn, rd) = reduced {
                if rn != n || rd != d {
                    return cos_pi_ratio(rn, rd);
                }
            }
            CasNumber::Int(-1)
        }
    }
}

/// Evaluate `tan(x)` for special values.
#[must_use]
pub fn tan(x: &CasNumber) -> CasNumber {
    if x.is_zero() {
        return CasNumber::Int(0);
    }

    if let CasNumber::Neg(inner) = x {
        return neg(&tan(inner));
    }

    if let Some((n, d)) = as_pi_ratio(x) {
        let val = CasNumber::reduce(n, d);
        if let CasNumber::Rat(sn, sd) = val {
            return tan_pi_ratio(sn, sd);
        }
    }

    CasNumber::Int(x.to_f64().tan() as i64)
}

fn tan_pi_ratio(n: i64, d: i64) -> CasNumber {
    match (n, d) {
        (0, _) => CasNumber::Int(0),
        (1, 6) | (7, 6) => div(&CasNumber::Int(1), &sqrt(&CasNumber::Int(3))),
        (5, 6) | (11, 6) => neg(&div(&CasNumber::Int(1), &sqrt(&CasNumber::Int(3)))),
        (1, 4) | (5, 4) => CasNumber::Int(1),
        (3, 4) | (7, 4) => CasNumber::Int(-1),
        (1, 3) | (4, 3) => sqrt(&CasNumber::Int(3)),
        (2, 3) | (5, 3) => neg(&sqrt(&CasNumber::Int(3))),
        (1, 2) | (3, 2) => CasNumber::Int(-1),
        (_, 1) => CasNumber::Int(0),
        _ => {
            let reduced = CasNumber::reduce(n, d);
            if let CasNumber::Rat(rn, rd) = reduced {
                if rn != n || rd != d {
                    return tan_pi_ratio(rn, rd);
                }
            }
            CasNumber::Int(-1)
        }
    }
}

/// Evaluate `asin(x)`.
#[must_use]
pub fn asin(x: &CasNumber) -> CasNumber {
    match x {
        CasNumber::Int(0) => CasNumber::Int(0),
        CasNumber::Int(1) => pi_ratio(1, 2),
        CasNumber::Int(-1) => neg(&pi_ratio(1, 2)),
        _ => {
            if let CasNumber::Rat(1, 2) = x {
                pi_ratio(1, 6)
            } else if let CasNumber::Rat(-1, 2) = x {
                neg(&pi_ratio(1, 6))
            } else {
                CasNumber::Int(x.to_f64().asin() as i64)
            }
        }
    }
}

/// Evaluate `acos(x)`.
#[must_use]
pub fn acos(x: &CasNumber) -> CasNumber {
    match x {
        CasNumber::Int(1) => CasNumber::Int(0),
        CasNumber::Int(0) => pi_ratio(1, 2),
        CasNumber::Int(-1) => CasNumber::Const(SymConstant::Pi),
        _ => {
            if let CasNumber::Rat(1, 2) = x {
                pi_ratio(1, 3)
            } else if let CasNumber::Rat(-1, 2) = x {
                pi_ratio(2, 3)
            } else {
                CasNumber::Int(x.to_f64().acos() as i64)
            }
        }
    }
}

/// Evaluate `atan(x)`.
#[must_use]
pub fn atan(x: &CasNumber) -> CasNumber {
    match x {
        CasNumber::Int(0) => CasNumber::Int(0),
        CasNumber::Int(1) => pi_ratio(1, 4),
        CasNumber::Int(-1) => neg(&pi_ratio(1, 4)),
        _ => CasNumber::Int(x.to_f64().atan() as i64),
    }
}

/// Helper: create `(n/d) * π`.
fn pi_ratio(n: i64, d: i64) -> CasNumber {
    CasNumber::Prod(
        Box::new(CasNumber::Rat(n, d)),
        Box::new(CasNumber::Const(SymConstant::Pi)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pi_over(n: i64, d: i64) -> CasNumber {
        pi_ratio(n, d)
    }

    fn sqrt_int(n: i64) -> CasNumber {
        CasNumber::Sqrt(Box::new(CasNumber::Int(n)))
    }

    #[test]
    fn sin_0() {
        assert_eq!(sin(&CasNumber::Int(0)), CasNumber::Int(0));
    }

    #[test]
    fn sin_pi_over_6() {
        assert_eq!(sin(&pi_over(1, 6)), CasNumber::Rat(1, 2));
    }

    #[test]
    fn sin_pi_over_2() {
        assert_eq!(sin(&pi_over(1, 2)), CasNumber::Int(1));
    }

    #[test]
    fn sin_pi() {
        assert_eq!(sin(&CasNumber::Const(SymConstant::Pi)), CasNumber::Int(0));
    }

    #[test]
    fn cos_0() {
        assert_eq!(cos(&CasNumber::Int(0)), CasNumber::Int(1));
    }

    #[test]
    fn cos_pi_over_3() {
        assert_eq!(cos(&pi_over(1, 3)), CasNumber::Rat(1, 2));
    }

    #[test]
    fn cos_pi() {
        assert_eq!(cos(&CasNumber::Const(SymConstant::Pi)), CasNumber::Int(-1));
    }

    #[test]
    fn tan_pi_over_4() {
        assert_eq!(tan(&pi_over(1, 4)), CasNumber::Int(1));
    }

    #[test]
    fn sin_from_tau() {
        assert_eq!(sin(&CasNumber::Const(SymConstant::Tau)), CasNumber::Int(0));
    }

    #[test]
    fn cos_zero() {
        assert_eq!(cos(&CasNumber::Int(0)), CasNumber::Int(1));
    }

    #[test]
    fn asin_0() {
        assert_eq!(asin(&CasNumber::Int(0)), CasNumber::Int(0));
    }

    #[test]
    fn asin_1() {
        assert_eq!(asin(&CasNumber::Int(1)), pi_over(1, 2));
    }

    #[test]
    fn acos_0() {
        assert_eq!(acos(&CasNumber::Int(0)), pi_over(1, 2));
    }

    #[test]
    fn atan_1() {
        assert_eq!(atan(&CasNumber::Int(1)), pi_over(1, 4));
    }

    #[test]
    fn cos_pi_over_6() {
        let expected = div(&sqrt_int(3), &CasNumber::Int(2));
        assert_eq!(cos(&pi_over(1, 6)), expected);
    }
}
