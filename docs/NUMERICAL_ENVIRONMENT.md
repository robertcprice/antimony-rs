# Isolated numerical environment

This is a **development-only SBML oracle**, not a solver in the Rust crate, CLI,
or default dependency graph. Its eight analytic anchors and comparison policy
are described in [the numerical guide](NUMERICAL.md). Successful installation is
not evidence of a successful kinetic simulation gate.

## Reviewed installation

The local reviewed host is CPython 3.12.12, macOS 15.5, arm64. Only released,
non-yanked CPython 3.12/macOS 14+ arm64 wheels were authorized:

| Package | Version | Wheel bytes | SHA-256 |
|---|---|---:|---|
| libroadrunner | 2.10.0 | 34,214,767 | `66f61f8ed7c40600895d2c8250e8515e25dc7648f5681a6e6a2e988701701e2a` |
| numpy | 2.2.6 | 5,084,103 | `894b3a42502226a1cac872f840030665f33326fc3dac8e57c607905773cdcde3` |

[Wheel URLs and metadata](../tools/numerical/wheels.json) come from official
[RoadRunner PyPI metadata](https://pypi.org/pypi/libroadrunner/2.10.0/json) and
[NumPy PyPI metadata](https://pypi.org/pypi/numpy/2.2.6/json). RoadRunner declares
`numpy~=2.2`; the reviewed environment deliberately pins 2.2.6 rather than allowing
future compatible releases to change the baseline. Runtime import and simulation
were tested; satisfying dependency metadata alone would not prove ABI compatibility.

```sh
UV_CACHE_DIR=.cache/uv uv venv --python /opt/homebrew/bin/python3.12 .venv-numerical
UV_CACHE_DIR=.cache/uv uv pip install --python .venv-numerical/bin/python \
  --only-binary :all: --require-hashes \
  -r tools/numerical/requirements-macos-arm64-py312.txt
.venv-numerical/bin/python -m unittest discover -s tools/numerical -p test_worker.py -v
```

No source builds, Tellurium, plotting packages, or automatic runtime installation.
`.venv-oracle` remains separate: Antimony 3.2.0 and python-libsbml 5.21.2 on
Python 3.12.12 were rechecked unchanged after installation. This is a package/version
check, not an independently captured before/after hash of every existing venv file.
The new venv occupied 120 MiB; observed free disk declined from 5.4 GiB to 5.0 GiB
including installation/cache and concurrent development activity. Do not infer exact
installation disk consumption from the volume-level free-space difference.

[Environment evidence](../tools/numerical/environment.json) records runtime/build
versions, selected installed-file hashes, API signatures, and actual setting
readback. The wheel reports LLVM 13 and bundled libSBML **5.21.1**, distinct from
the structural oracle's 5.21.2. Publisher wheel hashes establish artifact identity,
not reproducibility from a reviewed upstream source commit. No source-build
attestation or cross-platform result is claimed.

## Exact API and state contract

Each worker request creates a new `roadrunner.RoadRunner(sbml_text)`. Constructor
input is validated inline Level 3 Version 2 XML, never an arbitrary filename or
URL. The constructor also accepts URIs in the general upstream API, so passing
unvalidated request text directly would be unsafe. Model compilation is native
code: this boundary is intended for reviewed fixtures, not hostile SBML.

```python
rr = roadrunner.RoadRunner(sbml_text)
rr.conservedMoietyAnalysis = False
rr.setIntegrator("cvode")
i = rr.getIntegrator()
for key, value in {
    "stiff": True,
    "relative_tolerance": 1e-10,
    "absolute_tolerance": 1e-12,
    "maximum_num_steps": 100000,
    "variable_step_size": False,
}.items():
    i.setValue(key, value)
# Worker checks every requested value through i.getValue(key).
result = rr.simulate(start=0, end=10, steps=100,
                     selections=["time", "[S]", "S", "k"])
```

The live wrapper signature is `simulate(start=None, end=None, points=None,
selections=None, output_file=None, steps=None, times=None)`. `steps=100` yields
101 output rows including both endpoints. This is an output grid, not fixed
internal CVODE timesteps. The third positional argument is **points**, not steps.
Explicit `time` means model time; no seconds/minutes conversion is introduced by
this adapter. The parent uses each anchor's stated units and checks the time grid.
`S` selects species **amount**, `[S]` **concentration**, and `k` a parameter value.
Do not use `rr.S`: its legacy attribute behavior returns concentration. The live
stationary probe with compartment size 2 and initial concentration 4 returned
amount 8 and concentration 4 for all 101 rows, with parameter 3 unchanged.

Fresh instances avoid the documented state carryover between successive simulations
and ambiguity among `reset`, `resetAll`, and `resetToOrigin`. Only explicit
selections are accepted; no implicit default column order is trusted.

Primary references: [selection semantics](https://sys-bio.github.io/roadrunner/docs-build/selecting_values.html),
[solver API](https://sys-bio.github.io/roadrunner/docs-build/tutorial/solvers.html),
[Python wrapper at v2.10.0, lines 1495–1622](https://github.com/sys-bio/roadrunner/blob/v2.10.0/wrappers/Python/roadrunner/roadrunner.i#L1495-L1622),
and [main class API](https://sys-bio.github.io/roadrunner/docs-build/PythonAPIReference/cls_RoadRunner.html).
The generated website retains a stale 1.1.16 banner; the released installed APIs
were checked rather than assuming that banner identifies the wheel.

## Absolute tolerance: configured scalar versus effective vector

`absolute_tolerance=1e-12` is the **configured scalar**, not an assertion that each
species amount receives an unscaled absolute tolerance of 1e-12. In the reviewed
version, `getAbsoluteToleranceVector()` scales the scalar by the absolute species
amount when nonzero; for zero species it uses the absolute compartment volume
(unless that volume is zero, when the scalar stays unchanged). These anchors use
positive, fixed compartment volumes and no rate rules.

The worker records `effective_absolute_tolerance_before` and
`effective_absolute_tolerance_after` in `floating_species_ids` order. The former
is the pre-simulation query; the latter is a **current-final-state-derived query**.
The latter is not a saved solver-internal vector or proof of the tolerance used
at every internal timestep. For the stationary live probe S amount 8 and P amount
0 in volume 2, the vector was `[8e-12, 2e-12]` before and after. A changing model
can return different post-simulation values without changing the configured scalar.

This behavior is explicit in upstream
[`CVODEIntegrator::getAbsoluteToleranceVector`, v2.10.0 lines 805–889](https://github.com/sys-bio/roadrunner/blob/v2.10.0/source/CVODEIntegrator.cpp#L805-L889)
and the scalar/vector setting description at lines 165–167. Solver tolerance and
the parent's independently frozen comparison tolerance are different contracts;
the worker never loosens either or retries with another solver.

## Worker protocol and failure boundary

Invoke `.venv-numerical/bin/python -I /absolute/path/tools/numerical/worker.py`.
The parent supplies a temporary working directory and 60-second deadline, bounds
captured stdout/stderr, and terminates hung/crashed subprocesses. This is not an
OS-level sandbox. No native/core/example protocols change.

Request `schema="antimony-numerical-worker-v1"` requires exactly `sbml`,
`sbml_sha256`, `start=0`, `end=10`, integer `steps=100`, explicit `selections`, and
`solver` containing the settings above plus `name="cvode"`. Unknown fields,
duplicate JSON keys, nonfinite literals, DTD/entities, non-inline/non-L3V2 XML,
digest mismatches, unsupported settings, and invalid selection syntax are rejected.
Input JSON is capped at 8 MiB and SBML at 4 MiB, before simulation. At most 64
unique selection columns are accepted.

A response has the same schema and `status`:

- `PASS` (exit 0): finite 101-row `timeseries`, exact `column_names` and
  `selections`, original `sbml_sha256`, package/runtime `metadata`, exact selected
  `solver_readback`, `all_integrator_settings`, species IDs, and both tolerance queries.
- `ORACLE_BLOCKED` (exit 1): missing, mismatched, or unloadable pinned dependencies;
  never success credit.
- `ORACLE_ERROR` (exit 1): invalid request or simulation failure, with `reason`;
  never an analytic PASS.

Worker PASS only means the simulator completed the requested protocol. The parent
separately enforces provenance, projection preconditions, analytic curves,
conservation/boundary/held-parameter checks, comparison tolerances, and negative
controls. RoadRunner equality with itself is not independent scientific truth.
