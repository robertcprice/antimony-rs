# Value audit and next increments

Date: 2026-10-10. Independent local groundwork; not upstream endorsement.

## Assessment

The current project is a credible **bounded native compiler**, not yet a general
Antimony implementation. Its strongest asset is fail-closed, independently checked
semantic translation: the reference must match reviewed expectations, and native
SBML must independently match that projection. Private validated models and usable
read-only queries already exist. Adding another model-query abstraction is not the
highest-value task.

Two read-only audits reached different but compatible priorities: the code audit
favored ordinary syntax and usability; the evidence audit favored numerical
behavior. Structural equivalence reduces translation risk, but neither demonstrates
trajectory accuracy nor establishes biological validity. More tiny happy-path
examples alone would yield diminishing returns.

### Fresh audit evidence, before implementation

- 44 Python contract/checker tests passed, with zero failures/skips.
- Six live formatter cases passed: four exact/idempotent exports, two correct
  rejections (including a known-invalid-reference strict-profile case).
- All 73 fixture hashes and eight evidence identities matched recorded evidence.
- Six independent language probes reproduced three classification problems:
  accepted `model m` and `formula a=1,b=2;` were reported as syntax errors;
  malformed bare `nonsense;` was reported as unsupported language.
- No successful incorrect export was found in this audit. No complete build,
  full-suite run, numerical simulation or remote CI was part of that audit.

Sources: `src/syntax/parser.rs`, `src/format/antimony.rs`, `src/model.rs`,
`tests/hardening.rs`, `tools/oracle/{native_check,roundtrip_check,oracle}.py`,
`docs/VALIDATION.md`, `.github/workflows/native.yml`, and the historical
[canonical-format review](canonical-format-review.md).

## Ranked plan

### 1. Ordinary flat syntax and diagnostic correctness — verified bounded increment

Implement optional empty model-header parentheses and independently verified
comma-separated **formula** declarations. Upstream rejects a `parameter` keyword;
do not invent one. Correct malformed bare-identifier diagnostics while keeping
recognized unsupported features explicit. Keep existing initialization, unit,
resource and numeric restrictions unchanged.

Reference patterns: existing parser declaration/initializer handling, formatter
token-preservation contract, source-span tests, and pinned Antimony 3.2.0 probes.
The formatter currently assumes a fixed header length; update it together with
grammar, never reconstruct source from normalized model state.

Verification: original upstream goldens; exact native/reference projections;
format/reparse and byte idempotence; malformed/unsupported distinctions; qualifier,
ordering, forward-reference and budget regressions; independent native and Python
gates. Version the implemented subset without changing frozen target feature IDs.

Guard: no rule/event/function expansion, guessed defaults, comma syntax outside
verified declarations, or loosening comparisons to conceal mismatches. Unnamed
reactions are separately deferred until generated IDs, collision behavior and
first-occurrence ordering have reference evidence.

Delivered as `flat-declarations-v0.1-slice`: 212 integration tests + 2 doctests,
120 Python tests and 90 reference cases; 48 exact exports and 48 exact/idempotent
formatter roundtrips, each with 42 required rejections. See the
[independent review](flat-declarations-review.md) for full gates and limitations.
This fixes the three audited cases, not every diagnostic-family gap: quoted-dot
import paths still expose an inherited classification problem. Formula symbols
promoted to species remain outside the native parameter-only declaration subset.

### 2. Thin usable CLI — implemented and independently verified

Expose `check`, `format`, and `export` through one adapter, using the existing
`SourceFile`, `parse`, `analyze`, `format_antimony`, and SBML APIs. Add human
line/column diagnostics and a versioned structured report. Reuse the formatter's
staged output handling rather than the export example's direct write.

