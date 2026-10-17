//! Shared call metadata for numeric and conditional syntax; no raw-name nodes.
use super::{ExprKind, Name, error};
use crate::{Builtin, ComparisonOp, Diagnostic, DiagnosticCode, LogicalOp, Span};
#[derive(Clone)]
pub(super) enum Function {
    Numeric(Builtin),
    Compare(ComparisonOp),
    Logical(LogicalOp),
    Not,
    Piecewise,
    User { name: Name, arity: usize },
}
impl Function {
    pub(super) fn from_name(name: &str) -> Option<Self> {
        if let Some(f) = Builtin::from_name(name) {
            return Some(Self::Numeric(f));
        }
        if let Some(op) = ComparisonOp::from_name(name) {
            return Some(Self::Compare(op));
        }
        Some(match name {
            "and" => Self::Logical(LogicalOp::And),
            "or" => Self::Logical(LogicalOp::Or),
            "not" => Self::Not,
            "piecewise" => Self::Piecewise,
            _ => return None,
        })
    }
    pub(super) fn finish(self, mut args: Vec<usize>, span: Span) -> Result<ExprKind, Diagnostic> {
        if let Self::User { name, arity } = self {
            if args.len() != arity {
                return Err(error(
                    DiagnosticCode::DeclarationConflict,
                    span,
                    "user-defined function argument count does not match its declaration",
                ));
            }
            return Ok(ExprKind::UserCall { name, args });
        }
        let valid = match &self {
            Self::Numeric(f) => f.accepts_arity(args.len()),
            Self::Compare(ComparisonOp::NotEqual) => args.len() == 2,
            Self::Compare(_) => args.len() >= 2,
            Self::Logical(_) => true,
            Self::Not => args.len() == 1,
            Self::Piecewise => !args.is_empty(),
            Self::User { .. } => unreachable!(),
        };
        if !valid {
            let code =
                if matches!(self, Self::Numeric(Builtin::Min | Builtin::Max)) && args.is_empty() {
                    DiagnosticCode::UnsupportedFeature
                } else {
                    DiagnosticCode::Syntax
                };
            return Err(error(
                code,
                span,
                "builtin argument count is outside its supported contract",
            ));
        }
        Ok(match self {
            Self::Numeric(mut function) => {
                if function == Builtin::Log && args.len() == 1 {
                    function = Builtin::Ln;
                }
                ExprKind::Call { function, args }
            }
            Self::Compare(op) => ExprKind::Compare {
                op,
                args,
                infix: false,
            },
            Self::Logical(op) => ExprKind::Logical {
                op,
                args,
                infix: false,
            },
            Self::Not => ExprKind::Not { child: args[0] },
            Self::Piecewise => {
                let otherwise = if args.len() % 2 == 1 {
                    args.pop()
                } else {
                    None
                };
                let branches = args.chunks_exact(2).map(|p| (p[0], p[1])).collect();
                ExprKind::Piecewise {
                    branches,
                    otherwise,
                }
            }
            Self::User { .. } => unreachable!(),
        })
    }
}
