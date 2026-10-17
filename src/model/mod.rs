//! Read-only, privately constructed model for the implemented units slice.
pub(crate) mod units;
use crate::syntax::{BinaryOp, UnaryOp};
use crate::{Diagnostic, SourceFile, Span};
use std::collections::BTreeMap;
use units::{DimensionalStatus, ModelUnit, UnitDefinition};

/// Name of the actual implemented subset, not the full target release profile.
pub const IMPLEMENTED_PROFILE: &str = "modular-core-v0.8-slice";

/// Model checked for the implemented units flat-model subset only.
///
/// This is not a biological, dimensional, clinical, or simulation certificate.
/// There is no public constructor or mutable access that bypasses analysis.
#[derive(Clone, Debug)]
pub struct ValidatedModel {
    pub(crate) source: SourceFile,
    pub(crate) id: String,
    pub(crate) function_definitions: Vec<FunctionDefinition>,
    pub(crate) compartments: Vec<Compartment>,
    pub(crate) species: Vec<Species>,
    pub(crate) parameters: Vec<Parameter>,
    pub(crate) reactions: Vec<Reaction>,
    pub(crate) events: Vec<Event>,
    pub(crate) expressions: Vec<Expression>,
    pub(crate) initial_assignments: Vec<InitialAssignment>,
    pub(crate) rules: Vec<Rule>,
    pub(crate) warnings: Vec<Diagnostic>,
    pub(crate) unit_definitions: Vec<UnitDefinition>,
    pub(crate) unit_annotations: BTreeMap<String, String>,
    pub(crate) model_units: BTreeMap<ModelUnit, String>,
    pub(crate) species_substance_units: BTreeMap<String, String>,
    pub(crate) symbol_dimensions: BTreeMap<String, DimensionalStatus>,
    pub(crate) expression_dimensions: Vec<DimensionalStatus>,
}
impl ValidatedModel {
    /// Selected implemented subset identifier.
    pub fn profile_id(&self) -> &'static str {
        IMPLEMENTED_PROFILE
    }
    /// Target SBML level and version.
    pub fn sbml_target(&self) -> (u32, u32) {
        (3, 2)
    }
    /// Original source retained for provenance and source labels.
    pub fn source(&self) -> &SourceFile {
        &self.source
    }
    /// Model identifier.
    pub fn id(&self) -> &str {
        &self.id
    }
    /// User-defined functions in top-level source declaration order.
    pub fn function_definitions(&self) -> &[FunctionDefinition] {
        &self.function_definitions
    }
    /// Compartments in deterministic ID order.
    pub fn compartments(&self) -> &[Compartment] {
        &self.compartments
    }
    /// Species in deterministic ID order.
    pub fn species(&self) -> &[Species] {
        &self.species
    }
    /// Parameters in deterministic ID order.
    pub fn parameters(&self) -> &[Parameter] {
        &self.parameters
    }
    /// Reactions in deterministic ID order.
    pub fn reactions(&self) -> &[Reaction] {
        &self.reactions
    }
    /// Discrete events in source declaration order.
    pub fn events(&self) -> &[Event] {
        &self.events
    }
    /// Preserved expression initializations in upstream type/appearance order.
    /// These set initialization-time values only, never ongoing assignment rules.
    pub fn initial_assignments(&self) -> &[InitialAssignment] {
        &self.initial_assignments
    }
    /// Ongoing assignment (`:=`) and rate (`'=`) rules in upstream type/appearance order.
    pub fn rules(&self) -> &[Rule] {
        &self.rules
    }
    /// Expression node by model-scoped ID. IDs must not be mixed across models.
    pub fn expression(&self, id: ExpressionId) -> Option<&Expression> {
        self.expressions.get(id.0)
    }
    /// Explicit custom units; annotations never change numeric model values.
    pub fn unit_definitions(&self) -> &[UnitDefinition] {
        &self.unit_definitions
    }
    /// Explicit canonical source annotation. Species annotations denote concentration,
    /// not SBML substance units; use [`Self::species_substance_units`] for the latter.
    pub fn units(&self, symbol: &str) -> Option<&str> {
        self.unit_annotations.get(symbol).map(String::as_str)
    }
    /// Explicit model-wide default; no absent default is invented.
    pub fn model_units(&self, default: ModelUnit) -> Option<&str> {
        self.model_units.get(&default).map(String::as_str)
    }
    /// Exported species substance reference after concentration/compartment mapping.
    /// None means no explicit species substance attribute, not dimensionless.
    pub fn species_substance_units(&self, species: &str) -> Option<&str> {
        self.species_substance_units
            .get(species)
            .map(String::as_str)
    }
    /// Dimensional knowledge for an existing model symbol. None means unknown symbol.
    pub fn symbol_dimensions(&self, symbol: &str) -> Option<DimensionalStatus> {
        self.symbol_dimensions.get(symbol).copied()
    }
    /// Conservative dimensions of a numeric node; bare numbers are unspecified.
    pub fn expression_dimensions(&self, id: ExpressionId) -> Option<DimensionalStatus> {
        self.expression_dimensions.get(id.0).copied()
    }
    /// Non-fatal notices, including unspecified units and applied upstream defaults.
    pub fn warnings(&self) -> &[Diagnostic] {
        &self.warnings
    }
}
/// A three-dimensional compartment with optional positive scalar or symbolic size.
#[derive(Clone, Debug)]
pub struct Compartment {
    pub(crate) id: String,
    pub(crate) size: Option<f64>,
    pub(crate) initial_expression: Option<ExpressionId>,
    pub(crate) constant: bool,
    pub(crate) span: Span,
    pub(crate) implicit: bool,
}
impl Compartment {
    /// First declaration/inference occurrence in the retained source (not all references).
    pub fn source_span(&self) -> Span {
        self.span
    }

