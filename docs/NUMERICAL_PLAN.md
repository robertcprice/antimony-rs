# Numerical anchors: execution plan and decision log

Date: 2026-10-10. **Delivered: eight selected original numerical anchors passed.**
At plan creation no simulation pass was claimed; the decisions below preserve
that pre-execution contract. Final results are recorded in the
[independent numerical review](reviews/numerical-review.md) and
[bounded evidence record](../tools/numerical/evidence.json).
This phase adds development evidence, not a Rust solver, language feature or
biological-validity certificate. The 90-case structural corpus stays unchanged,
including its numerical NOT_RUN stages.

## Phase 0 — completed discovery

Three reviewers investigated simulator APIs/artifacts, the existing reference
contracts and independent mathematics before implementation. Relevant patterns:

- `docs/VALIDATION.md`, section 5: both native/reference simulation and independent
  analytic anchors, with explicit settings and sample counts.
- `tools/oracle/oracle.py`: pinned reference subprocesses, independent SBML
  inspection, closed typed projections and retained warnings.
- `tools/oracle/native_check.py`, `roundtrip_check.py`, `cli_check.py`: trusted
  prebuilt binaries, actual-source evidence and fail-closed missing-tool behavior.
- Existing accepted fixture patterns: explicit compartment binding per species,
  source-authored volume factors, original InitialAssignments and bounded units.
- Official RoadRunner release/API material and published wheel metadata, recorded
  in `NUMERICAL_ENVIRONMENT.md` with installed-version/readback evidence. Older
  online documentation is not assumed to describe every current solver default.

Discovery caught an authoring trap: `species S,P in c` binds only P to c.
Explicit intended bindings and independently asserted model preconditions must
precede trajectory comparisons. Initial amounts and concentrations are distinct.
The native source subset still initializes concentrations; observing amounts does
not establish support for amount-initialization syntax. Explicit `time` expressions
remain unsupported; the simulator's elapsed-time output is tested instead.

## Decisions frozen before simulation

### Environment and isolation

Use a **separate `.venv-numerical`**, CPython 3.12, with published macOS 14 arm64
wheels: `libroadrunner==2.10.0` and `numpy==2.2.6`. Discovery found approximately
39.3 MB of wheel downloads against 5.4 GiB free disk. Require wheel hashes and
binary distributions; do not build source or mutate `.venv-oracle`/system Python.
Pin URLs, hashes, package/runtime versions and relevant platform metadata. These
pins do not attest that a published wheel was built from a particular Git commit.

Runtime tools never download/install. Every simulation starts a fresh RoadRunner
instance in an isolated subprocess with bounded trusted SBML input and a timeout.
Subprocess isolation is not a security sandbox. No foreign runtime is introduced
into the Rust library or CLI.

### Sampling, integration and comparison

All eight cases request **101 rows**, including both endpoints of **t = 0 to 10**
(100 intervals). Require exact requested column order/count, complete rows, finite
values, independently checked timestamps and initialized first-row values. Never
truncate mismatched arrays through `zip` or silently omit unavailable selections.

Selected solver contract: CVODE, stiff mode, relative tolerance `1e-10`, absolute
tolerance `1e-12`, maximum steps `100000`, variable-step output disabled. The runtime
owner must prove these settings are supported and record effective readbacks.
Any scalar/vector tolerance API discrepancy must be resolved explicitly, not
hidden through fallback or assumed defaults.

Live setup confirmed all requested setting readbacks. In RoadRunner 2.10.0 the
configured scalar absolute tolerance is not necessarily the per-state vector:
the inspected tolerance-vector query scales using current amounts. Record the
pre-simulation and post-simulation query vectors separately; a post-run query is
not evidence that the solver used that vector throughout the trajectory. The
environment guide records the tagged-source behavior and live non-unit-volume
probe (amount 8 versus concentration 4 at volume 2).

Predeclared pointwise acceptance is

```text
abs(observed - expected) <= 1e-8 + 1e-7 * abs(expected)
```

Apply independently to native-versus-analytic, reference-versus-analytic and the
declared native/reference comparison. Check concentration and amount observations,
parameters where selected, initial conditions and model-specific invariants.
Report maximum errors and normalized error ratios, not just a pass flag. Tolerances
are fixed before simulation; no tuning after observing failures.

### Eight original cases

Detailed sources, equations, preconditions and derivations belong in
`NUMERICAL_MODELS.md`, written by the independent mathematics reviewer.

1. Unit-annotated first-order decay: `S(t)=3 exp(-0.4t)`; metadata does not silently
   rescale values. This is not a general unit-conversion test.
2. Reversible relaxation: `A(t)=1+2 exp(-0.4t)`, `B(t)=3-2 exp(-0.4t)`;
   conservation and equilibrium limit, not an assertion that t=10 is equilibrium.
