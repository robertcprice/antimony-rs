//! Small, deliberately restricted flat-model syntax, not the full profile grammar.
mod functions;
mod imports;
pub(crate) mod lexer;
pub(crate) mod numeric;
mod parser;
mod reserved;

use crate::{Diagnostic, DiagnosticCode, Label, Limits, Severity, SourceFile, Span};
pub use imports::{load_file, parse_with_imports};
pub(crate) use parser::parse_document;

/// Owned syntax for the implemented units flat-model slice.
///
/// Construction validates syntax and resource limits, not model semantics.
#[derive(Clone, Debug)]
pub struct ParsedDocument {
    pub(crate) source: SourceFile,
    pub(crate) limits: Limits,
    pub(crate) name: String,
    pub(crate) functions: Vec<ParsedFunction>,
    pub(crate) modules: Vec<ParsedModule>,
    pub(crate) statements: Vec<Statement>,
    pub(crate) expressions: Vec<Expr>,
}
impl ParsedDocument {
    /// Original source retained for diagnostics.
    pub fn source(&self) -> &SourceFile {
        &self.source
    }
    /// Named model ID, or `__main` for an anonymous model.
    pub fn model_id(&self) -> &str {
        &self.name
    }
    pub(crate) fn function_body_nodes(&self) -> Vec<bool> {
        let mut flags = vec![false; self.expressions.len()];
        let mut stack = self.functions.iter().map(|f| f.body).collect::<Vec<_>>();
        while let Some(id) = stack.pop() {
            if !flags[id] {
                flags[id] = true;
                stack.extend(self.expressions[id].kind.children());
            }
        }
        flags
    }
}
#[derive(Clone, Debug)]
pub(crate) struct ParsedFunction {
    pub name: Name,
    pub parameters: Vec<Name>,
    pub body: usize,
    pub span: Span,
}
#[derive(Clone, Debug)]
pub(crate) struct ParsedModule {
    pub name: Name,
    pub ports: Vec<Name>,
    pub statements: Vec<Statement>,
    pub span: Span,
}
#[derive(Clone, Debug)]
pub(crate) struct ParsedEventAssignment {
    pub variable: Name,
    pub expression: usize,
    pub span: Span,
}
#[derive(Clone, Debug)]
pub(crate) struct Name {
    pub text: String,
    pub span: Span,
}
#[derive(Clone, Debug)]
pub(crate) enum Statement {
    UnitDefinition {
        name: Name,
        factors: Vec<UnitFactor>,
    },
    UnitAnnotation {
        name: Name,
        unit: Name,
    },
    Compartment {
        name: Name,
        value: Option<InitialValue>,
        qualifier: Option<bool>,
    },
    Species {
        name: Name,
        compartment: Option<Name>,
        boundary: bool,
        qualifier: Option<bool>,
        substance_only: bool,
    },
    Parameter {
        name: Name,
        qualifier: Option<bool>,
        value: Option<InitialValue>,
    },
    Initial {
        name: Name,
        value: InitialValue,
    },
    Rule {
        kind: crate::RuleKind,
        name: Name,
        expression: usize,
        lexical_scalar: Option<f64>,
        qualifier: Option<bool>,
        span: Span,
    },
    Reaction {
        name: Name,
        start: Span,
        reactants: Vec<Participant>,
        products: Vec<Participant>,
        reversible: bool,
        rate: usize,
    },
    Event {
        name: Name,
        start: Span,
        trigger: usize,
        delay: Option<usize>,
        priority: Option<usize>,
        use_values_from_trigger_time: bool,
        persistent: bool,
        initial_value: bool,
        assignments: Vec<ParsedEventAssignment>,
        span: Span,
    },
    Alias {
        left: Name,
        right: Name,
        span: Span,
    },
    Submodel {
        instance: Name,
        module: Name,
        args: Vec<Name>,
        timeconv: Option<Name>,
        extentconv: Option<Name>,
        span: Span,
    },
    Delete {
        targets: Vec<Name>,
    },
}
#[derive(Clone, Debug)]
pub(crate) struct UnitFactor {
    pub name: Name,
    pub coefficient: Option<String>,
    pub exponent: i32,
}
/// Scalar syntax is kept distinct from expression syntax: upstream exports even
/// a parenthesized literal or unary plus as an InitialAssignment.
#[derive(Clone, Copy, Debug)]
pub(crate) enum InitialValue {
    Scalar(f64),
    Expression(usize),
}
#[derive(Clone, Debug)]
pub(crate) struct Participant {
    pub name: Name,
    pub stoichiometry: f64,
    pub boundary: bool,
}
#[derive(Clone, Debug)]
pub(crate) struct Expr {
    pub kind: ExprKind,
    pub span: Span,
    pub depth: usize,
}
#[derive(Clone, Debug)]
pub(crate) enum ExprKind {
    Boolean(bool),
    Not {
        child: usize,
    },
    Compare {
        op: crate::ComparisonOp,
        args: Vec<usize>,
        infix: bool,
    },
    Logical {
        op: crate::LogicalOp,
        args: Vec<usize>,
        infix: bool,
    },
    Piecewise {
        branches: Vec<(usize, usize)>,
        otherwise: Option<usize>,
    },
    Call {
        function: crate::Builtin,
        args: Vec<usize>,
    },
    UserCall {
        name: Name,
        args: Vec<usize>,
    },
    Rational {
        numerator: i64,
        denominator: i64,
    },
    Time,
    Constant(crate::NumericConstant),
    Number {
        lexeme: String,
        value: f64,
    },
    Symbol(Name),
    Unary {
        op: UnaryOp,
        child: usize,
    },
    Binary {
        op: BinaryOp,
        left: usize,
        right: usize,
    },
}
/// Arithmetic unary operator in a supported arithmetic expression.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnaryOp {
    /// Unary plus (identity).
    Plus,
    /// Unary minus.
    Minus,
}
/// Arithmetic binary operator in a supported arithmetic expression.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BinaryOp {
    /// Addition.
    Add,
    /// Subtraction.
    Subtract,
    /// Multiplication.
    Multiply,
    /// Division.
    Divide,
    /// Right-associative exponentiation, above unary minus.
    Power,
}
pub(crate) fn error(code: DiagnosticCode, span: Span, message: impl Into<String>) -> Diagnostic {
    Diagnostic::new(Severity::Error, code, message).with_label(Label::primary(span, "here"))
}

