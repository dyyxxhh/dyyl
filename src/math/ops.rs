//! Core arithmetic operations on CasNumber.
//! Pow/sqrt and rounding are in sub-modules `pow_sqrt` and `round`.

pub mod pow_sqrt;
pub mod round;

use crate::math::{gcd, CasNumber};
pub use pow_sqrt::{pow, sqrt as sqrt_inner};
pub use round::{ceil, floor, round};

/// Add two `CasNumber` values.
pub fn add(a: &CasNumber, b: &CasNumber) -> CasNumber {
    match (a, b) {
        (CasNumber::Int(x), CasNumber::Int(y)) => CasNumber::Int(x + y),
        (CasNumber::Int(x), CasNumber::Rat(n, d)) => CasNumber::reduce(x * d + n, *d),
        (CasNumber::Rat(n, d), CasNumber::Int(x)) => CasNumber::reduce(n + x * d, *d),
        (CasNumber::Rat(an, ad), CasNumber::Rat(bn, bd)) => {
            let l = lcm64(*ad, *bd);
            let left = an * (l / *ad);
            let right = bn * (l / *bd);
            CasNumber::reduce(left + right, l)
        }
        _ if a.is_zero() => b.clone(),
        _ if b.is_zero() => a.clone(),
        (CasNumber::Const(ca), CasNumber::Const(cb)) if ca == cb => {
            CasNumber::Prod(Box::new(CasNumber::Int(2)), Box::new(CasNumber::Const(*ca)))
        }
        (CasNumber::Sqrt(sa), CasNumber::Sqrt(sb)) if sa == sb => CasNumber::Prod(
            Box::new(CasNumber::Int(2)),
            Box::new(CasNumber::Sqrt(sa.clone())),
        ),
        _ => CasNumber::Sum(Box::new(a.clone()), Box::new(b.clone())),
    }
}

/// Subtract b from a.
pub fn sub(a: &CasNumber, b: &CasNumber) -> CasNumber {
    add(a, &neg(b))
}

/// Multiply two `CasNumber` values.
pub fn mul(a: &CasNumber, b: &CasNumber) -> CasNumber {
    match (a, b) {
        _ if a.is_zero() || b.is_zero() => CasNumber::Int(0),
        _ if a.is_one() => b.clone(),
        _ if b.is_one() => a.clone(),
        _ if a.is_neg_one() => neg(b),
        _ if b.is_neg_one() => neg(a),
        (CasNumber::Int(x), CasNumber::Int(y)) => CasNumber::Int(x * y),
        (CasNumber::Int(x), CasNumber::Rat(n, d)) => CasNumber::reduce(x * n, *d),
        (CasNumber::Rat(n, d), CasNumber::Int(x)) => CasNumber::reduce(n * x, *d),
        (CasNumber::Rat(an, ad), CasNumber::Rat(bn, bd)) => CasNumber::reduce(an * bn, ad * bd),
        (CasNumber::Sqrt(sa), CasNumber::Sqrt(sb)) if sa == sb => *sa.clone(),
        (CasNumber::Sqrt(sa), CasNumber::Sqrt(sb)) => {
            let inner = mul(sa, sb);
            CasNumber::Sqrt(Box::new(inner))
        }
        (CasNumber::Int(_), CasNumber::Sqrt(_))
        | (CasNumber::Sqrt(_), CasNumber::Int(_))
        | (CasNumber::Rat(..), CasNumber::Sqrt(_))
        | (CasNumber::Sqrt(_), CasNumber::Rat(..)) => {
            CasNumber::Prod(Box::new(a.clone()), Box::new(b.clone()))
        }
        (CasNumber::Const(_), CasNumber::Int(n)) if *n != 0 => {
            if *n == 1 {
                a.clone()
            } else {
                CasNumber::Prod(Box::new(CasNumber::Int(*n)), Box::new(a.clone()))
            }
        }
        (CasNumber::Int(n), CasNumber::Const(_)) if *n != 0 => {
            if *n == 1 {
                b.clone()
            } else {
                CasNumber::Prod(Box::new(CasNumber::Int(*n)), Box::new(b.clone()))
            }
        }
        _ => CasNumber::Prod(Box::new(a.clone()), Box::new(b.clone())),
    }
}

