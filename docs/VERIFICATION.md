# Groundwork verification

## Current bounded conditional increment

`conditional-core-v0.3-slice` passes twelve maintained gates on new immutable
reviewed binaries: 262 conditional cases (159 exact exports,103 expected rejects),
all ten retained gates, and one separate continuous-rate piecewise analytic model.
Python 441 tests pass, zero failures/skips. The 17-command four-feature Rust matrix
passes446/401/386/431 tests plus 2 doctests each; 64 source hashes remain stable.
See [scope and exact counts](CONDITIONALS.md), [tested core API](CONDITIONAL_API_NOTES.md),
[independent review](reviews/conditionals-review.md), and
[compact evidence](../tools/oracle/conditional-evidence.json). Prior 3A/.1 receipts
remain historical, including the separately corrected [dimension prose](MATH_UNITS_ERRATUM.md).
This is neither full-language conformance nor a general dynamics certificate.

## Historical rational compatibility correction (2026-10-14)

`math-core-v0.2.1-slice` corrects a newly discovered gap outside the previous
bounded corpus: upstream rational numeric tokenization and exact rational-versus-
division structure. See the [supplemental evidence](RATIONAL_CALLS_EVIDENCE.md),
[independent review](reviews/rational-review.md), [tested API example](RATIONAL_API_NOTES.md)
and [compact report](../tools/oracle/rational-evidence.json).

All ten maintained gates passed against new immutable reviewed binaries. The new
124 cases comprise73 exact supported exports and51 required rejections; all nine
retained gates pass with unchanged corpus expectations. Python392 tests pass,
zero failures/skips. Final native matrix, independent models and API compilation
are reported separately; none implies full-language or runtime validity.
The previous3A reports and binaries remain unchanged historical evidence.

## Historical numeric builtin/time increment (2026-10-14)

At that checkpoint the implementation was `math-core-v0.2-slice`, a bounded extension that
includes numeric functions excluded by the older frozen kinetic target. See the
[guide and ordinary-file example](BUILTINS.md), [independent review](reviews/builtins-review.md)
and [compact evidence](../tools/oracle/builtins-evidence.json).

Nine maintained gates passed on unchanged reviewed binaries: builtin109=66/43;
arithmetic54=48/6; original90, formatter90 and CLI90 each56/34; symbols66=54/12;
workload86=74/12; eight retained analytic models plus two new analytic models.
Ratios here mean supported/expected rejection, not compatibility percentages.
Python **370 tests passed**, zero failures/skips. Native matrix and independently
authored model results are recorded in the review. Initial failed fixture
expectations were retained and corrected with independently reviewed positive
companions, not native-driven golden replacement or tolerance relaxation.

Explicit current overlays preserve historical source/reference snapshots. The
historical receipt runner below still has incompatible hardcoded inventories and
cannot certify this expanded profile. Earlier sections retain their own
checkpoint meaning rather than silently acquiring current implementation credit.

## Historical source-aware symbol increment

At this earlier checkpoint the implementation identified `flat-model-v0.2-slice`. See the
[symbol guide](SYMBOLS.md), [independent review](reviews/symbols-review.md) and
[compact retained evidence](../tools/oracle/symbols-evidence.json).

All seven maintained live gates passed against stable reviewer-built binaries:
new symbols66 (54 supported/12 rejected); retained arithmetic54 (47/7);
original90, formatter90 and CLI90 (each55/35); workload86 (six motifs plus68generated
supported/12rejected); and eight analytic numerical models. These inventories
and routes overlap; only the eight analytic models have trajectory credit.
Explicit `--current-profile` overlays account for changed native outcomes while
preserving historical source manifests and reference snapshots.

Python suites: **183 oracle + 69 numerical + 51 workload + 31 receipt-harness =
334 tests passed**, zero failures/skips with the pinned numerical interpreter.
The API migration example compiled and ran against the reviewed native library.
Native feature matrices and independently authored models are recorded in the
review, not inferred from these structural gates.

The historical source receipt below predates these edits. Its recipe hardcodes
older native-outcome inventories and **cannot certify the current profile**;
passing its unit tests is not a new receipt. See the symbol guide for current
commands and the distinction between maintained gates and old receipts.

## Historical pre-arithmetic checkpoint

The optional unified CLI now exposes the independently checked pipeline without
changing the language profile. See the [CLI guide](CLI.md), [execution plan](CLI_PLAN.md),
[JSON report schema](cli-report.schema.json) and [independent review](reviews/cli-review.md).