3. Volume 3 with **no** source rate-volume factor: `S(t)=2 exp(-0.2t)` and
   `P(t)=2(1-exp(-0.2t))`; total amount 6. A compiler must not insert a volume factor.
4. Boundary reservoir: fixed `B=2`, product `P(t)=1+0.6t`; no false closed-system
   conservation requirement on an externally clamped source.
5. Source/drain balance: `S(t)=3(1-exp(-0.2t))`, including the zero initial value.
6. Forward InitialAssignments: `S(t)=3 exp(-0.4t)`, initialized `k=0.4`, `held=3`
   stays fixed while S decays. Final agreed source is
   `S=base+delta; formula k=p/2,p=0.8,base=2,delta=1; held=S; J:S=>;k*S;`.
   This deliberately uses the asserted implicit volume-one compartment; no V2
   or `held=S*cell` variant is part of the frozen case.
7. Stoichiometric consumption `2A -> B`: `A(t)=4 exp(-0.4t)`,
   `B(t)=2(1-exp(-0.4t))`; invariant `A+2B=4`.
8. Transfer between volumes 2 and 5: `A(t)=3 exp(-0.4t)`,
   `B(t)=1+1.2(1-exp(-0.4t))`; total amount `2A+5B=11`, not constant `A+B`.

Analytic constants are authored independently, not fitted to simulator output.
Source hashes, expected semantic projections, declared selections and preconditions
bind each curve to the actual model. Retain SBML warnings rather than suppressing
them. Cases unsupported by the native profile cannot earn numerical success.

## Implementation ownership

- Environment/runtime owner: pinned wheel files, requirements, environment
  manifest, isolated simulation worker, worker tests and environment guide.
- Mathematics owner: eight sources/manifest, analytic functions and tests,
  model derivations and independent invariants/preconditions.
- Harness/evidence owner: orchestrator/comparators, fail-closed tests, user guide,
  complete results and independent review.
- Root: scope, decision record, integration and final evidence reconciliation.

Owners coordinate explicit worker/manifest interfaces before freezing; no shared
file edits, native-source edits or changes to existing oracle fixture contracts.

## Verification and negative controls

1. Independently inspect both exports and require exact reviewed semantic
   projections and model preconditions before simulating either.
2. Check all 101 samples on each path against analytic curves and invariants.
   Verify solver selections, time grid, versions and effective settings.
3. Prove meaningful failures: wrong volume/rate/stoichiometry, frozen-state drift,
   omitted/swapped columns, missing/extra/time-shifted rows, NaN/infinity, vacuous
   comparisons, altered model/settings and missing worker/reference/binary.
4. Fingerprint sources, manifest, checker/worker/math code, requirements and
   executables before/after the run; preserve actual artifacts and bounded results.
5. Run new unit tests plus existing Python regressions and relevant native gates;
   review package exclusions for environments/cache/artifacts. No remote jobs are
   represented as executed merely because CI is configured.

## Delivered verification

- **8/8 original numerical models passed**, zero failed/blocked: both exported
  paths match the independent curves and invariants at all 101 requested points.
  Sixteen trajectories are eight models, not sixteen successes.
- **69 numerical Python tests passed**, zero failed/skipped: 29 checker controls,
  19 independent anchor tests and 21 worker tests, including three live tests.
- Independent native regression passed **13 CLI unit + 244 integration + 2 doctests**
  with all features and **205 integration + 2 doctests** without defaults. Formatting,
  check/build, strict Clippy/rustdoc and offline all-feature build passed; the
  no-default core dependency tree remains empty. Standard oracle binaries were
  protected by an isolated reviewer target.
- **Seven separate adversarial controls were caught**, not counted as successful
  models: four actual SBML mutations (volume factor, stoichiometry, boundary flag,
  InitialAssignment-to-rule) failed semantic preconditions and, when deliberately
  bypassed for the experiment, failed analytic comparison after simulation. Three
  modified live trajectories exposed swapped amounts/concentrations, shifted time
  and NaN. See the review for exact commands, runtime identity and error bounds.

## Claims and next steps

These eight selected numerical passes do **not** turn all 90 structural
cases into numerical passes. Keep the original numerical stages NOT_RUN and record
this separate denominator. Same-simulator agreement alone cannot exclude shared
simulator defects; closed-form checks provide a narrower independent anchor.
No stochastic, nonlinear/stiff-suite, PDE, biological or clinical certification.
The next priority is a representative workload and structured differential
generation for the existing supported language, with minimized frozen failures,
plus a current-source evidence runner that binds source/dependencies to the build
and verification run. Existing executable and checker hashes do not establish
source-to-binary reproducibility. Official SBML semantic-suite adoption remains a
separate provenance/licensing and coverage decision; broader language or release
readiness must not be inferred from this bounded milestone. See the
[updated priority review](reviews/value-priorities.md#4-broader-workload-and-reliability-evidence--bounded-increment-delivered).
