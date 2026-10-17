# Development-only analytic numerical gate

**Status: eight selected original analytic anchors passed locally.** Both native
and reference SBML trajectories passed independent closed-form comparisons and
invariants. The original 90-case corpus remains unchanged, including its numerical
NOT_RUN fields. See [actual commands and boundaries](reviews/numerical-review.md).
See [the plan](NUMERICAL_PLAN.md) and [independent model derivations](NUMERICAL_MODELS.md).

## What this checks

This optional Python harness evaluates **native-generated SBML** and separately
**reference-generated SBML** against independently handwritten closed-form curves.
Matching two outputs from the same simulator is necessary here, but insufficient:
both must independently satisfy every analytic sample and explicit invariants.
It is not a biological validation, general Antimony/SBML conformance proof, or
stochastic simulation test. The Rust crate and CLI acquire no simulator dependency.

The eight original models exercise annotated decay, reversible equilibration,
non-unit volume without an author-supplied volume factor, a clamped boundary
reservoir, source/drain dynamics, forward InitialAssignments (including a held
initial observation), non-unit stoichiometry and unequal-compartment transfer.
The unit-bearing case tests preserved metadata and a dimensionally consistent
model; it does not claim automatic unit conversion or rescaling.

## Frozen numerical contract

- Start 0, end 10, 100 intervals: **101 rows**, including both endpoints.
- Explicit ordered selections include every species amount (`S`) **and**
  concentration (`[S]`), compartments and relevant parameters. No default selection.
- Every time must match `i / 10` within `1e-12`; strictly increasing, no interpolation.
- Every numeric result must be finite; booleans are not numeric observations.
- Every observed value passes only if `abs(actual - expected) <= 1e-8 + 1e-7 * abs(expected)`.
  These thresholds were selected before any simulation, not tuned after results.
- CVODE: relative tolerance `1e-10`, scalar absolute tolerance `1e-12`,
  `maximum_num_steps=100000`, `variable_step_size=false`, `stiff=true`.
  Conserved-moiety reduction is disabled to avoid hiding conservation defects.
- These solver controls are not the acceptance tolerance. The worker records
  their readback plus the runtime's effective species-tolerance query.

RoadRunner scales the requested scalar absolute tolerance by species amount,
using compartment volume for zero-amount species. Its getter after simulation
computes from the **current state**, not a history of tolerances actually used by
CVODE. The gate checks the initial query against independent initial amounts and
the final query against returned final amounts; it does not mislabel either as a
complete trace of internal solver error controls.

The documented API uses [`S` for amount and `[S]` for concentration](https://sys-bio.github.io/roadrunner/docs-build/selecting_values.html).
[`simulate` uses points = steps + 1](https://sys-bio.github.io/roadrunner/docs-build/PythonAPIReference/cls_RoadRunner.html).
[Integrator settings](https://sys-bio.github.io/roadrunner/docs-build/tutorial/solvers.html)
are explicitly set/read back rather than inherited from defaults. These online
documents describe the API; the installed pinned runtime must independently prove
that it supports the requested contract.

## Execution and isolation

Installation is an explicit separate operation; no harness command runs Cargo,
pip, downloads or network requests. Keep existing Antimony/libSBML reference
conversion in `.venv-oracle`; simulation uses a separately pinned numerical
virtual environment. See [the pinned environment and installation record](NUMERICAL_ENVIRONMENT.md)
before invoking it.

```sh
# Existing trusted binaries must already be built; this command never builds.
.venv-oracle/bin/python tools/numerical/check.py \
  --native target/debug/examples/export_sbml --trusted-native
.venv-numerical/bin/python -m unittest discover -s tools/numerical -p 'test_*.py' -v
```

Each native/reference trajectory uses a fresh process and simulator instance,
with isolated Python imports (`-I`), a temporary working directory, inline SBML
rather than a filename/URI, and a 60-second deadline including compilation.
Only owned, source-hash-verified fixtures are accepted; imports and XML
DTD/entities are forbidden. This is state isolation, **not an OS security sandbox**.

Stages remain separate: reference conversion, native export, independent structural
inspection/preconditions, each simulation, each analytic comparison, trajectory
comparison, and invariants. Missing/wrong simulator or reference dependencies are
ORACLE_BLOCKED, never success. Runtime crashes/timeouts/protocol errors are
ORACLE_ERROR. Structural or numerical mismatches are FAIL. Unexecuted stages
remain NOT_RUN. Only all required PASS stages and unchanged fingerprints can
produce overall PASS. Rejected models are not numerical successes.

The final report records versions, source/SBML/executable/checker hashes, solver
settings and queries, full trajectories, warning messages and worst scaled error.
Do not strip warnings or promote simulator-generated values into analytic truth.
The gate also checks the recorded identities of selected installed runtime files
and the numerical interpreter; this is not a complete source-build attestation.
Hash drift fails the complete gate. The separate eight-model denominator must
never relabel the earlier 90 models as numerically validated.

## Review boundaries

Fresh-process success does not prove other platforms, untested solvers, broad
real-model coverage, stiff nonlinear kinetics, events/rules, dynamic compartments,
scaled-unit conversion or release readiness. Official SBML Test Suite models are
not used or claimed in this original eight-case increment. Remote CI remains a
separate authorized action.