That checkpoint used `flat-declarations-v0.1-slice`: optional empty
model-header parentheses and comma-separated `formula` parameter lists extend the
previous checked initialization/units/format/export pipeline. The
[current review](reviews/flat-declarations-review.md) records scope, exact commands,
original live goldens and independent evidence. The
[canonical layout](reviews/canonical-format-review.md),
[species/default units](reviews/species-default-units-review.md) and earlier phase
reviews remain historical checkpoints.

Independent native feature-matrix results:

| Build | Binary unit | Integration | Doctests |
|---|---:|---:|---:|
| All features | 13 | 244 | 2 |
| Default, no CLI | 0 | 212 | 2 |
| No-default core | 0 | 205 | 2 |
| CLI without SBML | 13 | 237 | 2 |

All four configurations passed check/build/tests/doctests, strict Clippy/rustdoc
and offline build. The no-default core dependency tree contains only this package.
CLI coverage includes 15 owner and 17 independent subprocess tests per CLI mode,
plus 13 binary I/O unit tests. Platform-gated safety tests establish local behavior,
not Linux/Windows verification.

**147 structural/CLI Python tests; 90 reference replays; 48 exact native exports + 42 required
rejections; 48 byte-idempotent exact native/reference formatting roundtrips + 42
formatter rejections**, zero final failures/skips/blocked cases in real-oracle runs.
The independent CLI checker passes all **90 cases**: 48 supported check/format/export
cases and 42 required rejections across all three commands. Artifacts match existing
adapters exactly; original reference projections and formatting idempotence remain
independent requirements. Each command retains 48 native warning records across
supported cases; CLI export inspection retains 307 libSBML warnings. CLI missing
prerequisites produce 90 blocked cases, never success credit.

Both binaries and the manifest stayed unchanged during the roundtrip check. Missing
oracle gives **90 blocked**, formatter/idempotence NOT_RUN and exit 2. Original
native, formatted native and formatted reference exports each retain **307**
independent SBML warnings. The original 90 structural cases retain numerical
**NOT_RUN** stages; the separate eight-model gate below does not relabel them.
All six remote CI jobs remain **NOT_RUN**; only local macOS arm64/Rust 1.96 is
claimed for native checks. Reference Antimony 3.2.0 /
independent libSBML 5.21.2 remain isolated development tools. Wheel-build source
mapping and bit-reproducible/hermetic compilation remain unverified. The separate
observed current-source receipt below now binds a recorded local build invocation
to exact executable paths and subsequent gates; executable hashes alone still do
not establish reproducibility.

The new grammar preserves scalar versus InitialAssignment mode, source tokens,
first appearance/order, forward references and resource limits. It introduces no
`parameter` keyword or guessed initial value. Formula-to-species promotion remains
a strict `declaration_conflict` rejection; `const`/`var`/`$` formula qualifiers,
nonempty ports and separator-free headers stay deferred. Bare standalone identifiers
now receive syntax diagnostics, but `import "secret.ant"` still exposes an inherited
lexical classification gap. This is not universal unsupported-family coverage.

Scalar values still require zero or abs[1e-307,1e308] and at most 15 significant
mantissa digits; nonzero underflow and excess precision reject, not round. General
unit algebra/conversion, amount initialization, compartment expressions, broader
grammar, broader numerical coverage and release gates remain incomplete. No full-profile,
dimensional/biological certificate, publication or external change is claimed.
Bounded workload and observed-source verification are now delivered separately;
next priorities are eligible external models, expanded independently specified
cases and sound failure reduction; see the [ranked plan](reviews/value-priorities.md). Unnamed reactions need their own
reference-first generated-ID increment.

## Selected analytic numerical gate

**69 numerical Python tests passed**, zero failures/skips: 29 checker controls,
19 independently authored anchor tests and 21 worker tests (including three live
runtime tests). These are separate from the 147 structural/CLI Python tests.
**8/8 original analytic models passed**, zero failed/blocked, with 16 fresh-process
native/reference trajectories of 101 rows each. Both paths must match independently
authored closed-form curves and invariants; pairwise simulator agreement alone
cannot pass. Counts are eight models, not 16 independent model successes.

The gate checks non-unit/unequal volumes, explicit amounts and concentrations,
reversible/source-drain kinetics, boundary clamps, fixed stoichiometry, units and
held InitialAssignment-derived parameters. Seven live negative controls reject
selected semantic/protocol mutations. Each structural inspection path retains
**104 libSBML warnings**; those warnings do not constitute numerical validation.
Unavailable tools yield blocked cases with no success credit.

