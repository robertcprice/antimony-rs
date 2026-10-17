//! Pinned formula lexer recognizes parenthesized signed-integer quotients as
//! single rational tokens, including their parentheses. Match source tokens
//! before groups disappear; never infer this distinction from a divide subtree.
use super::*;

impl Parser<'_> {
    /// Returns components and the token index after the complete rational token.
    /// Component bounds are checked after unary normalization, not here.
    pub(super) fn rational_at(
        &self,
        start: usize,
    ) -> Result<Option<(i64, i64, usize)>, Diagnostic> {
        if self.tokens.get(start).map(|t| &t.kind) != Some(&Kind::LParen) {
            return Ok(None);
        }
        let Some((numerator, pos)) = self.rational_component(start + 1)? else {
            return Ok(None);
        };
        if self.tokens.get(pos).map(|t| &t.kind) != Some(&Kind::Slash) {
            return Ok(None);
        }
        let Some((denominator, pos)) = self.rational_component(pos + 1)? else {
            return Ok(None);
        };
        if self.tokens.get(pos).map(|t| &t.kind) != Some(&Kind::RParen) {
            return Ok(None);
        }
        Ok(Some((numerator, denominator, pos + 1)))
    }

    fn rational_component(&self, mut pos: usize) -> Result<Option<(i64, usize)>, Diagnostic> {
        let negative = self.tokens.get(pos).map(|t| &t.kind) == Some(&Kind::Minus);
        pos += usize::from(negative);
        let Some(Token {
            kind: Kind::Number(lexeme),
            span,
        }) = self.tokens.get(pos)
        else {
            return Ok(None);
        };
        let Ok(value) = lexeme.parse::<f64>() else {
            return Ok(None);
        };
        // Antimony serializes numeric tokens with default-format precision15.
        // Under the existing significant-digit bound, a finite integral value
        // below the scientific threshold has exactly this plain spelling.
        if !value.is_finite()
            || value.fract() != 0.0
            || crate::syntax::numeric::scientific_parts(lexeme, value).is_some()
        {
            return Ok(None);
        }
        crate::syntax::numeric::check_scalar(value, lexeme, *span)?;
        let integer = value as i64; // plain spelling implies 0 <= value < 1e15
        Ok(Some((if negative { -integer } else { integer }, pos + 1)))
    }
}
