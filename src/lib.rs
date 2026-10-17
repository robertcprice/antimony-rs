//! A deliberately narrow native Antimony conditional-math compiler slice.
//!
//! This is **not full Antimony or full kinetic-core-v0.1 support**.
//! Bounded initialization, reaction arithmetic, explicit species/parameter/compartment
//! units and six model-default unit declarations are implemented. Species source units
//! denote concentration; their separately preserved SBML substance references follow
//! the audited upstream mapping without rescaling numbers. Rules, events, user functions,
//! imports and compartment expressions are explicitly deferred.
//! Empty model-header parentheses are optional. Comma-separated `formula` parameter
//! declarations reuse the same checked initialization semantics.
//! Canonical formatting preserves source token order and spelling, not comments or
//! whitespace. No simulation is provided.
//!
//! # Deliberate slice restrictions
//!
//! - One named `model name ... end` / `model name() ... end` or anonymous flat model;
//!   a newline/semicolon follows the header. Named and unnamed reactions are supported.
//! - Source-ordered declarations support role promotion, const/var/$ qualifiers,
//!   compatible redeclaration, last initialization and reaction-law replacement.
//!   `parameter` is not an accepted declaration keyword. Unset values remain absent;
//!   symbolic initializers remain InitialAssignments, never guessed constants.
//! - Finite scalar or acyclic expression initialization for species/parameters;
//!   explicit compartments permit unset or fixed positive scalar sizes. Species denote concentration,
//!   never amount. Reference-normalized initialization trees export as InitialAssignments.
//! - Reaction participants and rate require a **semicolon**, not just a newline.
//! - Numeric builtin calls support sin/cos/tan/exp/ln/log/log10/sqrt/abs/floor,
//!   ceil/ceiling, pow/power and nonempty min/max. Mathematical constants pi and
//!   exponentiale and the time/time() symbol have explicit expression nodes.
//!   Unary log means ln; binary log takes (base, value). Time initialization stays
//!   symbolic (`None`), not zero. Bounded Boolean/comparison/logical/piecewise
//!   expressions use explicit finite numeric/Boolean conversion and lazy branch
//!   initialization. Six Boolean literal aliases and capitalized zero-argument calls
//!   True()/TRUE()/False()/FALSE() are supported; lowercase Boolean calls reject.
//!   Textual logical aliases, comparison chains and XOR/implies defer.
//! - Builtin declarations and explicit `default_compartment` declarations reject.
//! - Verified unary forms and nested `+`/`*` groups are supported; contiguous
//!   `--` remains invalid upstream. Source layout is retained, while semantic
//!   analysis and export share the reference-normalized expression tree.
//! - Arithmetic-expression literals allow at most 15 significant digits and are zero or have
//!   magnitude in `[1e-307, 1e308]`. Outside this verified serialization envelope
//!   they fail explicitly, rather than silently round to match an oracle.
//! - Scalar species/parameter initialization, compartment sizes and stoichiometry
//!   have at most 15 significant digits and are zero or have magnitude in
//!   `[1e-307, 1e308]` (positive where required). The exponent and leading zeros
//!   do not count; trailing zeros after the first nonzero digit do. An all-zero
//!   mantissa has zero significant digits.
//!   Nonzero lexical underflow rejects; signed and scientific zero remain zero.
//!   Values outside this conservative envelope are unsupported, not declared
//!   invalid Antimony. Unit-coefficient numeric envelopes remain separately bounded.
//! - SBML numeric leaves use reference-verified integer/real/scientific representation
//!   (including signed numeric leaves); original lexemes remain in the IR.
//! - Unit definitions use a verified five-kind catalog, flat products/quotients,
//!   bounded integer powers and direct leading scalar coefficients. Parentheses,
//!   repeated source primitive kinds, new coefficients on aliases and custom names ending in `s` are
//!   deferred. [`UnitLimits`] documents fixed safety/serialization bounds.
//! - Model defaults use `unit substance/volume/area/length/extent/time_unit = ...`.
//!   Absent defaults remain absent; primitive/equivalent-definition redirects preserve
//!   upstream SBML identity. Incompatible known default dimensions reject.
//! - Species conversion preserves raw alias order, exact inverse-pair cancellation
//!   and generated definitions. Unsafe identifier collisions and upstream dangling
//!   references reject rather than overwrite model state or invent missing units.
//!   Implicit compartments use upstream's literal-litre conversion even with a volume
//!   default; any resulting known concentration-dimension conflict rejects.
//! - `has` annotations never rescale numbers. Dimensional knowledge is exposed
//!   through [`DimensionalStatus`]; successful checking is not full certification.
//! - No solver, complete language support, complete dimensional analysis, or
//!   cross-platform compatibility claim is made by successful compilation.
//!
//! Source names are display labels, never paths to open. All state is owned by
//! the caller; there is no global source registry or reference-runtime dependency.
//!
//! ```
//! use antimony::{Limits, SourceFile, SourceId};
//!
//! let source = SourceFile::new(SourceId::new(0), "example", "A = 1;", &Limits::default())?;
//! let span = source.span(0, 1)?;
//! assert_eq!(source.slice(span)?, "A");
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
#![forbid(unsafe_code)]