    /// SBML identifier.
    pub fn id(&self) -> &str {
        &self.id
    }
    /// Supplied positive scalar/evaluated size, or None when unset or non-scalar rule.
    pub fn size(&self) -> Option<f64> {
        self.size
    }
    /// Original initialization expression, when initialized by a symbolic expression.
    pub fn initial_expression(&self) -> Option<ExpressionId> {
        self.initial_expression
    }
    /// Declared mutability; defaults to constant unless `var`, ruled, or event-assigned.
    pub fn constant(&self) -> bool {
        self.constant
    }
    /// Whether the upstream volume-one default compartment was synthesized.
    pub fn is_implicit(&self) -> bool {
        self.implicit
    }
}
/// A species with optional initialization-time concentration or substance amount.
#[derive(Clone, Debug)]
pub struct Species {
    pub(crate) id: String,
    pub(crate) compartment: String,
    pub(crate) concentration: Option<f64>,
    pub(crate) initial_amount: Option<f64>,
    pub(crate) has_only_substance_units: bool,
    pub(crate) span: Span,
    pub(crate) initial_expression: Option<ExpressionId>,
    pub(crate) boundary: bool,
}
impl Species {
    /// First declaration/inference occurrence in the retained source (not all references).
    pub fn source_span(&self) -> Span {
        self.span
    }

    /// SBML identifier.
    pub fn id(&self) -> &str {
        &self.id
    }
    /// Owning compartment identifier.
    pub fn compartment(&self) -> &str {
        &self.compartment
    }
    /// Evaluated initialization-time concentration (when `has_only_substance_units` is false).
    pub fn initial_concentration(&self) -> Option<f64> {
        self.concentration
    }
    /// Evaluated initialization-time substance amount (when `has_only_substance_units` is true).
    pub fn initial_amount(&self) -> Option<f64> {
        self.initial_amount
    }
    /// True when declared with `substanceOnly species`, setting `hasOnlySubstanceUnits="true"`.
    pub fn has_only_substance_units(&self) -> bool {
        self.has_only_substance_units
    }
    /// Original initialization expression, when not initialized by a scalar literal.
    pub fn initial_expression(&self) -> Option<ExpressionId> {
        self.initial_expression
    }
    /// Boundary-condition flag, independent of SBML constant (always false here).
    pub fn boundary_condition(&self) -> bool {
        self.boundary
    }
    /// SBML constant flag. `const species` means boundary, not this flag.
    pub fn constant(&self) -> bool {
        false
    }
}
/// A parameter with optional scalar/symbolic initialization; not necessarily constant.
#[derive(Clone, Debug)]
pub struct Parameter {
    pub(crate) id: String,
    pub(crate) value: Option<f64>,
    pub(crate) span: Span,
    pub(crate) initial_expression: Option<ExpressionId>,
    pub(crate) constant: bool,
}
impl Parameter {
    /// First declaration/inference occurrence in the retained source (not all references).
    pub fn source_span(&self) -> Span {
        self.span
    }

