# antimony-rs — Pure-Rust Antimony Language, SBML L2/L3 & MCA Engine

A 100% pure-Rust library and CLI (zero C/C++ dependencies, `#![forbid(unsafe_code)]`) for parsing the [Antimony](https://tellurium.readthedocs.io/en/latest/antimony.html) biological modeling language, translating bidirectionally between Antimony and SBML Level 2 (v1–v5) / Level 3 (v1–v2) Core, and running native stoichiometry analysis, ODE + discrete-event simulation, steady-state solving, and Metabolic Control Analysis (MCA) elasticity calculations.

## Quick Start

```rust
use antimony::{SimulationOptions, SteadyStateOptions, parse, write_sbml};

let src = "
model pathway()
  species S1 = 10, S2 = 0, $X0 = 5;
  k1 = 0.4; k2 = 0.2;
  J0: $X0 -> S1; k1 * X0;
  J1: S1 -> S2; k2 * S1;
end
";

let doc = parse(src).expect("valid Antimony model");
let model = doc.model();

// 1. Export to SBML Level 3 Version 2 Core XML
let sbml_xml = write_sbml(&doc).expect("valid SBML L3V2 export");
assert!(sbml_xml.contains("<sbml"));

// 2. Extract stoichiometry matrix (N)
let stoich = model.stoichiometry_matrix().expect("stoichiometry matrix");
assert_eq!(stoich.species, vec!["S1", "S2"]);

// 3. Simulate ODE + discrete-event time-course in pure Rust
let traj = model
    .simulate(&SimulationOptions::new(0.0, 20.0, 200))
    .expect("simulation trajectory");

// 4. Solve for steady state & compute MCA elasticity matrices (unscaled & scaled)
let ss = model
    .steady_state(&SteadyStateOptions::default())
    .expect("converged steady state");
let mca = ss.elasticity_matrix(&model).expect("MCA elasticities");
assert_eq!(mca.reaction_ids, vec!["J0", "J1"]);
```

---

> **Verified bounded checkpoint (2026-10-15): `modular-core-v0.8-slice` (Phases 6–13).**
> Phases 6–13 (`substanceOnly` species, dynamic compartments, reaction kinetic-law overrides,
> discrete `at` events, hierarchical `model`/`module` blocks, submodels, ports, `M.x`, `is`,
> `delete M.x` submodel deletions, `timeconv`/`extentconv` conversion factors, bidirectional
> SBML L2 (v1–v5) & L3 (v1–v2) Core reading/BioModels import & Antimony decompilation via
> `read_sbml` / `sbml_to_antimony` / `import_sbml`, multi-file `import "other.ant"` composition
> via `parse_with_imports` / `load_file`, and a pure-Rust stoichiometry matrix, RK4 ODE +
> discrete-event simulator, steady-state solver, and MCA elasticity engine via
> `stoichiometry_matrix()` / `simulate()` / `steady_state()` / `elasticity_matrix()`)
> now pass a 48-case modular/event/Phase-6–9 structural + RoadRunner gate, a 55-case bidirectional
> SBML $\leftrightarrow$ Antimony roundtrip + RoadRunner gate, and 2 closed-form analytic
> trajectory anchors alongside the 36-case independent collaboration gate and all sixteen
> retained compatibility and numerical gates (**20 gates total**).
> See [crate architecture](docs/ARCHITECTURE.md),
> [modular, events, and Phase 6–13 scope](docs/MODULAR_AND_EVENTS.md),
> [collaboration verification](docs/COLLABORATION_VERIFICATION.md),
> [functions scope](docs/FUNCTIONS.md), and [independent functions review](docs/reviews/functions-review.md).
> The earlier [functions slice](docs/FUNCTIONS.md), [rules slice](docs/RULES.md),
> [conditional slice](docs/CONDITIONALS.md), and [rational correction](docs/RATIONAL_CALLS_EVIDENCE.md)
> remain preserved historical evidence. These bounded tests are not full-language proof.

A [documentation erratum](docs/MATH_UNITS_ERRATUM.md) corrects the older rational
dimension claim: bare numeric/rational literals are unspecified, not known
dimensionless. Historical evidence bytes and runtime behavior are unchanged.

**Status: independently verified bounded native check/format/export/import/simulate pipeline.**
An optional unified CLI now exposes that pipeline with human diagnostics, a
versioned JSON report and staged file output. See the [CLI guide](docs/CLI.md),
[report schema](docs/cli-report.schema.json), [execution plan](docs/CLI_PLAN.md)
and [independent CLI review](docs/reviews/cli-review.md). It adds no language semantics.
A separate development-only gate now verifies **eight original analytic numerical
models**, plus [two modular/substanceOnly/compartment models](docs/MODULAR_AND_EVENTS.md), [two user-defined function models](docs/FUNCTIONS.md), [two assignment/rate rule models](docs/RULES.md), one [continuous-rate piecewise input](docs/CONDITIONALS.md) and [two separate builtin/time analytic inputs](docs/BUILTINS.md#two-selected-analytic-inputs), not full-profile dynamics; see the [numerical guide](docs/NUMERICAL.md),
[derivations](docs/NUMERICAL_MODELS.md) and [independent review](docs/reviews/numerical-review.md).
A separate [bounded workload gate](docs/WORKLOADS.md) now checks six original
structural motifs and 80 finite-grid cases, with independently authored expected
structures. See the [historical source-receipt guide](docs/SOURCE_EVIDENCE.md) and
[workload review](docs/reviews/workloads-review.md).
The verified `modular-core-v0.8-slice` extends the symbol/arithmetic foundation with
numeric builtins, constants, time, bounded conditionals, flat species/parameter/compartment
assignment (`:=`) and rate (`'=`) rules, reaction kinetic-law overrides (`J = ...` / `J := ...`),
`substanceOnly` species (`hasOnlySubstanceUnits="true"` / `initialAmount`), discrete `at` events,
declaration-ordered non-recursive user-defined functions (`function ... end`), hierarchical
`model`/`module` submodels/ports/`M.x`/`is`/`delete`/`timeconv`/`extentconv` flattening,
bidirectional SBML L2/L3 Core reading/decompilation (`read_sbml`, `sbml_to_antimony`, `import_sbml`),
multi-file `import "..."` composition (`parse_with_imports`, `load_file`), and pure-Rust
stoichiometry matrix extraction, ODE/event simulation, steady-state solving, & MCA elasticity matrices (`stoichiometry_matrix`, `simulate`, `steady_state`, `elasticity_matrix`); see [scope, evidence and an ordinary-user
example](docs/MODULAR_AND_EVENTS.md). It supports native parsing, dependency-checked
arithmetic initialization, bounded species/parameter/compartment units, six
model-default unit declarations, deterministic canonical layout, SBML export, SBML-to-Antimony import, and pure-Rust simulation. This is **not full Antimony**. Optional empty or port-bearing model headers and
comma-separated `formula` declarations are supported; the source-ordered symbol,
unnamed-reaction (`_J0`), unnamed-event (`_E0`), and submodel flattening increments are
independently verified within their stated scope.
See the [flat-declarations review](docs/reviews/flat-declarations-review.md),
[canonical-format review](docs/reviews/canonical-format-review.md) and
[species/default units review](docs/reviews/species-default-units-review.md) for exact
scope, commands and limitations; earlier [Phase 2 evidence](docs/reviews/phase2-review.md)
is retained as a historical checkpoint.
This is an independent open-source project verified against upstream `libAntimony` 3.2.0, `libSBML` 5.21.2, and `libRoadRunner` 2.10.0.

## Flexible and modular modeling milestone (Phases 1–13 verified)

The [flexible modeling plan](docs/FLEXIBLE_MODELING_PLAN.md) records a 75-probe
gap audit and phased language expansion for independently authored models.
Phase 1 adds general nested arithmetic, verified unary forms and scientific
MathML literals. Phase 2 implements source-ordered declarations, unnamed
reactions and unset/symbolic initialization. Phase 3A adds verified numeric builtins, constants and
time; Phase 3A.1 corrects rational-token recognition and preserves typed rational
atoms distinctly from division. Phase 3B supports bounded Boolean/comparison/
logical/piecewise expressions with demand-driven initialization and all-branch
static validation. Phase 4 (`rules-core-v0.4-slice`) supports flat
parameter and species assignment (`:=`) and rate (`'=`) rules. Phase 5
(`functions-core-v0.5-slice`) supports declaration-ordered non-recursive
user-defined functions (`function name(params) body end`) and passes the 36-model
[independent collaboration gate](docs/COLLABORATION_VERIFICATION.md). Phases 6–13
(`modular-core-v0.8-slice`) add `substanceOnly` species (`initialAmount`), dynamic/symbolic
compartments (`=`, `:=`, `'=`), reaction kinetic-law overrides (`J = ...`, `J := ...`),
discrete `at` events (with `after` delays and `t0`/`persistent`/`fromTrigger`/`priority` options), reusable `model`/`module` definitions with ports, submodel
instantiation (`A: sub(...), timeconv = tf, extentconv = ef`), `delete M.x` submodel deletions, `M.x` member overrides, `is` aliasing, flattening, bidirectional SBML L2/L3 Core reading/BioModels import/decompilation (`read_sbml`, `sbml_to_antimony`, `examples/import_sbml.rs`), multi-file `import "other.ant"` composition (`parse_with_imports`, `load_file`), and pure-Rust stoichiometry matrix extraction, ODE/event simulation, steady-state solving, & MCA elasticity matrices (`stoichiometry_matrix`, `simulate`, `steady_state`, `elasticity_matrix`, `examples/ode_trajectory_csv.rs`); see
[MODULAR_AND_EVENTS.md](docs/MODULAR_AND_EVENTS.md) and [ARCHITECTURE.md](docs/ARCHITECTURE.md). `delay(...)` in general formulas
and algebraic rules (`0 = ...`) remain **not implemented support**. See [the arithmetic review](docs/reviews/arithmetic-review.md),
[symbol guide](docs/SYMBOLS.md) and [optional-value API migration](docs/SYMBOLS_API.md).

## What this project is building

A 100% pure-Rust library that reads Antimony (including multi-file `import` hierarchies) and SBML L2/L3 Core, explains errors clearly, exposes a
structured biological model and stoichiometry matrix, formats canonical Antimony, exports/imports supported models to and from SBML, and simulates ODE + discrete-event trajectories, steady states, and Metabolic Control Analysis (MCA) elasticity matrices in pure Rust without
silently changing their meaning.

```text
Antimony source -> tokens -> syntax tree -> semantic analysis -> validated model
                                                         |-> canonical Antimony (accepted subset)
                                                         |-> SBML L3V2 Core <-> read_sbml / sbml_to_antimony (L2 & L3)
                                                         `-> StoichiometryMatrix, RK4 + Event Simulator, Steady-State & MCA Elasticities
```

The first release is an explicitly restricted kinetic-model profile. The long
term objective is wider community use, not inventing another Antimony dialect.

## Read in this order

0. [Crate architecture and repository map](docs/ARCHITECTURE.md) — directory layout (`src/`, `examples/`, `tests/`) and end-to-end pipeline.
1. [Architecture and API](docs/DESIGN.md) — modules, data flow, public boundaries.
2. [First-release contract](docs/PROFILE_V0_1.md) — included and rejected features.
3. [Implementation plan](docs/IMPLEMENTATION_PLAN.md) — ordered, testable phases.
4. [Validation strategy](docs/VALIDATION.md) — upstream comparison and evidence.
5. [Source discovery](docs/SOURCES.md) — pinned upstream and verified APIs.
6. [Collaboration brief](docs/COLLABORATION.md) — questions and a shareable proposal.
7. [Groundwork verification](docs/VERIFICATION.md) — what was actually checked.
8. [CLI guide](docs/CLI.md) — executable usage, reports, features and filesystem limits.
9. [Selected numerical validation](docs/NUMERICAL.md) — eight analytic anchors, not full conformance.
10. [Bounded workloads](docs/WORKLOADS.md) — six original motifs and 80 finite-grid cases.
11. [Historical source-receipt evidence](docs/SOURCE_EVIDENCE.md) — observed build receipts, not hermetic reproduction.

The machine-readable [target profile](profiles/kinetic-core-v0.1.json) describes
**planned scope**, not current support. It must stay consistent with the prose.

## Use the unified CLI

```sh
cargo build --locked --features cli --bin antimony
target/debug/antimony check fixtures/accepted/first_conversion.ant --json
target/debug/antimony format fixtures/accepted/formula_list_forward.ant -o target/formatted.ant --json
target/debug/antimony export fixtures/accepted/first_conversion.ant -o target/model.xml --json
```

Human diagnostics/status go to stderr; raw formatted Antimony or SBML can go to
stdout. `--json` instead produces one report on stdout, so JSON format/export
require an actual `-o` file path. Exit codes are 0 success, 1 usage/I/O/unavailable
feature and 2 language/resource rejection. Warnings remain visible on success.

The `cli` feature is optional; defaults remain `sbml`. A build with
`--no-default-features --features cli` supports check/format but rejects export
before opening input. The core-only no-default build has zero normal dependencies;
JSON support is isolated to the optional adapter. Source text is never executed.

The CLI validates before staging file output and supports explicit in-place
formatting. It rejects final output symlinks, nonregular/read-only destinations
and nonregular file inputs; see the [precise safety limits](docs/CLI.md#file-safety-and-limits).
This is not a durability, hostile-filesystem or rollback-after-report-failure guarantee.
The older examples below retain their original separate protocols.

## Run the verified first conversion

With Rust 1.96 or newer (tested here on 1.96/macOS arm64):

```sh
cargo run --quiet --locked --example export_sbml -- fixtures/accepted/first_conversion.ant target/first-native.xml
# Forward references and original SBML InitialAssignments:
cargo run --quiet --locked --example export_sbml -- fixtures/accepted/init_forward_chain.ant target/init-native.xml
# Unit scale/multiplier metadata, without changing numeric values:
cargo run --quiet --locked --example export_sbml -- fixtures/accepted/units_scaled.ant target/units-native.xml
# Concentration annotations map separately to substance units, without rescaling:
cargo run --quiet --locked --example export_sbml -- fixtures/accepted/species_units_scaled_concentration.ant target/species-native.xml
```

This uses native Rust only. The example reads an explicit file, checks the model,
and writes SBML L3V2. It reports unspecified units rather than pretending they
are dimensionally certified. Optional independent inspection uses the separately
installed [pinned development oracle](docs/ORACLE.md), never a runtime dependency.

The implemented slice accepts flat named/anonymous models (`model m` or
`model m()` followed by a newline/semicolon), named or unnamed reactions,
fixed literal compartments/stoichiometry, concentration initialization, boundary
species, ordinary const/var declarations and bounded arithmetic. Compatible
redeclarations and formula-to-species promotion follow source order.

Initial values may be absent. Species/parameter initializers may refer forward
to acyclic arithmetic dependencies; an unset dependency leaves the value symbolic,
not zero. Cycles, demonstrated division by zero and non-finite/domain errors still
fail explicitly. Expressions remain SBML InitialAssignments, not ongoing rules.
Optional-value getters expose the distinction; see the [API guide](docs/SYMBOLS_API.md).

```antimony
model example
  compartment cell;
  species S in cell, P in cell;
  formula k;
  S => P; k*S;
end
```

This model is structurally valid but does not yet specify enough initial data to
simulate. Missing species concentrations, parameter values and explicit compartment
size are omitted from SBML rather than invented. The implicit default compartment
still has size one. Computed compartment sizes and reaction-valued expressions
remain deferred, as do nonempty model ports and separator-free same-line headers.
`parameter` is not an upstream declaration keyword and is not an alias.

Named bounded unit definitions and species/parameter/compartment `has` annotations
now preserve metadata. Six model-default unit names preserve inheritance/overrides.
Known dimensional conflicts reject; unspecified values stay unknown. No implicit numeric conversion is performed.

## Format supported source

```sh
cargo run --quiet --locked --example format_antimony -- fixtures/accepted/species_units_defaults_scaled.ant target/formatted.ant
```

The library API is `format_antimony(&ParsedDocument) -> Result<String, Diagnostic>`.
It also works without the `sbml` feature. Formatting validates before returning,
preserves original token/statement order and literal-versus-InitialAssignment mode,
and is byte-idempotent. It removes comments/whitespace; it does not sort all
semantically equivalent models into one representation. Output expansion must fit
the retained resource limits. Invalid input cannot create/overwrite CLI output.

Scalar initial values and stoichiometry require zero or magnitude **[1e-307, 1e308]**
and at most **15 significant mantissa digits** (leading zeros/exponent excluded,
trailing zeros after the first nonzero digit included). Nonzero underflow rejects,
not clamps. Arithmetic expressions now share this literal envelope; unit coefficients retain
their separately bounded contract. Scientific MathML preserves decimal mantissa
digits and exponent, including very small reaction-rate constants.

**Not yet supported:** general unit algebra/conversions, compartment expressions,
rules/events/user-defined functions, imports/composition or amount initialization. Numerical
simulation is intentionally outside the library/CLI; the separate development
oracle does not change that boundary. Additional restrictions include builtin/explicit `default_compartment`
identifiers and a verified 15-digit expression-literal precision/range envelope.
`++S`, `+-S`, `-+S` and grouped unary negatives work; contiguous `--` remains
invalid upstream. Source grouping is preserved by formatting, while the semantic
tree follows the pinned upstream normalization, not arbitrary reassociation.
Initialization, public model expressions, unit checking and export use that same
tree. See [the arithmetic review](docs/reviews/arithmetic-review.md).
Bare standalone identifiers now receive `syntax`; diagnostic classification is not
universally complete (for example, `import "secret.ant"` still receives `syntax`).

**Current verification:** ten maintained gates PASS, including **124 rational
cases (73 exact supported exports, 51 expected rejections)**. The unchanged
original90, formatter90 and CLI90 routes each yield56/34; arithmetic54 yields48/6;
builtins109 yields66/43; symbols66 yields54/12; workloads86 yields74/12.
Eight retained and two builtin/time analytic models pass their separate trajectory
gates. **392 Python tests pass**, zero failures/skips. These overlapping inventories
are not a compatibility percentage. See [corrective evidence](docs/RATIONAL_CALLS_EVIDENCE.md)
and the [public rational API example](docs/RATIONAL_API_NOTES.md).

**Historical arithmetic checkpoint (before builtin/rational corrections):** 162 oracle/CLI, 69 numerical, 51 workload
and 31 receipt-harness Python tests pass (no final skips). The new
54-case arithmetic gate passes (47 exact supported exports, 7 explicit rejections).
The unchanged 90-case native, formatter and CLI gates each pass, as do the
8 selected analytic models and 86 structural workloads. These are distinct and
overlapping suites, not 418 independent models or a compatibility percentage.
All final gates used hash-stable copied all-feature binaries. The new arithmetic
cases have no trajectory-validation credit. See the
[arithmetic review](docs/reviews/arithmetic-review.md) for native matrix counts,
commands, independent models and limitations.

**Historical pre-arithmetic checkpoint (not current-source verification):**
all features: **13 binary unit + 244 integration + 2 doctests**;
default without CLI: **212 + 2**; no-default core: **205 + 2**;
CLI without SBML: **13 + 237 + 2**. **147 structural/CLI Python tests; 90 reference replays;
48 exact native exports + 42 required rejections; 48 byte-idempotent exact
native/reference formatting roundtrips + 42 formatter rejections**.
The new CLI independently passed all **90 cases**: 48 supported check/format/export
cases and 42 required rejections across all three commands, with exact artifacts.
The separate numerical suite passes **69 Python tests and 8/8 selected analytic
models**, with native and reference paths checked against independent equations
at 101 time points. It retains 104 libSBML warnings per path. These overlapping
suites are not a language-support percentage; the original **90 structural cases
still have numerical NOT_RUN stages**. The configured native CI matrix
(Linux/macOS/Windows × Rust 1.96/stable) has **NOT_RUN remotely**; only local macOS
verification is claimed. Separately, **51 workload tests and 86 structural cases**
(6 original motifs + 64 supported generated + 16 required rejections) pass; all 86
remain numerical NOT_RUN. This is bounded generation, not fuzzing or published-model
validation. The historical **39-command source receipt** passed, including 31 receipt
tests and exact Cargo-artifact handoff into all gates. It records an observed local
build with cache reuse, not hermetic or bit-reproducible compilation. Its source
hashes predate the arithmetic edits; it has not been regenerated for this increment.
See [ranked next increments](docs/reviews/value-priorities.md): eligible external
models, expanded independent contracts and sound failure reduction. Unnamed-reaction
generated IDs need their own reference-first increment.

## Exercise the arithmetic expansion

```sh
cargo build --offline --all-features --bin antimony
python3 tools/oracle/expression_check.py --cli target/debug/antimony --trusted-native
```

The development-only arithmetic gate contains **54 original cases: 47 supported
exports and 7 explicit rejections**. It checks ordinary CLI input, exact reference
structure, independent libSBML validation, formatted-source export and byte
idempotence. It retains **numerical NOT_RUN**: this is not trajectory validation.
The comparator does not erase grouping, numeric node categories or rounding
mismatches. Its pinned interpreter is optional for library/CLI users and mandatory
for this development gate. Existing 90-case expectations are unchanged.

## Exercise a combined structural workload

```sh
target/debug/antimony check fixtures/workloads/enzyme_binding_catalysis.ant --json
.venv-oracle/bin/python tools/workloads/check.py \
  --native target/debug/examples/export_sbml \
  --formatter target/debug/examples/format_antimony --trusted-native
# All source-bound build/test/evidence gates; choose a new output directory.
.venv-oracle/bin/python tools/verification/run.py --execute \
  --target-dir target/independent-cli-review \
  --output-dir artifacts/my-current-source-run
```

The CLI must be built as above; standalone workload checking requires explicitly
prebuilt examples and the [pinned development oracle](docs/ORACLE.md). The source
runner builds its own isolated artifacts and does not install dependencies.
See [workload usage](docs/WORKLOADS.md) for export and scope details.

## Non-negotiables

- Never guess a rate constant or discard unsupported behavior.
- Preserve reaction direction, compartment semantics and amount/concentration.
- Keep parsing, semantic validation and model execution separate.
- Native library code must not shell out to Python or libAntimony.
- Reference tools are allowed in development/CI, clearly isolated from runtime.
- Start with one library package; split crates only when a demonstrated need arises.
- No publication, license selection, remote creation, or official-branding claims
  until maintainers agree. The manifest uses `publish = false`.

## Effort and confidence

Working estimate: 4–8 focused engineer-weeks for this bounded first release,
assuming access to upstream review and no major semantic surprises. Confidence
remains moderate-to-low for the full release profile despite the verified narrow
vertical slice. This is not a
commitment to complete native Antimony compatibility in that time. A narrower
working example can arrive earlier; broad parity remains a separate multi-month
scope.
