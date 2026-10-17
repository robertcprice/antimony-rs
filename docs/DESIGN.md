# Architecture and public API proposal

**Status: target architecture; not an upstream-approved API.** Phase 1 and a
narrow native initialization/units slice plus canonical source layout and ordinary
source-aware symbols, unnamed reactions and expanded arithmetic are implemented
as the foundation of `math-core-v0.2.1-slice`, now extended by typed numeric
calls, constants, time and exact rational atoms; see [current corrective review](reviews/rational-review.md)
and the [optional initial-value API](SYMBOLS_API.md).
The optional unified CLI is also independently verified without changing that
language profile; see its [guide](CLI.md), [plan](CLI_PLAN.md),
[JSON report schema](cli-report.schema.json) and [review](reviews/cli-review.md).
Eight retained and [two new builtin/time analytic models](BUILTINS.md#two-selected-analytic-inputs)
have development-only numerical validation through an isolated RoadRunner environment; see the [numerical guide](NUMERICAL.md)
and [review](reviews/numerical-review.md). This adds no simulator to the native
library or CLI and is not full-profile conformance.
Layout and signatures below remain broader design proposals where they differ
from the current documented crate API.

### Arithmetic source versus semantic representation

Source expressions retain lexemes and grouping for canonical layout. Before
semantic analysis, a bounded non-recursive normalization pass constructs the
reference's arithmetic groups, then lowers them to ordered binary trees. It does
not flatten all sums/products indiscriminately: completed group context matters.
Initialization, dimension analysis, public IR and MathML consume the same final
tree, preventing different arithmetic authorities. This can differ from literal
parenthesis evaluation under floating-point cancellation; compatibility follows
the pinned Antimony representation and is tested explicitly.

Numeric source spelling remains available, while scientific MathML leaves encode
decimal mantissa and exponent without deriving the mantissa by binary division.
The expression value follows the exported mantissa-times-power-of-ten meaning,
including observable rounding. Resource/depth checks apply after normalization
as well as before it. See `src/semantic/expression_normalization.rs` and
`src/syntax/numeric.rs`.

## 1. Design choice

One package initially: working package name `antimony-rs`, suggested Rust library
name `antimony` (both provisional). A thin optional CLI can be a binary in the
same package. This adapter is now implemented behind `cli`; default features remain
`sbml`, and a no-default core build retains zero normal dependencies. Avoid a
many-crate workspace before the boundaries stabilize.
Native parsing/analysis/export has no C/C++ or Python dependency. A separate
development oracle may use the existing implementation.

## 2. Intended full-profile layout — only partially implemented

```text
Cargo.toml                 # publish=false during collaboration groundwork
src/
  lib.rs                   # deliberately small facade
  source.rs                # SourceId, byte spans, line mapping
  diagnostic.rs            # codes, severity, primary/secondary labels
  limits.rs                # input, token, expression-depth and model-size limits
  syntax/
    token.rs
    lexer.rs               # comments, identifiers, numbers, punctuation
    parser.rs              # statement grammar and model wrappers
    expression.rs          # Pratt expression parser, reference-tested precedence
    ast.rs                 # unresolved syntax with original spans
  semantic/
    symbols.rs             # symbol collection/type inference/name resolution
    declarations.rs        # type, initial value, compartment ownership
    expressions.rs         # typed symbol references and dependency checks
    units.rs               # known/unspecified/inconsistent dimensions
    profile.rs             # supported-feature checks, explicit rejection
    validate.rs            # pipeline to produce a ValidatedModel
  model/
    mod.rs                 # read-only model access and typed IDs
    expression.rs          # resolved mathematical expression tree
    species.rs
    reaction.rs
    units.rs
  format/
    antimony.rs            # deterministic canonical formatting, not a lossless editor
  sbml/
    mod.rs                 # v0.1 export only
    writer.rs              # structured XML writer; no string concatenation of XML
    mathml.rs               # expression tree -> Content MathML
    mapping.rs             # explicit L3V2 Core field/flag mapping
  bin/antimony.rs           # implemented optional check/format/export entry point
  bin/antimony_cli/         # args, reports, orchestration and isolated file I/O
tests/
  syntax/ semantic/ export/ regression/
fixtures/
  accepted/ rejected/ upstream/ manifests/
tools/oracle/              # optional Python reference harness, never runtime
profiles/                  # versioned target features and evidence links
docs/
```

## 3. Data flow and responsibilities

**Source -> syntax.** Preserve UTF-8 byte spans and original number lexemes.
Parsing recognizes syntax; it must not assign default kinetics or evaluate a
model. Newline/semicolon and comment behavior are tested against the reference.
Identifiers follow the selected upstream grammar, not arbitrary Unicode guesses.

**Syntax -> semantics.** Collect declarations before resolving references so
ordinary forward references work. Distinguish model symbols, species,
parameters, compartments, and reaction-local scope when later supported.
Resolve initialization/expression dependency cycles before export; cyclic reaction
networks are valid and must not be rejected. Use ordered collections/IDs for
deterministic output, not HashMap iteration order as an output contract.

**Semantics -> validated IR.** Keep species initial amount and concentration as
distinct enum variants; never store both in an ambiguous scalar. Preserve
boundaryCondition and constant independently. Keep rate expressions as trees;
do not infer mass action from a reaction's name, arrow, or parameter names.
Compartment volume factors belong in the supplied model, not invented by export.
IR capacity for a future construct does not imply that its syntax is accepted by
the current profile. Expression-valued initialization may require an SBML
InitialAssignment; it is not an ongoing assignment rule.

**Validated IR -> artifacts.** Export only after all profile and semantic errors
are resolved. A declared unsupported feature is an error, not an ignorable
warning. Emit required SBML attributes, namespace and MathML structure explicitly.
Canonical formatting may remove comments/whitespace; source editing is not a
v0.1 promise. Model metadata that affects identity cannot silently disappear.
The current formatter retains whether a named header uses empty parentheses and
preserves comma-grouped `formula` parameter tokens. Both forms reuse existing
initialization semantics; no parameter keyword, implicit value or species-promotion
rule is introduced. See the [bounded grammar contract](reviews/flat-declarations-review.md#reviewed-boundary).

## 4. Proposed full-profile API (pseudocode, not the current callable API)

```rust
pub fn parse(source: &str, options: &ParseOptions)
    -> Result<ParsedDocument, Diagnostics>;

pub fn analyze(document: &ParsedDocument, profile: &Profile)
    -> Result<ValidatedModel, Diagnostics>;

pub fn format_antimony(document: &ParsedDocument)
    -> Result<String, Diagnostics>;

#[cfg(feature = "sbml")]
pub fn write_sbml(model: &ValidatedModel, options: &SbmlOptions)
    -> Result<String, Diagnostics>;
```

`ParsedDocument` and `ValidatedModel` are opaque owned public wrappers. Internal
AST layout is not a stable public contract in 0.1. Offer read-only iterators and
getters for model entities; avoid public mutable fields that bypass validation.
Diagnostics are structured values, not just terminal strings. Export callers
must be able to obtain warnings and provenance, not merely an XML string; exact
success-report shape is a Phase 1 API decision before stabilization.

`ValidatedModel` means valid **for a named profile**, not biologically accurate,
dimensionally complete, executable by every SBML tool, or clinically validated.
It carries the profile ID, target SBML level/version and warnings.

## 5. Dependency strategy

- Handwritten tokenization + recursive-descent statements + Pratt expressions
  is the initial proposal: small dependency surface and precise diagnostics.
  Validate precedence against upstream; do not invent it from mathematical habit.
- Evaluate a parser combinator only if the first vertical slice demonstrates a
  practical benefit. Winnow is a documented candidate, not an approved dependency.
- Structured XML writing: evaluate `quick-xml::Writer` under the optional `sbml`
  feature. A generic XML writer is not an SBML validator.
- Start core types with std; optional serde later, with a versioned format if
  persisted. The optional CLI now uses exactly pinned `serde_json = 1.0.150` for
  its versioned result report, not persisted model IR. It is absent from core-only
  builds. Native model IR is not a new external scientific standard.
- Pin actual dependency/MSRV choices in Phase 1 after testing. Do not promise
  no_std, WASM, zero-copy parsing, async parsing or a stable ABI in v0.1.
- Disallow unsafe in project native code. Thread independence is a testable
  goal; do not claim Send/Sync merely because the implementation is Rust.

## 6. Diagnostics and untrusted input

Error families: syntax, unresolved symbol, declaration conflict, unsupported
feature, units inconsistency, initialization cycle, resource limit, export.
Implicit parameter inference is legal Antimony. Distinguish an inferred but
uninitialized parameter (a strict-profile restriction) from an invalid reference
or incompatible declaration. Every error has a stable machine-readable code and useful source span; wording
may evolve. For a valid unsupported event, say "events are not in kinetic-core
v0.1", not "invalid Antimony".
This is the target classification contract, not exhaustive current coverage:
bare standalone identifiers are now correctly classified as syntax errors, but
the inherited `import "secret.ant"` lexical case still reports syntax. Known
unsupported-family guards must not be mistaken for a full-language recognizer.

Proposed default limits: 4 MiB source, 1 million tokens, 128 expression nesting,
100,000 model entities, 100 accumulated diagnostics. These are safety defaults,
not performance claims; make them configurable and test boundary behavior.
Imports are rejected, never followed. Native parsing/export performs no implicit
filesystem or network access. Avoid unbounded recursion on adversarial input.

The optional adapter alone opens explicit input/output paths. It bounds bytes
before UTF-8 decoding, keeps human text on stderr, and gives JSON exclusive stdout
by requiring a file destination for JSON format/export. It preserves core warning
and rejection codes instead of inferring them from prose. Shared staged output
checks final-destination policy and commits only after generation succeeds;
post-commit report failures cannot promise rollback. Full details and tested
platform limitations are in [CLI file safety](CLI.md#file-safety-and-limits).

## 7. Separation from consumers

Downstream consumers can consume the checked model/expressions through an explicit adapter in
its own repository. libRoadRunner may be used as a development comparator.
Neither is a dependency of this crate. Exporting an SBML model must not establish
a second authoritative copy of a consumer's biological state.

## 8. Deferred, not forgotten

SBML import, broad Antimony modules/composition, dynamic rules/events, lossless
formatting, LSP integration and incremental parsing need their own profiles and
acceptance tests. The data model should not block them, but v0.1 will not pretend
to implement them.