    /// SBML identifier.
    pub fn id(&self) -> &str {
        &self.id
    }
    /// Evaluated initialization-time value, or None when unset or symbolic.
    /// Check initial_expression to distinguish symbolic initialization from absence.
    pub fn value(&self) -> Option<f64> {
        self.value
    }
}
impl Parameter {
    /// Original initialization expression, if supplied instead of a scalar literal.
    pub fn initial_expression(&self) -> Option<ExpressionId> {
        self.initial_expression
    }
    /// Upstream SBML constant flag; false for dependencies on non-boundary species.
    pub fn constant(&self) -> bool {
        self.constant
    }
}
/// Preserved initialization-time expression, distinct from an assignment rule.
#[derive(Clone, Debug)]
pub struct InitialAssignment {
    pub(crate) symbol: String,
    pub(crate) expression: ExpressionId,
}
impl InitialAssignment {
    /// Initialized symbol identifier.
    pub fn symbol(&self) -> &str {
        &self.symbol
    }
    /// Model-scoped expression root, not a constant-folded replacement.
    pub fn expression(&self) -> ExpressionId {
        self.expression
    }
}
/// Kind of ongoing mathematical rule in the flat rules slice.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuleKind {
    /// Algebraic assignment rule (`x := expr`), active for all `t >= 0`.
    Assignment,
    /// Ordinary differential equation rate rule (`x' = expr`), defining `dx/dt`.
    Rate,
}
/// Preserved ongoing assignment or rate rule targeting a species, compartment, or parameter.
#[derive(Clone, Debug)]
pub struct Rule {
    pub(crate) kind: RuleKind,
    pub(crate) target: String,
    pub(crate) expression: ExpressionId,
    pub(crate) span: Span,
}
impl Rule {
    /// Rule kind (`Assignment` for `:=`, `Rate` for `'=`).
    pub fn kind(&self) -> RuleKind {
        self.kind
    }
    /// Target species, compartment, or parameter identifier.
    pub fn target(&self) -> &str {
        &self.target
    }
    /// Model-scoped root expression ID.
    pub fn expression(&self) -> ExpressionId {
        self.expression
    }
    /// Source span of the authoritative rule statement.
    pub fn source_span(&self) -> Span {
        self.span
    }
}
/// Trigger specification for a discrete SBML [`Event`].
#[derive(Clone, Debug)]
pub struct EventTrigger {
    pub(crate) expression: ExpressionId,
    pub(crate) persistent: bool,
    pub(crate) initial_value: bool,
}
impl EventTrigger {
    /// Trigger condition expression (`<trigger><math>...</math></trigger>`).
    pub fn expression(&self) -> ExpressionId {
        self.expression
    }
    /// SBML `persistent` attribute on `<trigger>` (defaults to `true`).
    pub fn persistent(&self) -> bool {
        self.persistent
    }
    /// SBML `initialValue` (`t0` in Antimony) attribute on `<trigger>` (defaults to `true`).
    pub fn initial_value(&self) -> bool {
        self.initial_value
    }
}
/// One state assignment (`variable = expression`) executed when an [`Event`] fires.
#[derive(Clone, Debug)]
pub struct EventAssignment {
    pub(crate) variable: String,
    pub(crate) expression: ExpressionId,
    pub(crate) span: Span,
}
impl EventAssignment {
    /// Target species, compartment, or parameter identifier.
    pub fn variable(&self) -> &str {
        self.variable.as_str()
    }
    /// Assigned mathematical expression (`<eventAssignment><math>...</math></eventAssignment>`).
    pub fn expression(&self) -> ExpressionId {
        self.expression
    }
    /// Source span of the `variable = expression` clause.
    pub fn source_span(&self) -> Span {
        self.span
    }
}
/// Discrete event (`at ...: var = expr, ...`) with optional delay and priority.
#[derive(Clone, Debug)]
pub struct Event {
    pub(crate) id: String,
    pub(crate) use_values_from_trigger_time: bool,
    pub(crate) trigger: EventTrigger,
    pub(crate) delay: Option<ExpressionId>,
    pub(crate) priority: Option<ExpressionId>,
    pub(crate) assignments: Vec<EventAssignment>,
    pub(crate) span: Span,
}
impl Event {
    /// Explicit or generated (`_E0`, `_E1`, ...) SBML event identifier.
    pub fn id(&self) -> &str {
        &self.id
    }
    /// SBML `useValuesFromTriggerTime` (`fromTrigger` in Antimony) attribute (defaults to `true`).
    pub fn use_values_from_trigger_time(&self) -> bool {
        self.use_values_from_trigger_time
    }
    /// Event trigger condition and persistence/t0 metadata.
    pub fn trigger(&self) -> &EventTrigger {
        &self.trigger
    }
    /// Optional delay expression (`at delay after trigger:`).
    pub fn delay(&self) -> Option<ExpressionId> {
        self.delay
    }
    /// Optional priority expression (`priority = expr`).
    pub fn priority(&self) -> Option<ExpressionId> {
        self.priority
    }
    /// Ordered event assignments executed when the event fires.
    pub fn assignments(&self) -> &[EventAssignment] {
        &self.assignments
    }
    /// Source span of the authoritative event statement.
    pub fn source_span(&self) -> Span {
        self.span
    }
}
/// Fixed positive reaction stoichiometry.
#[derive(Clone, Debug)]
pub struct SpeciesReference {
    pub(crate) species: String,
    pub(crate) stoichiometry: f64,
}
impl SpeciesReference {
    /// Participating species identifier.
    pub fn species(&self) -> &str {
        &self.species
    }
    /// Finite positive coefficient.
    pub fn stoichiometry(&self) -> f64 {
        self.stoichiometry
    }
}
/// A reaction with an explicit/generated ID and its supplied unevaluated rate.
#[derive(Clone, Debug)]
pub struct Reaction {
    pub(crate) span: Span,
    pub(crate) id: String,
    pub(crate) reversible: bool,
    pub(crate) reactants: Vec<SpeciesReference>,
    pub(crate) products: Vec<SpeciesReference>,
    pub(crate) modifiers: Vec<String>,
    pub(crate) rate: ExpressionId,
}
impl Reaction {
    /// First declaration/inference occurrence in the retained source (not all references).
    pub fn source_span(&self) -> Span {
        self.span
    }

