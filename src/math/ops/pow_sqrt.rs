//! Power and square root operations for CasNumber.
//! Imported via `ops::{pow, sqrt}`; calls `ops::mul`, `ops::inv`, `ops::div`.

use crate::math::ops;
use crate::math::CasNumber;

/// Integer power: a ^ b where b is integer.
pub fn pow_int(a: &CasNumber, b: i64) -> CasNumber {
    match b.cmp(&0) {
        std::cmp::Ordering::Equal => CasNumber::Int(1),
        std::cmp::Ordering::Greater => {
            let mut result = CasNumber::Int(1);
            let mut base = a.clone();
            let mut exp = b;
            while exp > 0 {
                if exp & 1 == 1 {
                    result = ops::mul(&result, &base);
                }
                base = ops::mul(&base, &base);
                exp >>= 1;
            }
            result
        }
        std::cmp::Ordering::Less => {
            let pos = pow_int(a, -b);
            ops::inv(&pos)
        }
    }
}

/// Power operation with integer, rational, or sqrt exponent.
pub fn pow(base: &CasNumber, exp: &CasNumber) -> CasNumber {
    match (base, exp) {
        (_, CasNumber::Int(0)) => CasNumber::Int(1),
        (_, CasNumber::Int(1)) => base.clone(),
        (CasNumber::Int(0), _) if !is_exp_neg(exp) => CasNumber::Int(0),
        (base, CasNumber::Int(n)) => pow_int(base, *n),
        (base, CasNumber::Rat(1, 2)) => sqrt(base),
        (base, CasNumber::Rat(p, q)) => {
            let pow_p = pow_int(base, *p);
            if *q == 2 {
                sqrt(&pow_p)
            } else {
                CasNumber::Prod(Box::new(base.clone()), Box::new(CasNumber::Int(1)))
            }
        }
        _ => CasNumber::Prod(Box::new(base.clone()), Box::new(CasNumber::Int(1))),
    }
}

/// Square root.
pub fn sqrt(a: &CasNumber) -> CasNumber {
    match a {
        CasNumber::Int(0) | CasNumber::Int(1) => a.clone(),
        CasNumber::Int(n) if *n > 0 => {
            let r = (*n as f64).sqrt() as i64;
            if r * r == *n {
                return CasNumber::Int(r);
            }
            CasNumber::Sqrt(Box::new(a.clone()))
        }
        CasNumber::Rat(n, d) => {
            let sn = sqrt(&CasNumber::Int(*n));
            let sd = sqrt(&CasNumber::Int(*d));
            ops::div(&sn, &sd)
        }
        CasNumber::Sqrt(inner) => CasNumber::Sqrt(Box::new(CasNumber::Sqrt(inner.clone()))),
        _ => CasNumber::Sqrt(Box::new(a.clone())),
    }
}

fn is_exp_neg(exp: &CasNumber) -> bool {
    match exp {
        CasNumber::Int(n) => *n < 0,
        CasNumber::Rat(n, _) => *n < 0,
        CasNumber::Neg(_) => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pow_2_3() {
        assert_eq!(
            pow(&CasNumber::Int(2), &CasNumber::Int(3)),
            CasNumber::Int(8)
        );
    }
    #[test]
    fn pow_zero() {
        assert_eq!(
            pow(&CasNumber::Int(2), &CasNumber::Int(0)),
            CasNumber::Int(1)
        );
    }
    #[test]
    fn pow_neg() {
        assert_eq!(
            pow(&CasNumber::Int(2), &CasNumber::Int(-1)),
            CasNumber::Rat(1, 2)
        );
    }
    #[test]
    fn pow_half() {
        assert_eq!(
            pow(&CasNumber::Int(2), &CasNumber::Rat(1, 2)),
            CasNumber::Sqrt(Box::new(CasNumber::Int(2)))
        );
    }
    #[test]
    fn sqrt_9() {
        assert_eq!(sqrt(&CasNumber::Int(9)), CasNumber::Int(3));
    }
    #[test]
    fn sqrt_2() {
        assert_eq!(
            sqrt(&CasNumber::Int(2)),
            CasNumber::Sqrt(Box::new(CasNumber::Int(2)))
        );
    }
    #[test]
    fn sqrt_0() {
        assert_eq!(sqrt(&CasNumber::Int(0)), CasNumber::Int(0));
    }
}
