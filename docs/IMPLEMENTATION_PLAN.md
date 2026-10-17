# Implementation plan — small native compiler, explicit compatibility

Status: Phase 1, a narrow Phase 2 slice, and bounded arithmetic-initialization/units
portions of Phase 3 are independently verified. The current subset is
`conditional-core-v0.3-slice`, a bounded extension rather than conformance to the frozen
target (which excludes functions); see the [conditional review](reviews/conditionals-review.md). Broader grammar, units and
remaining phases are target scope, not support claims. Evidence:
[Phase 1](reviews/phase1-review.md), [Phase 2](reviews/phase2-review.md),
[initialization review](reviews/phase3-initialization-review.md), and
[bounded units review](reviews/phase3-units-review.md), and
[species/default units review](reviews/species-default-units-review.md), and
[canonical layout review](reviews/canonical-format-review.md), and
[flat-declarations review](reviews/flat-declarations-review.md), and
[current optional CLI review](reviews/cli-review.md), and
[selected numerical review](reviews/numerical-review.md), and
[bounded workload review](reviews/workloads-review.md).
References S1–S13 are
defined in [SOURCES.md](SOURCES.md). Read the pinned material, not just this summary.

## Delivered arithmetic and later corrective increments

The [flexible-modeling plan](FLEXIBLE_MODELING_PLAN.md) Phase 1 expands ordinary
arithmetic, unary forms and scientific literals. Its separate exact structural
gate originally had 54 cases (47 positive, 7 required rejections); the explicit
current builtin overlay yields 48 positive and six required rejections; the comparator
remains strict and numerical trajectories remain NOT_RUN. See the
[arithmetic review](reviews/arithmetic-review.md) for executed results. Flexible-plan
Phase 2 is delivered within the [symbol boundary](SYMBOLS.md); Phase 3A delivers
[numeric builtins/constants/time](BUILTINS.md), and Phase 3A.1 delivers the
[verified rational correction](RATIONAL_CALLS_EVIDENCE.md). Phase 3B delivers
[bounded Boolean/logical/comparison/piecewise expressions](CONDITIONALS.md), not
all conditional syntax. Rules and user-defined functions remain planned. The frozen target feature IDs and original 90 fixtures are unchanged.
The earlier source receipt is historical and does not fingerprint this edited source.
Its runner hardcodes older outcome inventories and cannot certify the new profile;
use the explicitly versioned current gates in the conditional evidence guide.

## Historical pre-arithmetic increment and evidence

The optional unified CLI is independently verified over the unchanged
`flat-declarations-v0.1-slice`. All-feature native gates pass **13 binary unit +
244 integration + 2 doctests**; default without CLI **212 + 2**; no-default core
**205 + 2**; CLI without SBML **13 + 237 + 2**. Structural/CLI Python gates pass **147 tests**.
The **90 reference cases** still comprise **48 exact exports + 42 required
rejections**, with **48 exact/idempotent native/reference formatting roundtrips +
42 formatter rejections**. The CLI independently passes all 90 through check,
format and export, with exact existing-adapter artifacts and warning preservation.

The CLI wraps existing APIs; it is not a language-profile expansion. See the
[decision/execution plan](CLI_PLAN.md), [user guide](CLI.md),
[report schema](cli-report.schema.json) and [review](reviews/cli-review.md).
The prior flat-declaration increment remains recorded in its own historical review;
the frozen 26-feature target profile is unchanged.