/// Divide a by b.
pub fn div(a: &CasNumber, b: &CasNumber) -> CasNumber {
    match (a, b) {
        (_, CasNumber::Int(0)) => CasNumber::Int(0),
        (a, CasNumber::Int(1)) => a.clone(),
        (a, CasNumber::Int(-1)) => neg(a),
        (CasNumber::Int(x), CasNumber::Int(y)) if *y != 0 => CasNumber::reduce(*x, *y),
        (CasNumber::Rat(an, ad), CasNumber::Rat(bn, bd)) => CasNumber::reduce(an * bd, ad * bn),
        (CasNumber::Rat(n, d), CasNumber::Int(x)) if *x != 0 => CasNumber::reduce(*n, *d * x),
        (a, CasNumber::Rat(n, d)) => mul(a, &CasNumber::reduce(*d, *n)),
        (CasNumber::Int(_), CasNumber::Sqrt(_)) => {
            CasNumber::Prod(Box::new(a.clone()), Box::new(b.clone()))
        }
        (CasNumber::Sqrt(_), CasNumber::Int(d)) if *d != 0 => {
            CasNumber::Prod(Box::new(CasNumber::Rat(1, *d)), Box::new(a.clone()))
        }
        _ => mul(a, &inv(b)),
    }
}

/// Compute the multiplicative inverse.
pub fn inv(a: &CasNumber) -> CasNumber {
    match a {
        CasNumber::Int(0) => CasNumber::Int(0),
        CasNumber::Int(x) => CasNumber::reduce(1, *x),
        CasNumber::Rat(n, d) => CasNumber::reduce(*d, *n),
        CasNumber::Sqrt(_) => CasNumber::Prod(Box::new(CasNumber::Int(1)), Box::new(a.clone())),
        CasNumber::Const(_) | CasNumber::Sum(_, _) | CasNumber::Prod(_, _) => {
            CasNumber::Prod(Box::new(CasNumber::Int(1)), Box::new(a.clone()))
        }
        CasNumber::Neg(x) => neg(&inv(x)),
    }
}

/// Negate a value.
pub fn neg(a: &CasNumber) -> CasNumber {
    match a {
        CasNumber::Int(n) => CasNumber::Int(-n),
        CasNumber::Rat(n, d) => CasNumber::Rat(-n, *d),
        CasNumber::Neg(x) => *x.clone(),
        CasNumber::Sqrt(_) | CasNumber::Const(_) => CasNumber::Neg(Box::new(a.clone())),
        CasNumber::Sum(x, y) => CasNumber::Sum(Box::new(neg(x)), Box::new(neg(y))),
        CasNumber::Prod(x, y) => CasNumber::Prod(Box::new(neg(x)), y.clone()),
    }
}

/// Absolute value.
pub fn abs(a: &CasNumber) -> CasNumber {
    match a {
        CasNumber::Int(n) => CasNumber::Int(n.abs()),
        CasNumber::Rat(n, d) => CasNumber::Rat(n.abs(), *d),
        CasNumber::Neg(x) => abs(x),
        _ => a.clone(),
    }
}

/// Integer division (strike) toward zero.
pub fn strike(a: &CasNumber, b: &CasNumber) -> CasNumber {
    match (a, b) {
        (CasNumber::Int(x), CasNumber::Int(y)) if *y != 0 => CasNumber::Int(x / *y),
        _ => CasNumber::Int(0),
    }
}

/// Remainder (surplus) with sign of dividend.
pub fn surplus(a: &CasNumber, b: &CasNumber) -> CasNumber {
    match (a, b) {
        (CasNumber::Int(x), CasNumber::Int(y)) if *y != 0 => CasNumber::Int(x % *y),
        _ => CasNumber::Int(0),
    }
}

/// Natural logarithm (approximate with f64).
pub fn ln(a: &CasNumber) -> CasNumber {
    if a.is_zero() || is_cas_neg(a) {
        return CasNumber::Int(-1);
    }
    CasNumber::Int(a.to_f64().ln() as i64)
}

/// Square root (delegates to pow_sqrt::sqrt_inner).
pub fn sqrt(a: &CasNumber) -> CasNumber {
    sqrt_inner(a)
}

/// Check if a `CasNumber` is negative (for domain checks like log).
fn is_cas_neg(a: &CasNumber) -> bool {
    match a {
        CasNumber::Int(n) => *n < 0,
        CasNumber::Rat(n, _) => *n < 0,
        CasNumber::Neg(_) => true,
        _ => false,
    }
}

fn lcm64(a: i64, b: i64) -> i64 {
    if a == 0 || b == 0 {
        0
    } else {
        let g = gcd(a.unsigned_abs(), b.unsigned_abs()) as i64;
        a.abs() / g * b.abs()
    }
}

#[cfg(test)]
#[path = "ops/ops_tests.rs"]
mod tests;
