//! Verified, bounded numeric vocabulary shared by syntax, analysis and export.

/// Numeric builtins admitted by the math-core profile. All arguments/results are real-valued.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Builtin {
    /// Sine (radians).
    Sin,
    /// Cosine (radians).
    Cos,
    /// Tangent (radians).
    Tan,
    /// Natural exponential.
    Exp,
    /// Natural logarithm; source `ln(x)` or unary `log(x)`.
    Ln,
    /// Base-ten logarithm.
    Log10,
    /// Logarithm with arguments `(base, value)`.
    Log,
    /// Principal real square root.
    Sqrt,
    /// Absolute value.
    Abs,
    /// Round down to an integer.
    Floor,
    /// Round up to an integer; source `ceil` or `ceiling`.
    Ceiling,
    /// Real power with arguments `(base, exponent)`; source `pow` or `power`.
    Power,
    /// Minimum of one or more numeric arguments.
    Min,
    /// Maximum of one or more numeric arguments.
    Max,
}
#[derive(Clone, Copy)]
pub(crate) enum UnitBehavior {
    Dimensionless,
    Preserve,
    Compatible,
    Root,
    Power,
}
impl Builtin {
    pub(crate) fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "sin" => Self::Sin,
            "cos" => Self::Cos,
            "tan" => Self::Tan,
            "exp" => Self::Exp,
            "ln" => Self::Ln,
            "log" => Self::Log,
            "log10" => Self::Log10,
            "sqrt" => Self::Sqrt,
            "abs" => Self::Abs,
            "floor" => Self::Floor,
            "ceil" | "ceiling" => Self::Ceiling,
            "pow" | "power" => Self::Power,
            "min" => Self::Min,
            "max" => Self::Max,
            _ => return None,
        })
    }
    pub(crate) fn accepts_arity(self, count: usize) -> bool {
        match self {
            Self::Log => count == 1 || count == 2,
            Self::Power => count == 2,
            Self::Min | Self::Max => count >= 1,
            _ => count == 1,
        }
    }
    /// Content MathML operator (log bases and root degrees are emitted separately).
    pub fn mathml_operator(self) -> &'static str {
        match self {
            Self::Sin => "sin",
            Self::Cos => "cos",
            Self::Tan => "tan",
            Self::Exp => "exp",
            Self::Ln => "ln",
            Self::Log | Self::Log10 => "log",
            Self::Sqrt => "root",
            Self::Abs => "abs",
            Self::Floor => "floor",
            Self::Ceiling => "ceiling",
            Self::Power => "power",
            Self::Min => "min",
            Self::Max => "max",
        }
    }
    pub(crate) fn unit_behavior(self) -> UnitBehavior {
        match self {
            Self::Abs | Self::Floor | Self::Ceiling => UnitBehavior::Preserve,
            Self::Min | Self::Max => UnitBehavior::Compatible,
            Self::Sqrt => UnitBehavior::Root,
            Self::Power => UnitBehavior::Power,
            _ => UnitBehavior::Dimensionless,
        }
    }
    // Evaluate only initialization-time arithmetic. Unknowns stay unknown, but a
    // known invalid argument still rejects, even beside an unset dependency.
    pub(crate) fn evaluate(self, args: &[Option<f64>]) -> Result<Option<f64>, &'static str> {
        if matches!(self, Self::Ln | Self::Log10) && args[0].is_some_and(|x| x <= 0.0)
            || self == Self::Log
                && (args[0].is_some_and(|b| b <= 0.0 || b == 1.0)
                    || args[1].is_some_and(|x| x <= 0.0))
            || self == Self::Sqrt && args[0].is_some_and(|x| x < 0.0)
        {
            return Err("builtin initialization is outside the finite real domain");
        }
        if args.iter().any(Option::is_none) {
            return Ok(None);
        }
        let a = args[0].expect("known arguments");
        let value = match self {
            Self::Sin => a.sin(),
            Self::Cos => a.cos(),
            Self::Tan => a.tan(),
            Self::Exp => a.exp(),
            Self::Ln => a.ln(),
            Self::Log10 => a.log10(),
            Self::Log => args[1].unwrap().log(a),
            Self::Sqrt => a.sqrt(),
            Self::Abs => a.abs(),
            Self::Floor => a.floor(),
            Self::Ceiling => a.ceil(),
            Self::Power => {
                let b = args[1].unwrap();
                if a < 0.0 && b.fract() != 0.0 || a == 0.0 && b <= 0.0 {
                    return Err("initialization power is outside the supported real domain");
                }
                a.powf(b)
            }
            Self::Min => args.iter().map(|v| v.unwrap()).fold(a, f64::min),
            Self::Max => args.iter().map(|v| v.unwrap()).fold(a, f64::max),
        };
        Ok(Some(value))
    }
}
/// Builtin mathematical constants, not model symbols or guessed parameter defaults.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NumericConstant {
    /// The ratio of a circle's circumference to diameter.
    Pi,
    /// Euler's number.
    ExponentialE,
}
impl NumericConstant {
    /// Finite binary64 numerical approximation for initialization evaluation.
    pub fn value(self) -> f64 {
        match self {
            Self::Pi => std::f64::consts::PI,
            Self::ExponentialE => std::f64::consts::E,
        }
    }
    /// Content MathML constant element.
    pub fn mathml_element(self) -> &'static str {
        match self {
            Self::Pi => "pi",
            Self::ExponentialE => "exponentiale",
        }
    }
}
