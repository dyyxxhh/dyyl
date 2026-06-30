//! Char-code arithmetic for math.add / math.sub.

use crate::math::CasNumber;

/// Add an integer to a single-character string by Unicode scalar offset.
/// Returns None if multi-char, non-integer offset, or invalid scalar result.
pub(crate) fn char_code_add(s: &str, offset: &CasNumber) -> Option<String> {
    let offset_i = offset.as_int()?;
    let mut chars = s.chars();
    let ch = chars.next()?;
    if chars.next().is_some() {
        return None;
    }
    let code = ch as u32;
    let new_code = if offset_i >= 0 {
        code.checked_add(offset_i as u32)?
    } else {
        code.checked_sub((-offset_i) as u32)?
    };
    char::from_u32(new_code).map(|c| c.to_string())
}

/// Subtract an integer from a single-character string by Unicode scalar offset.
pub(crate) fn char_code_sub(s: &str, offset: &CasNumber) -> Option<String> {
    let offset_i = offset.as_int()?;
    char_code_add(s, &CasNumber::Int(-offset_i))
}
