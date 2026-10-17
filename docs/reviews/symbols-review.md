# Independent source-aware symbols review

Date: 2026-10-14. Scope: **`flat-model-v0.2-slice`**, Phase 2 of the
[flexible-modeling plan](../FLEXIBLE_MODELING_PLAN.md). Native matrix, independent source checks and maintained
reference gates passed. This is not full Antimony support, a simulator,
a published release or an upstream endorsement.

## What this increment enables

A user can write an incomplete model, inspect it, and later supply its initial
values without inventing zeros. Ordinary compatible declarations merge in source
order; formulas can become species; qualifiers and compartment locations retain
the meaning of subsequent explicit declarations. Unnamed reactions receive
reference-compatible generated identifiers, and later assignments can update a
reaction's kinetic law. The library and CLI accept arbitrary source, not only
models included in the tests.

The API deliberately changes three numeric queries to `Option<f64>` rather than
preserving a false guarantee that every structurally valid model has a complete
initial state. Species and parameters also retain `initial_expression()`:
`None` numeric with no expression is unset; `None` numeric with an expression is
symbolic but not yet evaluable. Evaluated expressions keep their InitialAssignment
for export. Explicit unset compartments remain distinct from the implicit size-1
default compartment. See the [API migration guide](../SYMBOLS_API.md).

## Review findings and corrections

Independent tests found two defects in the first native candidate:

1. A preceding `unit _J0=second` incorrectly collided with a generated `_J0`
   reaction instead of reserving that name and generating `_J1`. Reference probes
   also showed that earlier unit alias factors and annotation right-hand sides
   reserve names before their definitions. Reservation now follows those source
   occurrences without inventing numeric symbols for units.
2. A scalar reaction-law reassignment such as `_J0=9.999e-5` gave a public numeric
   expression value one binary64 ULP different from the identical directly written
   kinetic law. Both paths now use the same scientific-MathML numeric meaning.
   The regression checks public values as well as identical exported XML.

Cross-review of a reviewer-authored format case identified an existing parser
acceptance error: an extra newline or semicolon **between** reaction participants'
mandatory separator and the rate is rejected by the reference. Native parsing now
rejects it too. A multiline block comment there remains accepted, as does a newline
**after** the rate. The positive test was corrected rather than treating a native
acceptance as authority.

Pinned reference checks also established that a later explicit species compartment
location replaces its earlier location. The initially strict native conflict was
removed, and an outdated Agy-authored negative assertion was corrected. First-source
provenance remains distinct from the currently effective definition.

## Verified native and independent results

### Feature matrix and stable artifacts

The reviewer executed `cargo fmt --all -- --check` and, for each feature set,
`cargo build --all-targets`, `cargo test --all-targets`, `cargo test --doc`, and
`cargo clippy --all-targets -- -D warnings`. Cargo commands used `--offline --locked`.

| Features | Unit/integration tests | Doctests | Build | Strict Clippy |
|---|---:|---:|---|---|
| Default (`sbml`) | 268 passed | 2 passed | PASS | PASS |
| All features | 313 passed | 2 passed | PASS | PASS |
| No default features | 259 passed | 2 passed | PASS | PASS |
| CLI without SBML | 304 passed | 2 passed | PASS | PASS |

**17 commands passed; zero failed or ignored tests.** These feature configurations
repeat tests; their totals must not be added as distinct cases. The 21 reviewer
regressions in `tests/symbols_independent.rs` (20 without SBML) and 16 Agy-authored
ergonomics tests are included in the table, not additional to it. Agy's headless
session could not execute shell commands; this reviewer executed its tests and
the full matrix locally. One stale Agy assertion expecting conflicting compartment
locations was corrected to reference-backed last-location behavior before the matrix.

The reviewer fingerprinted **49 Cargo/source/test/example files** before and after:
all hashes matched. The scoped record is
`artifacts/symbols-phase2/independent/matrix.json`, SHA-256
`8a7238396d4444f9c390e2fc8679069018e79245331b56ae2eb7189cc897657a`.
This is not the older whole-project source receipt, dependency inventory or a
bit-reproducibility claim. Existing local toolchain and dependency caches were used.

After restoring an all-feature build, three artifacts were copied to
`artifacts/symbols-phase2/bin`; `manifest.json` pins their hashes. Final independent
source checks and maintained gates use these immutable copies, not a shared binary
that may change features between commands. CLI SHA-256:
`1e7957b4d7502b735d426aeaab82a935d78fb35aa0337bc66ba513a3c76affac`.

### Independently authored source exercise