Optional empty model-header parentheses and comma-separated `formula` parameter
lists reuse existing scalar/arithmetic initialization semantics. A newline/semicolon
after the header remains required; `parameter` is not a declaration keyword.
`const`/`var`/`$` formula qualifiers and nonempty ports stay deferred. Formula symbols
later promoted to species retain strict `declaration_conflict` rejection.
Bare standalone identifiers now report syntax, but universal diagnostic correctness
is not claimed: `import "secret.ant"` remains an inherited lexical classification gap.
See the [reviewed boundary](reviews/flat-declarations-review.md#reviewed-boundary).

**Selected analytic anchors are now verified:** 69 separate numerical Python tests
and 8/8 original models pass. Native and reference exports each match independent
closed-form expectations, explicit amount/concentration observations and invariants
at 101 time points. Seven live negative controls confirm selected failure sensitivity.
The original 90-case corpus keeps its numerical NOT_RUN records; this does not
complete Phase 5 or establish full-profile conformance. See the
[numerical plan](NUMERICAL_PLAN.md), [guide](NUMERICAL.md),
[model derivations](NUMERICAL_MODELS.md), [environment](NUMERICAL_ENVIRONMENT.md)
and [independent review](reviews/numerical-review.md).

**Bounded workload/source evidence is delivered:** 6 original structural motifs,
64 supported generated cases and 16 explicit negatives pass independently specified
contracts, while retaining numerical NOT_RUN. The 39-command local source receipt
passed all four native configurations and 147/69/51/31 separate Python suites,
then handed exact Cargo artifacts to the 90/8/86 evidence gates. This observed cached
build is not a hermetic or bit-reproducible-build claim. See the
[execution plan](RELIABILITY_PLAN.md), [workload review](reviews/workloads-review.md)
and [source-evidence boundary](SOURCE_EVIDENCE.md).

Earlier next-work ranking (superseded by the user-approved
[flexible-modeling sequence](FLEXIBLE_MODELING_PLAN.md)):

1. **Extend reviewed workloads:** the separate six original motifs and 80-case
   finite grid now pass structural checks with 51 workload Python tests. They are
   not published real models, fuzz coverage or numerical tests. Sound minimization
   remains planned. See the [workload guide](WORKLOADS.md) and
   [source-receipt guide](SOURCE_EVIDENCE.md). Next nominate provenance-eligible
   external models and expand independently specified dimensions; coverage-guided
   fuzzing and remote CI require separate work/authorization.
2. **Separate grammar increments:** unnamed reactions require generated-ID,
   collision and ordering evidence; broader functions/rules/units are not implied.

### Historical A+B checkpoint

Species/default units (A) passed 144 native integration + 1 doctest, 87 Python and
71 reference cases. Canonical layout (B) passed 185 native integration + 2 doctests,
110 Python and 73 reference cases, with 40 exact/idempotent native/reference
roundtrips + 33 formatter rejections. Their reviews remain historical evidence;
the current flat-declaration increment extends that accepted grammar conservatively.
No publication, remote, licensing or external work is implied.

## Phase 0 — specification and evidence baseline

**Effort:** 1–2 days including review. **Current:** research baseline established;
Antimony 3.2.0 is a pinned executable local oracle, pending maintainer agreement.
Wheel-to-source reproducibility remains unverified.

Implement/document: pin upstream, map grammar/defaults/units, freeze profile,
list fixture sources, choose native runtime independence and public boundaries.
Turn upstream inconsistencies into named questions, not guessed implementations.

References: S1–S8; `PROFILE_V0_1.md`, `DESIGN.md`.

Verify: profile includes both inclusions and exclusions; every phase has evidence
requirements; no current-support claim; no external simulator dependency.

Guard: do not copy a whole parser or adopt a project license without reviewing
provenance. Do not assume Bison operator declarations establish expression semantics.

## Phase 1 — buildable shell and executable reference contract

**Effort:** 2–3 days. **Depends on:** Phase 0. **Current:** independently verified
for the bounded foundation/oracle gate; see [review](reviews/phase1-review.md).

Implement:
- Initialize one Rust package with `publish=false`, explicit toolchain/MSRV choice,
  no unsafe and minimal dependency budget. Add source IDs, checked spans, structured
  diagnostics and limits. No pretend parse function returning empty success.
- Add an isolated reference conversion script using S4/S8's documented APIs,
  confirm actual Python signatures/version and record source/package mapping.
- Create original small fixtures for reaction arrows, default/non-unit compartment,
  boundary species, missing values and arithmetic precedence. Inspect output manually.
- Write compatibility record schema and freeze initial feature IDs.

References: S4, S7, S8, S10; `VALIDATION.md` sections 2–4.

Verify: cargo check/build/test actually execute; span and diagnostic tests pass;
reference conversion accepts valid input, rejects malformed input, and records
versions; no native parser support claimed. Zero is a valid load result.

Guard: no system-Python installation, hidden network calls, live-develop oracle,
empty/stub golden files or upstream-reference success labeled native success.

## Phase 2 — first full vertical slice, then grammar coverage

**Effort:** 4–6 days. **Depends on:** Phase 1. **Current:** narrow numeric flat-model
slice independently verified (55 native integration + 1 doctest; 8 exact native
exports + 5 expected rejections). Broader grammar remains incomplete. No full
release-profile claim; see [exact restrictions](reviews/phase2-review.md#actual-acceptance-boundary).

Implement:
- Lexer and a narrow parser for one irreversible first-order reaction with explicit
  state. Carry spans end-to-end. Add minimal semantic resolution and SBML output
  needed for this example, then independently validate it.
- Prove this path before generalizing parser abstractions. Expand to flat wrappers,
  declarations, source/sink reactions, numeric stoichiometry and arithmetic.
- Implement reference-tested precedence/association in a Pratt expression parser.
- Detect excluded syntax with explicit diagnostics; no regex-only reaction rewriting.

References: S1 "Introduction and Basics", S2 formula/lexer sections, S3, S7.

Verify: example becomes valid independently checked SBML; reference projection
matches; intentional event input fails cleanly; token and expression tests cover
comments, scientific notation, UTF-8 offsets, malformed input and source exhaustion.

Guard: no guessed coefficients; no `->` interpreted as irreversible; no hardcoded
XML template that succeeds only for the chosen example.

**Checkpoint to show Herbert:** one source file, readable model inspection,
native export versus reference comparison, one clear unsupported-feature error,
and a short limitations statement. Not a full-language compatibility announcement.

## Phase 3 — reliable semantic model

**Effort:** 5–8 days. **Depends on:** Phase 2. **Current:** bounded species/parameter
arithmetic initialization, forward dependencies, cycle/domain guards and original
SBML InitialAssignments plus bounded parameter/compartment unit definitions and
annotations, species concentration/substance mapping and six model defaults
independently verified. The later [numeric builtin/time increment](BUILTINS.md)
also adds typed calls, constants and symbolic time. This does **not** complete Phase 3: general unit algebra,
amount initialization and compartment expressions remain deferred.
See [reviewed subset](reviews/species-default-units-review.md).

Implement:
- Two-pass symbol collection/resolution; verified implicit inference and defaults.
- Species/parameter/compartment identities and declaration provenance.
- Separate amount/concentration/unset values; correct boundary/constant mappings.
- Initial-expression dependency graph and cycle handling; preserve initialization
  expressions rather than converting them to ongoing rules.
- Precisely scoped units, missing-unit warnings and provable mismatch diagnostics.
- Profile validator and private construction of `ValidatedModel`.

References: S1 Model Elements/Assignments/Units, S3 mapping, S5 Units/Errors tests.

Verify: declaration-order equivalence; fixed non-unit compartment cases;
concentration/amount distinctions; boundary flags; unit and initialization negatives;
all excluded feature families fail without producing a valid artifact.

Guard: implicit legal symbols are not automatically invalid; reject unresolved
initial values as a documented strict-profile restriction. No unknown-to-zero
coercion or blanket dimensional-consistency claim.

## Phase 4 — complete profile serialization and public interface

**Effort:** 3–5 days. **Depends on:** Phase 3. **Current:** SBML export and
source-token-preserving canonical layout and the optional CLI independently verified
for the accepted subset, not complete serialization of the full target profile. See
[stage-B review](reviews/canonical-format-review.md) and
[CLI review](reviews/cli-review.md).

Implement:
- Structured SBML L3V2 Core writer plus Content MathML for every supported expression.
- Required attributes/defaults, initial assignments, units and modifiers.
- Canonical Antimony formatter with explicit non-lossless behavior.
- Read-only model queries and warning/report access; thin optional CLI after the
  library is usable. CLI contains no duplicate parser or scientific semantics.

References: S3, S6/S7, S10, S13; `DESIGN.md` proposed API.

Verify: all positive profile fixtures independently validate; semantic projections
match; native format/reparse maintains meaning; XML escaping and number output
retain fidelity; excluded constructs are not erased to make export pass.

Guard: serialization is not validation. No template-string XML concatenation;
no direct public IR mutation bypassing checks; no surprise runtime dependencies.

## Phase 5 — compatibility, regression and hardening

**Effort:** 4–6 days. **Depends on:** Phase 4; fixture work begins earlier.
**Partial early work:** eight bounded hardening tests and native CI configuration
added with the initialization increment. Eight separately selected original analytic
models now pass the [numerical gate](reviews/numerical-review.md); the original
90 structural cases retain numerical NOT_RUN stages. Six remote CI jobs are NOT_RUN.
This is not completion of fuzzing, cross-platform or full-profile gates.

Implement:
- Complete curated corpus and attributed upstream subset; per-stage coverage report.
- Differential semantic checks, selected numerical comparisons and eligible
  official SBML fixtures using documented settings/expected results.
- Resource-limit tests, fuzz targets, deterministic output and concurrent-call tests.
- CI for chosen MSRV/current stable and supported operating systems.

References: S5/S6/S8/S9, `VALIDATION.md`.

Verify: actual counts/artifacts, all profile cases pass, independent oracle gates
run, no unexplained skips, fuzz smoke completes within declared budget. Record
failures and missing tools rather than hiding them.

Guard: no bytewise XML equality as the only oracle; no indiscriminate algebraic
simplification; no vendored GoogleTest counts claimed as Antimony coverage.

## Phase 6 — collaborative release review

**Effort:** 2–4 days engineering, plus reviewer/calendar time. **Depends on:** Phase 5.

Implement: API/documentation review, supported-feature table, examples, changelog,
provenance notices and dependency audit. Confirm repository/crate name, project
license, roles and publishing approval. Invite upstream review without claiming
endorsement. Only then remove `publish=false` if authorized.

References: S11; `COLLABORATION.md`; all release gates in `VALIDATION.md`.

Verify: clean checkout builds/tests/docs on advertised targets; examples run;
package contents reviewed; no reference virtualenv or credentials included;
compatibility report names exact versions and limitations.

Guard: no "official", "complete", "standard", or fastest claims without evidence
and agreement. Do not silently change profile scope to get release tests green.

## First five working days

1. Finalize profile and oracle pin; bootstrap package and provenance ledger.
2. Produce the first reference fixtures and source/diagnostic primitives.
3. Parse one complete explicit reaction model with a mathematical expression tree.
4. Resolve it and emit independently validated SBML.
5. Compare native/reference semantics, test a rejected feature, and prepare a
   concise review artifact for Herbert. Re-estimate using actual progress.

This is a proposed stretch milestone, not a deadline promise. If a semantic
ambiguity remains, present the failing/minimal case rather than fabricating success.

## Budget, risks and follow-on scope

Sequential planning total: approximately 21–34 engineer-days, plus review/rework;
budget 4–8 focused weeks. Calendar time increases with part-time availability,
oracle build friction or unresolved language questions. No measured velocity yet.

Risks: exact expression semantics; unit/default handling; ambiguous aliases;
dependency/runtime packaging; a first profile too narrow for collaborators. Mitigate
with the first vertical slice and a small nominated model corpus, not a large rewrite.

Next profiles can add enumerated built-ins, assignment/rate rules, richer units,
modules/composition, events and import. Each requires explicit new semantics and
oracle fixtures. Broad SBML import and PDE/particle coupling are separate projects,
not hidden v0.1 acceptance criteria. Preserve the no-solver library boundary.