    /// SBML identifier.
    pub fn id(&self) -> &str {
        &self.id
    }
    /// True for `->`, false for `=>`; no reverse rate is invented.
    pub fn reversible(&self) -> bool {
        self.reversible
    }
    /// Reactant references.
    pub fn reactants(&self) -> &[SpeciesReference] {
        &self.reactants
    }
    /// Product references.
    pub fn products(&self) -> &[SpeciesReference] {
        &self.products
    }
    /// Species in the rate but not either participant list.
    pub fn modifiers(&self) -> &[String] {
        &self.modifiers
    }
    /// Model-scoped root expression ID.
    pub fn rate(&self) -> ExpressionId {
        self.rate
    }
}
/// A top-level user-defined function (`function name(params) body end`).
#[derive(Clone, Debug)]
pub struct FunctionDefinition {
    pub(crate) id: String,
    pub(crate) parameters: Vec<String>,
    pub(crate) body: ExpressionId,
    pub(crate) span: Span,
}
impl FunctionDefinition {
    /// SBML function identifier.
    pub fn id(&self) -> &str {
        &self.id
    }
    /// Formal parameter identifiers in declaration order.
    pub fn parameters(&self) -> &[String] {
        &self.parameters
    }
    /// Model-scoped root expression ID of the normalized function body.
    pub fn body(&self) -> ExpressionId {
        self.body
    }
    /// Source span of the complete `function ... end` definition.
    pub fn source_span(&self) -> Span {
        self.span
    }
}
/// Model-scoped expression-node identity; not a global ID.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct ExpressionId(pub(crate) usize);
/// Resolved class of an expression symbol.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SymbolKind {
    /// Species concentration.
    Species,
    /// Scalar parameter, whose SBML constant flag is available on the parameter.
    Parameter,
    /// Fixed compartment size.
    Compartment,
    /// Formal parameter (`<bvar>`) bound inside a [`FunctionDefinition`] body.
    FunctionParameter,
}
/// Read-only node contents in the model's expression arena.
#[derive(Clone, Debug)]
pub enum ExpressionKind {
    /// A Boolean constant, preserved as Boolean MathML rather than a numeric leaf.
    Boolean(bool),
    /// Logical negation with finite context conversion.
    Not {
        /// Model-scoped operand.
        child: ExpressionId,
    },
    /// Ordered numeric-context comparison arguments.
    Compare {
        /// Relational operation.
        op: crate::ComparisonOp,
        /// At least two arguments; NotEqual has exactly two.
        args: Vec<ExpressionId>,
    },
    /// Ordered logical arguments; empty calls retain their identities.
    Logical {
        /// Logical operation.
        op: crate::LogicalOp,
        /// Model-scoped operands.
        args: Vec<ExpressionId>,
    },
    /// First-true ordered alternatives, with explicitly optional fallback.
    Piecewise {
        /// Ordered value/condition alternatives.
        branches: Vec<PiecewiseBranch>,
        /// Fallback, absent when omitted in source.
        otherwise: Option<ExpressionId>,
    },
    /// Exact unreduced rational atom from parenthesized integer notation.
    /// Components fit signed 32-bit; the denominator is nonzero. Wider fields
    /// avoid overflow during internal unary normalization, not broadened ranges.
    Rational {
        /// Signed numerator, retained without cancelling common factors.
        numerator: i64,
        /// Signed denominator, retained without sign normalization.
        denominator: i64,
    },
    /// A verified numeric builtin with ordered, model-owned argument IDs.
    Call {
        /// Resolved numeric builtin, never a raw host-language function.
        function: crate::Builtin,
        /// Argument IDs in source order, owned by this model arena.
        args: Vec<ExpressionId>,
    },
    /// A call to a previously declared [`FunctionDefinition`] with ordered argument IDs.
    UserCall {
        /// Resolved user-defined function identifier.
        name: String,
        /// Argument IDs in source order, owned by this model arena.
        args: Vec<ExpressionId>,
    },
    /// SBML time symbol. Initialization stays symbolic; no zero is invented.
    Time,
    /// A mathematical constant with explicit MathML identity.
    Constant(crate::NumericConstant),
    /// Finite numeric literal and its exported arithmetic value.
    Number {
        /// Source lexeme without a unary sign for parsed expressions.
        /// Scalar reaction-law reassignment synthesizes an equivalent canonical
        /// decimal spelling from its separately validated scalar value.
        lexeme: String,
        /// Finite MathML numeric value; scientific notation uses mantissa × 10^exponent,
        /// which can differ one binary64 ULP from parsing the whole decimal string.
        value: f64,
    },
    /// A resolved symbol.
    Symbol {
        /// Symbol identifier.
        name: String,
        /// Validated symbol class.
        kind: SymbolKind,
    },
    /// Unary operation.
    Unary {
        /// Operator.
        op: UnaryOp,
        /// Operand ID.
        child: ExpressionId,
    },
    /// Binary operation in reference-normalized grouping, with operand order preserved.
    Binary {
        /// Operator.
        op: BinaryOp,
        /// Left operand ID.
        left: ExpressionId,
        /// Right operand ID.
        right: ExpressionId,
    },
}
/// Located numeric expression node. Child IDs refer to the same model arena.
#[derive(Clone, Debug)]
pub struct Expression {
    pub(crate) kind: ExpressionKind,
    pub(crate) span: Span,
}
impl Expression {
    /// Node's immutable operation and operands.
    pub fn kind(&self) -> &ExpressionKind {
        &self.kind
    }
    /// Owning source range (a normalized operator may cover its whole source group).
    /// A numeric/unary node synthesized from scalar reaction-law reassignment
    /// uses that assignment target's source range, not the literal's range.
    pub fn span(&self) -> Span {
        self.span
    }
}

/// One owned piecewise alternative; IDs refer to the containing model's arena.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PiecewiseBranch {
    /// Value selected when this condition is true and earlier conditions are false.
    pub value: ExpressionId,
    /// Boolean-context condition; unknown initialization is not false.
    pub condition: ExpressionId,
}
