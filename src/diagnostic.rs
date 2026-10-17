//! Structured diagnostics with no printing or other output side effects.
use crate::{LimitExceeded, LimitKind, Limits, Span};

/// Diagnostic importance, independent of its code and source labels.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
#[non_exhaustive]
pub enum Severity {
    /// Compilation cannot succeed.
    Error,
    /// Non-fatal issue that must remain visible to the caller.
    Warning,
    /// Informational context.
    Note,
}

/// Stable machine-readable diagnostic families; human wording may evolve.
///
/// Reserving a code does not claim that its language feature is implemented.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
#[non_exhaustive]
pub enum DiagnosticCode {
    /// Malformed syntax.
    Syntax,
    /// Unresolvable reference.
    UnresolvedSymbol,
    /// Incompatible declarations.
    DeclarationConflict,
    /// Valid language outside the selected profile.
    UnsupportedFeature,
    /// Provable dimensional inconsistency.
    UnitsInconsistency,
    /// Cyclic initial-value dependencies.
    InitializationCycle,
    /// Resource ceiling reached.
    ResourceLimit,
    /// Artifact export failed.
    Export,
    /// Units are unspecified, not proved inconsistent.
    UnspecifiedUnits,
    /// An explicitly reported upstream default compartment was applied.
    DefaultCompartment,
}
impl DiagnosticCode {
    /// Stable code, independent of severity and human wording.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Syntax => "syntax",
            Self::UnresolvedSymbol => "unresolved_symbol",
            Self::DeclarationConflict => "declaration_conflict",
            Self::UnsupportedFeature => "unsupported_feature",
            Self::UnitsInconsistency => "units_inconsistency",
            Self::InitializationCycle => "initialization_cycle",
            Self::ResourceLimit => "resource_limit",
            Self::Export => "export",
            Self::UnspecifiedUnits => "unspecified_units",
            Self::DefaultCompartment => "default_compartment",
        }
    }
}

/// Role of a source label within a diagnostic.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum LabelStyle {
    /// Main source of the issue.
    Primary,
    /// Related location (for example the previous declaration).
    Secondary,
}
/// A checked source span and an explanatory message.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Label {
    style: LabelStyle,
    span: Span,
    message: String,
}
impl Label {
    /// Construct a primary location.
    pub fn primary(span: Span, message: impl Into<String>) -> Self {
        Self {
            style: LabelStyle::Primary,
            span,
            message: message.into(),
        }
    }
    /// Construct a related location.
    pub fn secondary(span: Span, message: impl Into<String>) -> Self {
        Self {
            style: LabelStyle::Secondary,
            span,
            message: message.into(),
        }
    }
    /// Label's presentation role.
    pub fn style(&self) -> LabelStyle {
        self.style
    }
    /// Checked byte span; resolve its ID using the compilation's sources.
    pub fn span(&self) -> Span {
        self.span
    }
    /// Human-readable explanatory message.
    pub fn message(&self) -> &str {
        &self.message
    }
}

/// Owned, renderer-independent diagnostic. No source is opened or printed.
///
/// A diagnostic may have no labels for issues such as source-size rejection.
/// Diagnostic message/label sizes are caller-managed; [`Diagnostics`] bounds only
/// the number of accumulated diagnostics.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Diagnostic {
    severity: Severity,
    code: DiagnosticCode,
    message: String,
    labels: Vec<Label>,
    notes: Vec<String>,
}
impl Diagnostic {
    /// Construct a diagnostic, initially without labels or notes.
    pub fn new(severity: Severity, code: DiagnosticCode, message: impl Into<String>) -> Self {
        Self {
            severity,
            code,
            message: message.into(),
            labels: Vec::new(),
            notes: Vec::new(),
        }
    }
    /// Attach a source label, preserving insertion order.
    #[must_use]
    pub fn with_label(mut self, label: Label) -> Self {
        self.labels.push(label);
        self
    }
    /// Attach explanatory context, preserving insertion order.
    #[must_use]
    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.notes.push(note.into());
        self
    }
    /// Importance of this issue.
    pub fn severity(&self) -> Severity {
        self.severity
    }
    /// Machine-readable family.
    pub fn code(&self) -> DiagnosticCode {
        self.code
    }
    /// Human-readable description.
    pub fn message(&self) -> &str {
        &self.message
    }
    /// Source labels in insertion order.
    pub fn labels(&self) -> &[Label] {
        &self.labels
    }
    /// Additional context in insertion order.
    pub fn notes(&self) -> &[String] {
        &self.notes
    }
}

/// Bounded, ordered report. A rejected push returns an error and leaves the
/// report unchanged; callers must handle that error rather than silently discard it.
#[derive(Clone, Debug)]
pub struct Diagnostics {
    entries: Vec<Diagnostic>,
    limits: Limits,
}
impl Diagnostics {
    /// Start an empty report using the supplied diagnostic ceiling.
    pub fn new(limits: &Limits) -> Self {
        Self {
            entries: Vec::new(),
            limits: limits.clone(),
        }
    }
    /// Append if the ceiling permits; never silently drop a diagnostic.
    pub fn push(&mut self, diagnostic: Diagnostic) -> Result<(), LimitExceeded> {
        self.limits
            .checked_add(LimitKind::Diagnostics, self.entries.len(), 1)?;
        self.entries.push(diagnostic);
        Ok(())
    }
    /// Read the ordered report.
    pub fn iter(&self) -> std::slice::Iter<'_, Diagnostic> {
        self.entries.iter()
    }
    /// Number of accumulated diagnostics.
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    /// Whether the report has no diagnostics.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
    /// Whether any accepted entry has error severity.
    pub fn has_errors(&self) -> bool {
        self.entries.iter().any(|d| d.severity == Severity::Error)
    }
}
