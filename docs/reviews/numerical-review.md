# Independent analytic numerical review

**Status: PASS for eight selected original analytic models on the local pinned runtime.**
The original 90-case language corpus retains its numerical NOT_RUN stages.

## 1. What We Built (Plain English)

A development-only gate independently tests native-generated and reference-generated
SBML against handwritten closed-form trajectories. It checks explicit species
amounts and concentrations, conserved amounts, boundary clamps, compartments and
parameters at 101 time points. Both paths must match analytic truth; agreement
with each other is not sufficient. No solver, dependency or grammar change was
added to the Rust library or CLI. See the [guide](../NUMERICAL.md),
[model derivations](../NUMERICAL_MODELS.md), [plan](../NUMERICAL_PLAN.md) and
[isolated environment](../NUMERICAL_ENVIRONMENT.md).

## 2. Why It Matters (Impact & Implications)

Structural equivalence and valid XML cannot alone establish correct dynamics.
These anchors expose specific volume, stoichiometry, boundary and initialization
errors, including a parameter initialized from a species that must remain held
while that species decays. Unequal-volume transfer conserves total **amount**, not
the sum of concentrations. This is bounded translation evidence, not biological
truth, general Antimony/SBML conformance or stochastic validation.

## 3. Test Results (Verified, Not Assumed)

### Analytic and structural independence

One reviewer authored the equations, constants, structural preconditions and
original fixtures independently of simulator outputs. Another implemented the
orchestration/comparison gate. Reference Antimony 3.2.0 and independent libSBML
5.21.2 conversion/inspection matched all eight native projections; the reviewer
also checked **16 manual structural preconditions**, eight per SBML path. No
simulator-generated values became analytic expectations. Original model sources
are not copied official SBML Test Suite cases.

The accepted formulas and frozen settings were reviewed **before simulation**:
`t=0..10`, 100 intervals/101 points; CVODE stiff mode, fixed output grid,
relative tolerance `1e-10`, scalar absolute tolerance `1e-12`, maximum 100000
internal steps; conserved-moiety analysis disabled. Every selected value must
satisfy `abs(actual-expected) <= 1e-8 + 1e-7*abs(expected)`.
No thresholds were loosened after observing results.

### Executed gate

```sh
.venv-numerical/bin/python -m unittest discover -s tools/numerical -p 'test_*.py' -v
.venv-oracle/bin/python tools/numerical/check.py \
  --native target/debug/examples/export_sbml --trusted-native
```

- **69 numerical Python tests passed**, 0 failed/skipped: 29 checker controls,
  19 independent anchor tests and 21 worker tests, including three live-runtime
  tests. A preliminary system-Python invocation skipped three live tests; it did
  not earn live-runtime credit. The final pinned-environment run executed them.
- **8/8 selected numerical models passed**, 0 failed/blocked; native and reference
  each ran in a fresh process, for 16 trajectories with 101 rows apiece. This is
  eight models, not sixteen model successes or 1616 independent test cases.
- All rows had the exact requested columns, finite values and required time grid.
  Both analytic paths, trajectory differences and all invariants passed.
- Retained **104 libSBML warnings per path** (native and reference). They were
  neither suppressed nor treated as scientific validation. The unit-annotated
  decay has zero warnings; unspecified-unit cases retain theirs.
- All **27 fingerprinted inputs remained unchanged** during the gate, including
  source/manifest, native executable, checker modules, runtime records, interpreter
  and selected installed runtime files. Wheel hashes identify published artifacts;
  they do not prove source-build reproducibility.

| Original anchor | Worst native analytic absolute error | Worst native analytic scaled error |
|---|---:|---:|
| Unit-annotated decay | 4.742260e-9 | 0.095024 |
| Reversible equilibration | 1.076166e-9 | 0.006925 |
| Non-unit volume, no volume factor | 2.418515e-9 | 0.022809 |
| Clamped boundary | 3.552714e-15 | 7.758e-9 |
| Source/drain | 1.232128e-9 | 0.004583 |
| Forward InitialAssignment / held observation | 4.742260e-9 | 0.095024 |
| Stoichiometric conversion | 1.396509e-9 | 0.023313 |
| Unequal-compartment transfer | 3.656497e-9 | 0.039654 |

Scaled error is absolute error divided by that value's allowed bound; the pass
threshold is 1. Global native/reference analytic maximum was
`4.742259696044471e-9` absolute and `0.09502306465735567` scaled. Pairwise
native/reference maximum absolute difference was `3.6193270602780103e-13`, in
the non-unit-volume case. Tiny pairwise differences are retained, not normalized.

### Native and original-corpus regression gates

The independent reviewer used existing `CARGO_TARGET_DIR=target/independent-cli-review`
to avoid changing the standard oracle binaries. All-feature tests passed **13
binary unit + 244 integration + 2 doctests**; no-default tests passed **205
integration + 2 doctests**. There were zero failures/ignored tests. Executed:
`cargo fmt --all --check`; locked all-target all-feature check/build/test;
locked all-feature doctests; strict all-feature Clippy (`-D warnings`);
strict all-feature rustdoc (`RUSTDOCFLAGS='-D warnings'`, `--no-deps`);
locked offline all-target all-feature build; locked no-default all-target tests
and doctests; locked no-default dependency tree (zero normal dependencies).
These are actual reruns, not the previous CLI matrix relabeled as new results.

