//! Shared scalar/stoichiometry envelope verified through the pinned reference.
use super::error;
use crate::{Diagnostic, DiagnosticCode, Span};

/// Avoid underflow, subnormal serialization and overflowing reference rounding.
/// This is a strict-profile restriction, not a claim that larger/smaller numbers
/// are invalid Antimony. Arithmetic and unit quantities have their own tighter
/// bounds; this guard covers unrestricted-syntax scalar roles only.
pub(super) fn check_scalar(value: f64, lexeme: &str, span: Span) -> Result<(), Diagnostic> {
    let mantissa = lexeme.split(['e', 'E']).next().expect("numeric mantissa");
    // Match arithmetic/unit literal counting: ignore exponent and leading zeros,
    // but retain trailing zeros after the first nonzero digit. An all-zero
    // mantissa has zero significant digits, regardless of its spelling.
    let significant_digits = mantissa
        .bytes()
        .filter(u8::is_ascii_digit)
        .skip_while(|b| *b == b'0')
        .count();
    let nonzero_mantissa = significant_digits != 0;
    if value.is_finite()
        && (significant_digits > 15
            || (value == 0.0 && nonzero_mantissa)
            || (value != 0.0 && !(1e-307..=1e308).contains(&value.abs())))
    {
        return Err(error(
            DiagnosticCode::UnsupportedFeature,
            span,
            "scalar literal exceeds this slice's verified serialization envelope: at most 15 significant digits, zero or magnitude [1e-307, 1e308]; nonzero underflow is not rounded to zero",
        ));
    }
    Ok(())
}

/// Reference `%g` notation thresholds at 15 significant digits. Retain exact
/// decimal source digits rather than dividing a binary float to get a mantissa.
pub(crate) fn scientific_parts(lexeme: &str, value: f64) -> Option<(String, i32)> {
    if value == 0.0 || (1e-4..1e15).contains(&value) {
        return None;
    }
    let (decimal, exponent) = match lexeme.split_once(['e', 'E']) {
        Some((decimal, exponent)) => (
            decimal,
            exponent.parse::<i32>().expect("bounded nonzero exponent"),
        ),
        None => (lexeme, 0),
    };
    let decimal_position = decimal.find('.').unwrap_or(decimal.len()) as i32;
    let digits: String = decimal.chars().filter(char::is_ascii_digit).collect();
    let first = digits.find(|c| c != '0').expect("nonzero literal");
    let significant = digits[first..].trim_end_matches('0');
    let mut mantissa = significant[..1].to_owned();
    if significant.len() > 1 {
        mantissa.push('.');
        mantissa.push_str(&significant[1..]);
    }
    Some((mantissa, exponent + decimal_position - first as i32 - 1))
}

/// Interpret a scientific MathML cn exactly as the reference does: binary64
/// mantissa multiplied by 10^exponent. This can differ one ULP from direct decimal
/// parsing, so initialization and the public numeric value use the exported form.
pub(crate) fn expression_value(lexeme: &str, parsed: f64) -> f64 {
    match scientific_parts(lexeme, parsed) {
        Some((mantissa, exponent)) => {
            mantissa.parse::<f64>().expect("numeric mantissa") * 10.0_f64.powf(f64::from(exponent))
        }
        None => parsed,
    }
}
