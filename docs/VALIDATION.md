# Compatibility and release verification plan

Current evidence: [Phase 1](reviews/phase1-review.md), the
[narrow Phase 2 slice](reviews/phase2-review.md), and
[bounded initialization](reviews/phase3-initialization-review.md), and
[bounded units](reviews/phase3-units-review.md), and
[species/default units](reviews/species-default-units-review.md), and
[canonical layout](reviews/canonical-format-review.md) passed their gates.
The bounded [symbol foundation](SYMBOLS.md) and later [numeric builtin/time
extension](BUILTINS.md), followed by the [rational compatibility correction](RATIONAL_CALLS_EVIDENCE.md),
and now [bounded conditionals](CONDITIONALS.md) have passed their independent gates.
The remaining full-profile/release gates below are goals, not implied passes.

## 1. Four separate questions

1. Did we parse the intended syntax?
2. Did we preserve the model's meaning during resolution/export?
3. Is the exported SBML independently valid for its declared level/version?
4. Do selected exported models behave like the reference in a simulator?

None proves the model is biologically true. The crate does not perform clinical
validation and does not itself solve ODEs/PDEs.

## 2. Pin and isolate the oracle

`tools/oracle/` now exists and has been independently exercised in Phase 1/2.
The pinned setup is documented in [ORACLE.md](ORACLE.md). Use a project-specific
environment; do not install into system Python. Check disk/resource
availability before installing native wheels. Prefer a published compatible
release chosen with maintainers over building the full native dependency stack.

Manifest must record Antimony source/release, package hashes, libSBML build and
enabled packages, Python/platform, optional RoadRunner version, solver settings,
SBML target, corpus version and individual fixture hashes/licenses. A source SHA
and a PyPI wheel are not proven identical without a traceable mapping.

Process each reference model in a fresh subprocess initially; clear library state
and record diagnostics. Reference packages are dev-only. Absence of an oracle is
`ORACLE_BLOCKED`, not a passing native test and not permission to generate guessed
golden output. Do not auto-download on ordinary `cargo test`.

## 3. Release fixture inventory (targets, not complete coverage)

The historical original language corpus has 90 reference cases: 48 supported and
42 required rejections. The explicit `math-core-v0.2.1-slice` native-outcome overlay
expects 56 supported and 34 rejections for those unchanged sources/snapshots. Two
cases have audited invalid-reference SBML and can earn only native rejection
credit. The new symbol corpus is separate; see [current gate instructions](RATIONAL_CALLS_EVIDENCE.md).
These bounded inventories are not the complete floor below.

Minimum 40 curated cases before a v0.1 release candidate:

- 8 syntax/model-boundary cases: comments, separators, UTF-8 spans, numeric syntax,
  anonymous/named model, invalid token and truncated source.
- 10 semantic cases: inferred species, inferred parameter, forward references,
  explicit non-unit compartment, default compartment, boundary flags, source/sink,
  reversible/irreversible and fractional stoichiometry.
- 8 expression/initialization/unit cases: precedence, unary minus, exponent
  association, initial dependency, cycle, unspecified units, consistent units,
  inconsistent specified dimensions.
- 10 unsupported-feature cases: event, assignment rule, rate rule, nested module,
  import, function call, dynamic compartment, annotation, substanceOnly, package syntax.
- 4 adversarial/resource cases: oversized source, deep expression, malformed
  Unicode boundary and symbol/entity explosion.

These 40 are a floor, not sufficient by themselves. Every feature family in the
profile needs coverage, including exclusions not represented by those first 10
negative cases; every discovered regression becomes an additional fixture.
Use tiny original examples first. Record notices/hashes before copying upstream cases.

Each case contains source, target feature IDs, expected acceptance/rejection,
expected diagnostic if rejected, semantic projection if accepted, provenance,
oracle version/status and any numerical applicability. Do not promote expectations
produced solely by the Rust implementation itself.

