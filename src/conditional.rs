//! Typed conditional operators. Numeric/Boolean conversion belongs to the shared
//! initialization evaluator, never to source strings or a host-language runtime.
/// Relational operator, with ordered numeric-context arguments.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ComparisonOp {
    /// All successive arguments are equal.
    Equal,
    /// Exactly two arguments differ.
    NotEqual,
    /// Successive arguments strictly increase.
    Less,
    /// Successive arguments do not decrease.
    LessEqual,
    /// Successive arguments strictly decrease.
    Greater,
    /// Successive arguments do not increase.
    GreaterEqual,
}
impl ComparisonOp {
    pub(crate) fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "eq" | "equals" => Self::Equal,
            "neq" => Self::NotEqual,
            "lt" => Self::Less,
            "leq" => Self::LessEqual,
            "gt" => Self::Greater,
            "geq" => Self::GreaterEqual,
            _ => return None,
        })
    }
    pub(crate) fn compare(self, a: f64, b: f64) -> bool {
        match self {
            Self::Equal => a == b,
            Self::NotEqual => a != b,
            Self::Less => a < b,
            Self::LessEqual => a <= b,
            Self::Greater => a > b,
            Self::GreaterEqual => a >= b,
        }
    }
    #[cfg(feature = "sbml")]
    pub(crate) fn mathml(self) -> &'static str {
        match self {
            Self::Equal => "eq",
            Self::NotEqual => "neq",
            Self::Less => "lt",
            Self::LessEqual => "leq",
            Self::Greater => "gt",
            Self::GreaterEqual => "geq",
        }
    }
}
/// Ordered logical operator, with explicit finite truth conversion.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LogicalOp {
    /// Conjunction; zero arguments yield true.
    And,
    /// Disjunction; zero arguments yield false.
    Or,
}
impl LogicalOp {
    pub(crate) fn identity(self) -> bool {
        self == Self::And
    }
    #[cfg(feature = "sbml")]
    pub(crate) fn mathml(self) -> &'static str {
        if self == Self::And { "and" } else { "or" }
    }
}