Runtime is development-only RoadRunner 2.10.0 / NumPy 2.2.6 in a separate
CPython 3.12.12 macOS arm64 environment, never a Rust/CLI dependency. Requested
CVODE settings, actual readback, time/selection contracts and frozen comparison
tolerances are recorded. Its scalar absolute tolerance is state-scaled; the
post-simulation getter is not evidence of the solver's internal tolerance history.
No biological, dimensional, cross-platform, full-language or full-profile certificate
is implied. The original 90-case corpus remains unchanged and numerically NOT_RUN.

See the [guide](NUMERICAL.md), [derivations](NUMERICAL_MODELS.md),
[environment](NUMERICAL_ENVIRONMENT.md), [independent review](reviews/numerical-review.md)
and [machine-readable evidence](../tools/numerical/evidence.json). Full ignored
run artifacts and exact error metrics are linked from the review.

## Bounded workloads and observed current-source receipt

**51 workload Python tests and 86 live structural cases passed**: six original
motifs, 64 supported finite-grid cases and 16 explicitly required rejections.
This is not a fuzz campaign or a corpus of published biological models. All 86
retain numerical NOT_RUN. Four structural projection paths must match independent
contracts; formatter idempotence and exact negative stage/code are required.
Original reference inspection retains 765 warnings (48 belong to upstream-accepted
negative cases); each supported native/formatted-reference/formatted-native path
retains 717. Missing binary/reference controls each block 86, with zero pass credit.

The actual **39-command current-source receipt passed**, none unexecuted; 283
project inputs and recorded dependency/toolchain/config/executable identities stayed
unchanged. All four native configurations above were rerun, as were **147 oracle,
69 numerical, 51 workload and 31 receipt Python tests**, zero failures/skips.
It then passed exact Cargo-selected artifacts into the original three overlapping
90-case gates, separate eight numerical models and separate 86 workloads. Final
Cargo artifact records say `fresh: true` (cache reuse), not clean compilation.

This closes the observed-source/build/gate evidence gap, not hermetic or
bit-reproducible compilation, complete dynamic-library trust, upstream source-wheel
attestation, malicious transient mutation or cross-platform execution. Independent
review also fixed fail-open manifest mutation/deletion and cached-source snapshot
ordering, and hardened report validation against wrong case IDs or failed substages.
No native Rust implementation or grammar was changed in this increment.

See [workload usage](WORKLOADS.md), [model contracts](WORKLOAD_MODELS.md),
[source-evidence protocol](SOURCE_EVIDENCE.md), [execution plan](RELIABILITY_PLAN.md)
and [final review](reviews/workloads-review.md). Compact summaries link their exact
retained full-report hashes; historical reviews retain their previous identities.

## Historical planning-only check (superseded status, retained evidence)

Checked 2026-10-10 UTC using Python 3.14.6 and local Git.

**13/13 planning-consistency checks passed.** These are document/manifest checks,
not Antimony language tests. **0 Rust builds and 0 native/oracle runtime tests**
were run **at that planning-only checkpoint**; a Cargo manifest and implementation
now exist as described above.

## Executed checks

- PASS: Expected planning files — 11 files
- PASS: Local Markdown links resolve — 9 local links; []
- PASS: Feature IDs unique — 26 features
- PASS: Included target features — 13 planned inclusions
- PASS: Explicit rejected feature families — 13 planned exclusions
- PASS: No implementation coverage invented
- PASS: Target is native and SBML L3V2 Core
- PASS: All seven ordered phases present
- PASS: Every feature references an implementation phase
- PASS: Reference pin consistent and runtime unverified
- PASS: No placeholder implementation claimed
- PASS: Independent repository with no remote
- PASS: Whitespace clean — []

## Independent source review

A second source review checked DESIGN and PROFILE against pinned upstream
semantics. Clarifications applied: initialization cycles differ from legal reaction
cycles; implicit parameters are valid language; units do not automatically rescale
values; boundary species are not SBML immutable constants.

## Not established at the historical planning checkpoint

No parser correctness, executable upstream compatibility, numerical agreement,
full-language support or performance has been demonstrated. The first example
was illustrative then; Phase 1 has since executed its reference conversion. Source links were
consulted during research; this local check did not crawl every external URL.

No remote, release, crate registration, collaborator message, or external change
was made. Cargo build/test commands were future gates at that checkpoint; their later
executed results are recorded in the Phase 1 review.