## 4. Compare semantics, not serialized bytes

Normalize reference and native SBML through an independently parsed projection:

- entities, identifiers, compartment ownership and dimensions;
- amount versus concentration initialization, unset values, constant/boundary flags;
- parameter values/units and InitialAssignment expressions;
- reactant/product stoichiometry, modifier sets and reversibility;
- mathematical operators, arguments, symbol binding and unit definitions;
- package declarations and all behavior-changing constructs.

Ignore XML namespace-prefix spelling, whitespace, attribute order and timestamps.
Do not ignore behavior-affecting defaults. Record only explicit, reviewed
canonicalizations (for example known unary-minus representation); do not assume
algebraic equivalence, reorder floating-point expressions, or simplify singularities.
Canonical Antimony formatting is checked by parse/analyze/format/reparse equivalence.

## 5. Independent SBML and numerical checks

Use libSBML to inspect parse messages and run consistency checks. Fail on relevant
errors; classify warnings with rationale, especially unspecified units. No global
warning suppression. Required L3V2 Core attributes and initialization semantics
must be asserted directly, not inferred from an absence of exceptions.

For a selected runnable subset, use the same independently installed simulator
to execute reference-generated SBML and native-generated SBML. Pin initial state,
solver, tolerances, duration and sampling. Inspect more than one time point.
This can reveal translation defects but does not rule out shared simulator errors.

Add analytic checks (first-order decay, reversible equilibrium) and eligible
official SBML semantic cases as independent anchors. Official cases provide
settings/amount-versus-concentration selection and expected outputs; use their
sampling contract (steps+1 samples) and documented absolute/relative tolerances.
Do not claim stochastic conformance from deterministic comparisons.

A separate [eight-case original analytic gate](NUMERICAL.md) now passed locally
through independently pinned RoadRunner execution of native and reference SBML.
Each path also matched handwritten closed forms and explicit invariants; agreement
between paths alone was not sufficient. This does **not** relabel the original
90-case corpus: its numerical stages remain NOT_RUN. These are not official SBML
Test Suite cases, cross-platform runs or a general conformance claim. See the
[numerical review](reviews/numerical-review.md) for frozen settings and actual results.

## 6. Native engineering gates

These commands are runnable after Cargo bootstrap. The independent reviews record
which ran and their actual outcomes; other platform/release work remains pending:

```sh
cargo fmt --all --check
cargo check --all-targets --all-features
cargo test --all-targets --all-features
cargo test --doc --all-features
cargo test --no-default-features
cargo clippy --all-targets --all-features -- -D warnings
cargo doc --no-deps --all-features
cargo package --list --allow-dirty
```

Add a reproducible build on the chosen MSRV and current stable. Test Linux,
macOS and Windows before promising platform support. A six-cell native CI matrix
is configured but has NOT_RUN remotely; local tests alone do not fulfill that goal. Default core tests remain
offline-capable once Rust dependencies are fetched. Optional oracle jobs explicitly
report available/unavailable dependencies. Archive job artifacts and corpus manifests.

Fuzz lexer/parser/analyzer/export inputs under limits; regression-test each crash.
Differential fuzzing must stay inside the supported grammar to avoid claiming an
intentional profile rejection is a semantic bug. Test repeated/concurrent parses
for state leakage and deterministic outputs. Benchmark only after correctness;
do not claim speed superiority from a toy fixture.

## 7. Report and release gate

Use separate statuses `PASS`, `FAIL`, `UNSUPPORTED_PROFILE`, `ORACLE_BLOCKED`,
`NOT_RUN`. Separate syntax/resolve/export/numerical columns. Unsupported behavior
can pass a **rejection test**, but is never counted as supported-model success.

Release requires all selected profile gates pass, no unexplained skips, a known
limitations list, dependency/license review, maintainer review and authorization
to publish. No artifact may claim official Antimony status solely because tests pass.