The inherited oracle Python suite was also rerun separately: **143 passed**, zero
failures/skips. It is not merged into the 69 numerical-method denominator. After
the narrow profile evidence wording/link update, the unchanged CLI binaries were
checked again against all 90 language fixtures: **48 supported + 42 required
rejections**, zero failures/blocked/changed inputs. The current CLI evidence
records this actual rerun without converting those numerical NOT_RUN fields.
It retains 307 libSBML warnings per export/formatted path and, separately, 48
core-library warnings per CLI command.

### Failure controls and review findings

Actual unavailable-native, unavailable-simulator-interpreter and unavailable-
reference-interpreter commands each returned exit 2 with **8 blocked, 0 passed**.
Reference unavailability produces aggregate FAIL with per-case ORACLE_BLOCKED;
preflight missing native/runtime produces aggregate ORACLE_BLOCKED. No stage
not reached is reported as passed.

Checker regressions reject common-mode incorrect trajectories even when they
agree with each other, nonfinite/boolean/huge-integer observations, wrong/truncated
rows, wrong columns/time grid/SBML identity, changed settings or runtime versions,
wrong amount-scaled tolerance queries, changed inputs and malformed worker output.
Independent review found and fixed a direct-comparator NaN/oversized-integer edge:
the full pipeline already rejected it, but the standalone comparator now rejects
it too. The final gate was rerun after hardening, not relabeled from its earlier
successful run.

RoadRunner's scalar absolute tolerance is amount-scaled. Its getter recomputes
from current amounts (zero amounts use compartment volume); the post-simulation
query is **not a log of CVODE's internal tolerance history**. The gate independently
checks initial scaling from the analytic initial state and final scaling from the
returned state. Scalar readback is not mislabeled a uniform per-species tolerance.

An independent reviewer reran all **69 numerical Python tests**, zero failures or
skips, and executed **seven live negative controls**. Four actual SBML mutations
(adding an unwanted volume factor, changing stoichiometry 2 to 1, removing the
boundary clamp, replacing the held InitialAssignment with an assignment rule)
were rejected by manual structural preconditions. For sensitivity testing only,
the reviewer deliberately bypassed those preconditions and simulated each mutant:
all four then failed the independent analytic comparison. Swapping amount and
concentration observations also failed analytic comparison; wrong time and NaN
observations failed protocol validation before numerical credit. These controls
are not additional positive models or unique automated test methods.

Pinned runtime: RoadRunner 2.10.0, NumPy 2.2.6, CPython 3.12.12, macOS 15.5 arm64.
Its reported bundled libSBML is 5.21.1, distinct from the structural oracle's
5.21.2. Existing Antimony/libSBML environment remains separate. No numerical
execution on Linux/Windows or remote CI is claimed.

The [compact durable numerical evidence](../../tools/numerical/evidence.json)
retains all 27 fingerprints, per-case stages, settings, errors, warning IDs and
SBML hashes. Final profile evidence-only SHA-256 is
`9c79b230d32e7c5df75802bfc073e79491a9f68c68e06ac4f6dedbc51d9f34f9`;
both the eight-case gate and 90-case CLI gate were rerun after that change.
The standard native exporter remained
`a7b40bb9b320cdcea3f44ccea5cb8b0effb601e2901d459b65cc67ebdfce3dec`.

The documentation reviewer verified **108 local links/anchors**, zero errors,
and `cargo package --list --allow-dirty` reported **297 entries**, with no
virtual-environment/cache/target/artifact/bytecode leakage; only the expected
unset package-metadata warning remained. No rebuild changed the oracle binaries.

Full trajectories and process results are retained in ignored
`artifacts/numerical-review.json`; unit output in `artifacts/numerical-unit-review.log`;
unavailable-tool controls in `artifacts/numerical-missing-*-review.json`.

## 4. What's Next (Concrete Next Steps)

Keep the original 90 numerical NOT_RUN records intact unless a future explicit
mapping actually executes those models. Next use representative models and
bounded differential generation, then eligible official semantic cases with their
own sampling/selection/tolerance contracts. Broader kinetics, events, rules,
dynamic compartments, scaled-unit conversion, other solvers and other platforms
are not covered by this eight-case gate. Release and remote execution still need
separate authorization. An optional `graphify update .` can refresh architecture
metadata; it was not run here.

## 5. Novel Opportunities (Innovation & Monetization)

A small inspectable collection of independent analytic contracts is useful for
exporter regression checks and integration pipelines without embedding a simulator
in production. Its differentiator is transparent evidence and explicit limits,
not inflated full-language or biological-validation claims. No publication or
external collaboration action is implied.

## Key Learnings:

1. Same-simulator agreement is insufficient; both outputs need independent anchors.
2. Unequal compartment volumes require amount-weighted conservation.
3. InitialAssignment values are not continuously evaluated assignment rules.
4. Solver control values, state-scaled tolerance queries and acceptance thresholds differ.