impl ExprKind {
    /// All syntactic children, including inactive branches. No recursion or allocation.
    pub(crate) fn children(&self) -> impl DoubleEndedIterator<Item = usize> + '_ {
        let mut first = None;
        let mut second = None;
        let mut args: &[usize] = &[];
        let mut branches: &[(usize, usize)] = &[];
        let mut otherwise = None;
        match self {
            Self::Unary { child, .. } | Self::Not { child } => first = Some(*child),
            Self::Binary { left, right, .. } => {
                first = Some(*left);
                second = Some(*right);
            }
            Self::Call { args: a, .. }
            | Self::UserCall { args: a, .. }
            | Self::Compare { args: a, .. }
            | Self::Logical { args: a, .. } => args = a,
            Self::Piecewise {
                branches: b,
                otherwise: o,
            } => {
                branches = b;
                otherwise = *o;
            }
            _ => (),
        }
        first
            .into_iter()
            .chain(second)
            .chain(args.iter().copied())
            .chain(branches.iter().flat_map(|(v, c)| [*v, *c]))
            .chain(otherwise)
    }
}
impl ExprKind {
    pub(crate) fn map_children(&self, mut map: impl FnMut(usize) -> usize) -> Self {
        let mut kind = self.clone();
        match &mut kind {
            Self::Unary { child, .. } | Self::Not { child } => *child = map(*child),
            Self::Binary { left, right, .. } => {
                *left = map(*left);
                *right = map(*right);
            }
            Self::Call { args, .. }
            | Self::UserCall { args, .. }
            | Self::Compare { args, .. }
            | Self::Logical { args, .. } => {
                for id in args {
                    *id = map(*id);
                }
            }
            Self::Piecewise {
                branches,
                otherwise,
            } => {
                for (v, c) in branches {
                    *v = map(*v);
                    *c = map(*c);
                }
                if let Some(id) = otherwise {
                    *id = map(*id);
                }
            }
            _ => (),
        }
        kind
    }
}