Verify library/CLI agreement, exit-code and JSON contracts, warning visibility,
Unicode/in-place paths, no output on rejection and failure-safe replacement.
Keep filesystem code out of the core; do not add another parser or hide warnings.
The optional `antimony` binary now provides these commands, a versioned JSON
report, source positions and shared staged output. Independent verification passed
13 binary unit + 244 integration tests + 2 doctests with all features, four feature
configurations, 143 Python tests and the unchanged 90-case corpus (48 supported
models through all three commands, 42 required rejections). See [usage](../CLI.md),
[design decisions](../CLI_PLAN.md) and [the CLI review](cli-review.md).
The earlier rough estimate was 1–3 focused engineering days, not measured effort.

### 3. Numerical analytic anchors — delivered bounded evidence increment

Use a separate pinned, development-only simulator environment. Compare both native
and reference SBML against analytic decay/equilibrium models, non-unit compartment
behavior, boundary species and initialization. Record versions, selections,
amount/concentration meaning, time grid, tolerances and conservation checks.

Follow `docs/VALIDATION.md` numerical contract. Same-simulator agreement alone is
insufficient; analytic expectations provide an independent anchor. Missing tools
must block, not pass. No runtime solver dependency or biological-validity claim.
Delivered as a **separate eight-model numerical scope**: all eight passed native
and reference analytic comparisons, differential trajectories and invariants;
**69 numerical Python tests** passed without failures/skips. Independent native
regression remained **13 CLI unit + 244 integration + 2 doctests** with all features
and **205 integration + 2 doctests** without defaults, with the requested quality
gates passing. Seven independently exercised adversarial controls were caught;
they are failure-detection evidence, not additional passing models.

The original 90-case structural corpus retains numerical NOT_RUN status. See the
[independent numerical review](numerical-review.md),
[pre-execution decision contract](../NUMERICAL_PLAN.md),
[analytic derivations](../NUMERICAL_MODELS.md) and
[bounded evidence record](../../tools/numerical/evidence.json).
The earlier estimate of roughly 2–4 focused engineering days was planning guidance,
not a measured effort claim.

### 4. Broader workload and reliability evidence — bounded increment delivered

Select a representative model workload before expanding major language features.
Use structured differential generation for initialization DAGs, units, compartments
and numeric bounds, with minimized frozen counterexamples. Add resource-boundary
subprocess tests and later a genuinely coverage-guided fuzz target. Existing
deterministic mutation tests must not be relabeled fuzz coverage.

Add a current-source evidence runner that fingerprints source, dependencies,
checkers and fixtures before/after building and running gates. Bring the native
checker's manifest-mutation guard to parity with the roundtrip checker. Current
binary/checker hashes are useful but do not prove source-to-binary reproducibility.
Run the configured OS/toolchain matrix only when remote setup is authorized.

Delivered: **6 original motifs + 64 supported finite-grid cases + 16 required
rejections**, with **51 workload Python tests**. All new cases remain numerical
NOT_RUN; the motifs are original examples, not published-model validation, and
finite structured generation is not fuzz coverage. The observed current-source
receipt passed **39 commands**, including four native configurations and separate
147 oracle/69 numerical/51 workload/31 receipt Python tests. Exact Cargo artifacts
were passed into all existing and new gates; source/dependency/config/toolchain
identities remained unchanged. Cache reuse was recorded honestly; hermetic or
bit-reproducible builds are not established. See [workloads](../WORKLOADS.md),
[source evidence](../SOURCE_EVIDENCE.md), [the plan](../RELIABILITY_PLAN.md) and
[the review](workloads-review.md).

**Next bounded priority:** nominate provenance-eligible external model workloads
and expand reviewed parameter dimensions before increasing counts. Sound
classified-discrepancy-preserving minimization remains planned. Broader numerical
coverage, coverage-guided fuzzing and authorized remote-platform execution remain
separate milestones; no new grammar is justified solely by this batch.

## Boundaries and opportunity

No publication, remote creation, license selection, collaborator messages or
downstream integration is implied. Numerical execution is verified only for the eight
explicit original anchors; broader numerical coverage and remote-platform stages
remain unexecuted. No general compatibility or release-readiness claim follows.
The differentiating opportunity is a small usable
offline compiler backed by auditable conformance evidence—not unsupported claims
to be the official standard, fastest parser, or full Antimony replacement.
