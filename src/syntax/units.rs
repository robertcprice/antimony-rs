//! Restricted unit statements kept separate from arithmetic-expression parsing.
use super::Parser;
use crate::model::units::UnitLimits;
use crate::syntax::lexer::Kind;
use crate::syntax::{Statement, UnitFactor, error};
use crate::{Diagnostic, DiagnosticCode};

impl Parser<'_> {
    pub(super) fn unit_statement(&mut self) -> Result<bool, Diagnostic> {
        if self.is_word("unit") {
            self.bump();
            let name = self.name()?;
            self.expect(Kind::Equal, "unit definition requires =")?;
            let factors = self.unit_factors()?;
            self.statements
                .push(Statement::UnitDefinition { name, factors });
            self.terminator()?;
            return Ok(true);
        }
        let mut look = self.pos;
        if matches!(self.tokens.get(look).map(|t| &t.kind), Some(Kind::Ident(_))) {
            look += 1;
            while self.tokens.get(look).map(|t| &t.kind) == Some(&Kind::Dot)
                && matches!(
                    self.tokens.get(look + 1).map(|t| &t.kind),
                    Some(Kind::Ident(_))
                )
            {
                look += 2;
            }
            if matches!(
                self.tokens.get(look).map(|t| &t.kind),
                Some(Kind::Ident(s)) if s == "has"
            ) {
                let name = self.dotted_name()?;
                self.bump();
                let unit = self.name()?;
                if matches!(
                    self.token().kind,
                    Kind::Star | Kind::Slash | Kind::Caret | Kind::LParen
                ) {
                    return Err(self.unsupported(
                        "inline unit expressions are deferred; use a named unit definition",
                    ));
                }
                self.statements
                    .push(Statement::UnitAnnotation { name, unit });
                self.terminator()?;
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn unit_factors(&mut self) -> Result<Vec<UnitFactor>, Diagnostic> {
        let mut factors = Vec::new();
        let mut direction = 1;
        let mut coefficient = None;
        if let Kind::Number(text) = &self.token().kind {
            coefficient = Some(text.clone());
            self.bump();
            if self.token().kind == Kind::Star {
                self.bump();
            } else if self.token().kind == Kind::Slash {
                if coefficient.as_deref() != Some("1") {
                    return Err(
                        self.unsupported("scaled reciprocal units are deferred; use a unit power")
                    );
                }
                coefficient = None;
                direction = -1;
                self.bump();
            }
        }
        loop {
            if !matches!(self.token().kind, Kind::Ident(_)) {
                return Err(self.unsupported("unit factors require named units; parentheses and internal scalar coefficients are deferred"));
            }
            let name = self.name()?;
            let mut exponent = 1;
            if self.token().kind == Kind::Caret {
                self.bump();
                let sign = if self.token().kind == Kind::Minus {
                    self.bump();
                    -1
                } else {
                    1
                };
                let token = self.bump();
                let Kind::Number(text) = token.kind else {
                    return Err(error(
                        DiagnosticCode::UnsupportedFeature,
                        token.span,
                        "unit powers require bounded nonzero integer literals",
                    ));
                };
                exponent = text.parse::<i32>().ok().filter(|n| *n > 0 && *n <= UnitLimits::SOURCE_EXPONENT)
                    .ok_or_else(|| error(DiagnosticCode::UnsupportedFeature, token.span, "unit source exponent must be a nonzero integer with magnitude at most 16"))? * sign;
            }
            if coefficient.is_some() && (direction != 1 || exponent != 1) {
                return Err(self.unsupported("scalar coefficients on powered units are deferred because juxtaposition and multiplication differ upstream"));
            }
            factors.push(UnitFactor {
                name,
                coefficient: coefficient.take(),
                exponent: direction * exponent,
            });
            if factors.len() > UnitLimits::COMPONENTS {
                return Err(self.unsupported(
                    "unit definitions are bounded to five distinct primitive components",
                ));
            }
            direction = match self.token().kind {
                Kind::Star => 1,
                Kind::Slash => -1,
                _ => break,
            };
            self.bump();
        }
        Ok(factors)
    }
}
