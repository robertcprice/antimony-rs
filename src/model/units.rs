//! Read-only unit metadata. Annotations never rescale mathematical values.
/// Supported SBML primitive kinds; this is intentionally not the full catalog.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum UnitKind {
    /// Substance amount.
    Mole,
    /// Time.
    Second,
    /// Volume (length cubed).
    Litre,
    /// Length.
    Metre,
    /// Explicit dimensionless units, distinct from unspecified units.
    Dimensionless,
}
impl UnitKind {
    /// Canonical SBML spelling.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Mole => "mole",
            Self::Second => "second",
            Self::Litre => "litre",
            Self::Metre => "metre",
            Self::Dimensionless => "dimensionless",
        }
    }
    pub(crate) fn from_name(name: &str) -> Option<Self> {
        match name {
            "mole" | "moles" => Some(Self::Mole),
            "second" | "seconds" => Some(Self::Second),
            "litre" | "litres" | "liter" | "liters" => Some(Self::Litre),
            "metre" | "metres" | "meters" => Some(Self::Metre),
            "dimensionless" => Some(Self::Dimensionless),
            _ => None,
        }
    }
}
/// One preserved SBML unit component: `(multiplier * 10^scale * kind)^exponent`.
#[derive(Clone, Debug)]
pub struct UnitComponent {
    pub(crate) kind: UnitKind,
    pub(crate) exponent: i32,
    pub(crate) scale: i32,
    pub(crate) multiplier: f64,
}
impl UnitComponent {
    /// Primitive SBML kind.
    pub fn kind(&self) -> UnitKind {
        self.kind
    }
    /// Integer exponent within the published implementation bound.
    pub fn exponent(&self) -> i32 {
        self.exponent
    }
    /// Decimal scale. It affects metadata, never the stored parameter value.
    pub fn scale(&self) -> i32 {
        self.scale
    }
    /// Positive finite decimal multiplier.
    pub fn multiplier(&self) -> f64 {
        self.multiplier
    }
}
/// Preserved source or generated definition after bounded alias expansion.
/// Generated species definitions may contain repeated primitive components.
#[derive(Clone, Debug)]
pub struct UnitDefinition {
    pub(crate) id: String,
    pub(crate) components: Vec<UnitComponent>,
}
impl UnitDefinition {
    /// Unit identifier.
    pub fn id(&self) -> &str {
        &self.id
    }
    /// Components in upstream expression order.
    pub fn components(&self) -> &[UnitComponent] {
        &self.components
    }
}
/// Dimension exponents for the supported substance/time/length primitive subset.
/// Decimal scale and multiplier are intentionally not dimensions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Dimensions {
    pub(crate) substance: i32,
    pub(crate) time: i32,
    pub(crate) length: i32,
}
impl Dimensions {
    /// Substance amount exponent.
    pub fn substance(self) -> i32 {
        self.substance
    }
    /// Time exponent.
    pub fn time(self) -> i32 {
        self.time
    }
    /// Length exponent (volume contributes three).
    pub fn length(self) -> i32 {
        self.length
    }
}
/// Conservative dimensional knowledge, not a model-wide certificate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DimensionalStatus {
    /// Explicitly known dimensions, possibly dimensionless.
    Known(Dimensions),
    /// No explicit units; this is never assumed dimensionless.
    Unspecified,
    /// Operation has unknown operands or falls outside the dimensional checker.
    Indeterminate,
}
/// Exact fixed bounds on the implemented unit language, additional to source/token/entity limits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UnitLimits;
impl UnitLimits {
    /// Maximum absolute source exponent; zero exponents are unsupported.
    pub const SOURCE_EXPONENT: i32 = 16;
    /// Maximum absolute exponent after alias expansion.
    pub const RESOLVED_EXPONENT: i32 = 64;
    /// Maximum distinct primitive components after expansion.
    pub const COMPONENTS: usize = 5;
    /// Generated concentration-times-volume definitions preserve at most ten components.
    pub const GENERATED_COMPONENTS: usize = 10;
    /// Maximum significant digits of a leading scalar coefficient.
    pub const COEFFICIENT_DIGITS: usize = 15;
    /// Minimum decimal scale of a normalized leading coefficient.
    pub const MIN_SCALE: i32 = -12;
    /// Maximum decimal scale of a normalized leading coefficient.
    pub const MAX_SCALE: i32 = 12;
}

/// Explicitly selected model-wide SBML default. Absence remains unspecified.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum ModelUnit {
    /// Default substance amount units.
    Substance,
    /// Default three-dimensional compartment units.
    Volume,
    /// Default two-dimensional compartment units.
    Area,
    /// Default one-dimensional compartment units.
    Length,
    /// Default reaction extent units.
    Extent,
    /// Default model time units (`time_unit` in Antimony).
    Time,
}
impl ModelUnit {
    /// Antimony identifier for this default.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Substance => "substance",
            Self::Volume => "volume",
            Self::Area => "area",
            Self::Length => "length",
            Self::Extent => "extent",
            Self::Time => "time_unit",
        }
    }
    pub(crate) fn from_name(name: &str) -> Option<Self> {
        match name {
            "substance" => Some(Self::Substance),
            "volume" => Some(Self::Volume),
            "area" => Some(Self::Area),
            "length" => Some(Self::Length),
            "extent" => Some(Self::Extent),
            "time_unit" => Some(Self::Time),
            _ => None,
        }
    }
    /// SBML Model attribute associated with this default.
    pub fn sbml_attribute(self) -> &'static str {
        match self {
            Self::Substance => "substanceUnits",
            Self::Volume => "volumeUnits",
            Self::Area => "areaUnits",
            Self::Length => "lengthUnits",
            Self::Extent => "extentUnits",
            Self::Time => "timeUnits",
        }
    }
}
