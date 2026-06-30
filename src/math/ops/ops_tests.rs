use super::*;

#[test]
fn add_int_int() {
    assert_eq!(
        add(&CasNumber::Int(3), &CasNumber::Int(4)),
        CasNumber::Int(7)
    );
}
#[test]
fn add_int_rat() {
    assert_eq!(
        add(&CasNumber::Int(1), &CasNumber::Rat(1, 3)),
        CasNumber::Rat(4, 3)
    );
}
#[test]
fn add_rat_rat_same() {
    assert_eq!(
        add(&CasNumber::Rat(1, 6), &CasNumber::Rat(1, 6)),
        CasNumber::Rat(1, 3)
    );
}
#[test]
fn add_rat_rat_diff() {
    assert_eq!(
        add(&CasNumber::Rat(1, 3), &CasNumber::Rat(1, 6)),
        CasNumber::Rat(1, 2)
    );
}
#[test]
fn mul_int_int() {
    assert_eq!(
        mul(&CasNumber::Int(3), &CasNumber::Int(4)),
        CasNumber::Int(12)
    );
}
#[test]
fn mul_int_rat() {
    assert_eq!(
        mul(&CasNumber::Int(3), &CasNumber::Rat(1, 3)),
        CasNumber::Int(1)
    );
}
#[test]
fn mul_rat_rat() {
    assert_eq!(
        mul(&CasNumber::Rat(2, 3), &CasNumber::Rat(3, 4)),
        CasNumber::Rat(1, 2)
    );
}
#[test]
fn mul_by_zero() {
    assert_eq!(
        mul(&CasNumber::Int(0), &CasNumber::Int(42)),
        CasNumber::Int(0)
    );
}
#[test]
fn mul_one_third_by_three() {
    assert_eq!(
        mul(&CasNumber::Rat(1, 3), &CasNumber::Int(3)),
        CasNumber::Int(1)
    );
}
#[test]
fn div_int_int() {
    assert_eq!(
        div(&CasNumber::Int(6), &CasNumber::Int(3)),
        CasNumber::Int(2)
    );
}
#[test]
fn div_int_by_int() {
    assert_eq!(
        div(&CasNumber::Int(1), &CasNumber::Int(3)),
        CasNumber::Rat(1, 3)
    );
}
#[test]
fn neg_int() {
    assert_eq!(neg(&CasNumber::Int(5)), CasNumber::Int(-5));
}
#[test]
fn abs_value() {
    assert_eq!(abs(&CasNumber::Int(-5)), CasNumber::Int(5));
}
#[test]
fn abs_rat() {
    assert_eq!(abs(&CasNumber::Rat(-3, 4)), CasNumber::Rat(3, 4));
}
#[test]
fn strike_positive() {
    assert_eq!(
        strike(&CasNumber::Int(7), &CasNumber::Int(2)),
        CasNumber::Int(3)
    );
}
#[test]
fn strike_negative() {
    assert_eq!(
        strike(&CasNumber::Int(-7), &CasNumber::Int(2)),
        CasNumber::Int(-3)
    );
}
#[test]
fn surplus_positive() {
    assert_eq!(
        surplus(&CasNumber::Int(7), &CasNumber::Int(3)),
        CasNumber::Int(1)
    );
}
#[test]
fn surplus_negative() {
    assert_eq!(
        surplus(&CasNumber::Int(-7), &CasNumber::Int(2)),
        CasNumber::Int(-1)
    );
}
#[test]
fn inv_rational() {
    assert_eq!(inv(&CasNumber::Rat(2, 3)), CasNumber::Rat(3, 2));
}
#[test]
fn sqrt_perf() {
    assert_eq!(sqrt(&CasNumber::Int(9)), CasNumber::Int(3));
}
#[test]
fn sqrt_non_square() {
    assert_eq!(
        sqrt(&CasNumber::Int(2)),
        CasNumber::Sqrt(Box::new(CasNumber::Int(2)))
    );
}
#[test]
fn sqrt_zero() {
    assert_eq!(sqrt(&CasNumber::Int(0)), CasNumber::Int(0));
}
#[test]
fn sqrt_rational() {
    assert_eq!(sqrt(&CasNumber::Rat(4, 9)), CasNumber::Rat(2, 3));
}
