//! f64 approximation for `CasNumber` values.
use crate::math::CasNumber;

/// Format a `CasNumber` as its f64 approximation with 15 significant digits.
#[must_use]
pub fn format_15_sig_digits(val: &CasNumber) -> String {
    let f = val.to_f64();
    format_sig_digits(f, 15)
}

/// Format an f64 with N significant digits.
fn format_sig_digits(value: f64, sig_digits: usize) -> String {
    if value == 0.0 {
        return "0".to_string();
    }

    let abs_val = value.abs();
    let magnitude = abs_val.log10().floor();
    // Precision = sig_digits - 1 - magnitude (accounts for leading zeros after dot)
    // Clamp to at least 0 so format doesn't panic on negative precision.
    let precision = {
        let prec = (sig_digits as i32) - 1 - (magnitude as i32);
        if prec < 0 {
            0
        } else {
            prec as usize
        }
    };

    let formatted = format!("{value:.prec$}", prec = precision);
    if formatted.contains('.') {
        let trimmed = formatted
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_string();
        if trimmed.is_empty() || trimmed == "-" {
            "0".to_string()
        } else {
            trimmed
        }
    } else {
        formatted
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::SymConstant;

    #[test]
    fn approx_pi() {
        let pi = CasNumber::Const(SymConstant::Pi);
        let s = format_15_sig_digits(&pi);
        assert_eq!(s, "3.14159265358979", "got: {s}");
        let digits: Vec<char> = s.chars().filter(|&c| c != '.' && c != '-').collect();
        assert_eq!(digits.len(), 15, "got {digits:?} from {s}");
    }

    #[test]
    fn approx_e() {
        let e = CasNumber::Const(SymConstant::E);
        let s = format_15_sig_digits(&e);
        assert_eq!(s, "2.71828182845905", "got: {s}");
    }

    #[test]
    fn approx_tau() {
        let tau = CasNumber::Const(SymConstant::Tau);
        let s = format_15_sig_digits(&tau);
        assert_eq!(s, "6.28318530717959", "got: {s}");
    }

    #[test]
    fn approx_int() {
        assert_eq!(format_15_sig_digits(&CasNumber::Int(42)), "42");
    }

    #[test]
    fn approx_zero() {
        assert_eq!(format_15_sig_digits(&CasNumber::Int(0)), "0");
    }

    #[test]
    fn approx_rational() {
        let r = CasNumber::Rat(1, 3);
        let s = format_15_sig_digits(&r);
        assert_eq!(s, "0.333333333333333", "got: {s}");
    }

    #[test]
    fn approx_large() {
        assert_eq!(
            format_15_sig_digits(&CasNumber::Int(1_234_567_890)),
            "1234567890"
        );
    }

    #[test]
    fn approx_negative() {
        let s = format_15_sig_digits(&CasNumber::Int(-42));
        assert_eq!(s, "-42");
    }
}