mod builtins;
mod conditional;
pub use conditional::{ComparisonOp, LogicalOp};
mod diagnostic;
mod limits;
mod source;

pub use diagnostic::{Diagnostic, DiagnosticCode, Diagnostics, Label, LabelStyle, Severity};
pub use limits::{LimitExceeded, LimitKind, Limits};
pub use source::{SourceFile, SourceId, SourceLocation, Span, SpanError};

mod format;
mod model;
#[cfg(feature = "sbml")]
mod sbml;
mod semantic;
mod simulation;
mod syntax;

pub use format::format_antimony;
pub use model::{
    Compartment, Event, EventAssignment, EventTrigger, Expression, ExpressionId, ExpressionKind,
    FunctionDefinition, IMPLEMENTED_PROFILE, InitialAssignment, Parameter, PiecewiseBranch,
    Reaction, Rule, RuleKind, Species, SpeciesReference, SymbolKind, ValidatedModel,
};
#[cfg(feature = "sbml")]
pub use sbml::{SbmlDocument, read_sbml, sbml_to_antimony, write_sbml};
pub use simulation::{
    ElasticityMatrix, SimulationConfig, SteadyState, SteadyStateConfig, StoichiometryMatrix,
    Trajectory, simulate, steady_state,
};
pub use syntax::{BinaryOp, ParsedDocument, UnaryOp, load_file, parse_with_imports};

/// Parse the implemented numeric-math flat-model slice with checked resource limits.
///
/// EOF counts toward the token limit. Explicit Pratt tasks avoid call-stack growth.
/// Both logical syntax nesting and resulting tree
/// depth obey the expression ceiling, with an additional hard ceiling of 256.
/// This fail-fast API returns one mandatory diagnostic; it never accumulates an
/// unbounded error report, including when the report budget is zero.
pub fn parse(source: &SourceFile, limits: &Limits) -> Result<ParsedDocument, Diagnostic> {
    syntax::parse_document(source, limits)
}

/// Resolve and check syntax against the implemented numeric-math flat-model subset.
///
/// Absent and symbolic initial values are retained, never replaced by zero.
/// Known initial-expression values must be finite. Structural validity does not
/// establish simulation readiness. Numeric compartment defaults are applied only
/// as documented upstream behavior and reported. Unit checking rejects only provable conflicts;
/// bare arithmetic numbers and symbols without explicit or inherited units never
/// default to dimensionless.
pub fn analyze(document: &ParsedDocument) -> Result<ValidatedModel, Diagnostic> {
    semantic::analyze_document(document)
}

/// Bounded explicit unit metadata and conservative dimensional knowledge.
pub use model::units::{
    DimensionalStatus, Dimensions, ModelUnit, UnitComponent, UnitDefinition, UnitKind, UnitLimits,
};

pub use builtins::{Builtin, NumericConstant};