The final exercise ran **97 original inputs** against the copied CLI: 44 targeted
reference probes, 48 combinations of initialization completeness, reserved names,
qualifiers and reaction arrows, and five follow-up regression sources.

- **85 supported exports:** exact whole-model reference projection equality,
  independent libSBML consistency checks without errors/fatal messages, formatting
  idempotence and identical native XML before/after formatting.
- **12 expected rejections:** three reference-rejected cycles (native reaction-rate
  self-reference remains unsupported); five known invalid initialization-domain
  cases despite unset operands; and four retained restrictions (nonpositive
  compartment sizes, bare location statement and both-empty reaction).
- **Zero unexpected failures.**

Reference versions were Antimony **3.2.0** and python-libSBML **5.21.2**. Results,
source strings, classifications, diagnostics and binary hash are retained in
`artifacts/symbols-phase2/independent/open-models.json`. Early reports recorded two
native rejections (unit-name reservation and changed compartment location), both
fixed before this final run; the one-ULP public-value defect was detected by Rust
API assertions rather than relying only on XML comparison.

These are selected structural cases, not a compatibility percentage, fuzz-coverage
claim, real-world corpus or trajectory benchmark. Missing initial values do not
certify simulation readiness. Conceptual overlap with maintained fixtures must not
be counted as additional scientific coverage.

### Maintained gates

The evidence owner executed the following gates against the same copied artifacts;
this reviewer independently inspected their reports, statuses, counts and recorded
binary hashes. All seven gates passed with zero failed or blocked cases:

- New symbols gate: **66 cases**, 54 supported exports and 12 expected rejections.
- Original native, formatter and CLI gates: **90 cases each**, the same 55 supported
  and 35 rejected inputs through three interfaces, not 270 distinct models.
- Retained arithmetic gate: **54 cases**, 47 supported and seven expected rejections.
- Workload gate: **86 cases**, six motifs, 68 generated supported inputs and 12
  expected rejections.
- Existing analytic numerical gate: **eight selected models** passed again.

The original inventories and reference snapshots remain frozen. Explicit
current-profile overlays change outcomes for seven formerly rejected original
inputs and four formerly rejected generated workloads. The arithmetic report
separates the retained corpus profile from the current implementation profile.
Missing or changed reference evidence is not converted into acceptance.

Reports are `artifacts/symbols-phase2/gates/{symbols,original,formatter,cli,arithmetic,workload,numerical}.json`.
All reported input-change checks were empty/false; this reviewer additionally
rehashed all 133 arithmetic and 157 symbols report inputs without discrepancies,
rechecked all three copied binaries, and rechecked all 49 matrix source hashes.
The evidence owner's Python logs record **334 passing tests**: 183 oracle/CLI,
69 numerical, 51 workload and 31 verification tests; no failures or skips.
Its API migration example was also compiled and executed locally.

As an additional regression check, the reviewer reran the preceding phase's 191
independently generated arithmetic sources against the final CLI: **183 exact
exports and eight expected syntax rejections**, unchanged from the earlier result.
This is a repeated regression sample, not newly authored Phase 2 coverage. The
new result is retained separately in
`artifacts/symbols-phase2/independent/arithmetic-expanded-models.json`; no historical
arithmetic result was overwritten.

The symbols, arithmetic, original-corpus and workload gates still record numerical
`NOT_RUN`. The eight numerical anchors are a separate, repeated result and do not
supply trajectory coverage for incomplete models or every new declaration form.

## Retained boundaries and quality risks

Positive scalar or unset compartments are supported; computed compartment sizes
remain deferred. Nonpositive sizes and known invalid finite-real initialization
subexpressions remain rejected. An unset sibling must not mask division by zero,
invalid powers, overflow or a dependency cycle. Multiplication by zero and a zero
exponent do not manufacture values for an unset dependency.

Reaction-valued expressions, both-empty reaction sides, bare location statements,
functions, time, conditions, runtime rules, events and hierarchical composition
remain outside this increment. A symbolic InitialAssignment is never substituted
for an assignment or rate rule. Existing numeric precision/range, source/token/
depth/entity limits and validated-model export gating remain in force.

No native oracle/process dependency, fixture whitelist or new normal dependency
was introduced. The historical source-bound receipt is not regenerated by this
review and must not be presented as a fingerprint of the changed source. No
cross-platform, clean-machine or new numerical-trajectory validation is claimed.

## Next steps and opportunities

The next shared foundation is typed builtin/function calls, time and conditions,
followed by true assignment/rate rules and reusable user functions. The public
optional-value representation and first-source spans support model inspection and
editor diagnostics without forcing a solver into the language crate. The broader
independent-testing milestone remains unfinished until those later phases pass.
