# Independent initialization + regression review

Reviewed 2026-10-10 UTC. **Gate: PASS for `initialization-flat-v0.1-slice`.**
This is the bounded arithmetic-initialization portion of Phase 3 plus regression
automation—not completion of Phase 3, the target profile, or a release gate.

## 1. What We Built (Plain English)

Models may now initialize species and parameters with bounded arithmetic and
forward references. The checker determines dependency order without recursive
chain traversal, rejects cyclic/missing/invalid initialization, and keeps original
expressions for SBML InitialAssignments rather than replacing them with ongoing
rules or silently folding away meaning. Numeric model queries expose checked
initial results. Fixed compartment sizes remain literal-only.

Independent review added eight deterministic hardening tests and a native-only
six-cell CI configuration. The tests cover generated/mutated input, source spans,
repeated/concurrent compilation, finite outputs, dependency order and resource
ceilings. The configured CI matrix has **not run remotely**; it is not evidence
of Linux/Windows or multiple-toolchain compatibility.

## 2. Why It Matters (Impact & Implications)

A useful class of models can express relationships such as `z=a*2; a=b+1; b=3`
without users manually substituting values or moving declarations. Initialization
cycles remain different from legal cyclic reaction networks. The increment keeps
a single semantic source of truth: retained expressions become InitialAssignments,
not duplicated scalar attributes or assignment rules.

### Reviewed safety and semantic boundaries

- Iterative Kahn traversal resolves initialization-only edges; repeated symbol
  references contribute one dependency. Long chains do not consume recursion depth.
- Unknown/missing initial values never become zero. Errors include checked source
  locations, including a true member of an unresolved dependency cycle.
- Each operation requires a finite real `f64` result. Division by either sign of
  zero, non-real powers, zero to nonpositive powers, intermediate overflow and
  negative resulting species concentrations fail closed. This is not exact symbolic
  arithmetic or elimination of future runtime singularities.
- Species expressions denote concentration, not amount. Explicit scalar compartment
  size participates as its own symbol; no hidden numeric volume scaling is inserted.
  Compartment expressions—including `+2`/`(2)` assignment forms—are deferred, avoiding
  unimplemented species/volume dependency cycles.
- Original roots remain accessible through read-only InitialAssignment views.
  Parameter constant flags propagate upstream-observed normal/boundary species
  dependencies. Expression-initialized entities omit scalar value/concentration
  attributes when exported. Their native initial-value queries remain evaluated.
- InitialAssignment serialization follows reference type/first-occurrence order,
  not dependency order. Declaration order does not alter evaluated values; XML order
  is not incorrectly claimed byte-identical after arbitrary source reordering.
- The validated model still has private construction/state, immutable accessors,
  no public mutation bypass, no unsafe code, and no native filesystem/network/process
  calls. Explicit I/O remains in the example adapter. No solver was introduced.
- Earlier bounded grammar, numeric-expression precision/range and unsupported-
  grouping restrictions remain. Units, functions, rules/events, imports, amount
  initialization, formatter and complete unsupported-family diagnostics are not
  implemented by this increment.

The reference remains Antimony 3.2.0 with separately installed libSBML 5.21.2.
Release-tag and research SHAs do not prove the wheel build's source SHA. Numerical
simulation remains **NOT_RUN**, and successful translation does not establish
biological or dimensional validity.

## 3. Test Results (Verified, Not Assumed)

Independent final run: Rust/Cargo 1.96.0, CPython 3.12.12, macOS 15.5 arm64.
Native executable SHA-256:
`ff639e228947da9f4756eb01a4ce9aac93c95b0d2b0708ed0141712371274de5`.

| Exact command | Observed result |
|---|---|
| `cargo fmt --all --check` | Pass |
| `cargo check --locked --all-targets --all-features` | Pass |
| `cargo build --locked --all-targets --all-features` | Pass |
| `cargo test --locked --all-targets --all-features` | **81 integration passed**: 22 foundation + 33 slice + 18 initialization + 8 hardening; 0 unit/example tests; no failures/ignored |
| `cargo test --locked --doc --all-features` | **1 passed** |
| `cargo test --locked --no-default-features` | **77 integration + 1 doctest passed**; 4 export-only tests not compiled |
| `cargo clippy --locked --all-targets --all-features -- -D warnings` | Pass |
| `RUSTDOCFLAGS='-D warnings' cargo doc --locked --no-deps --all-features` | Pass |
| `cargo build --locked --offline --all-targets --all-features` | Pass using already-fetched dependencies |
| `.venv-oracle/bin/python -m unittest discover -s tools/oracle -p 'test_*.py' -v` | **54 passed**, no failures/skips |
| `.venv-oracle/bin/python tools/oracle/corpus.py check` | **28 reference replays passed** |
| `.venv-oracle/bin/python tools/oracle/native_check.py --native target/debug/examples/export_sbml --trusted-native` | **16 exact structural export matches + 12 expected rejections**; 0 failed/blocked; **28 numerical NOT_RUN** |
| Same native-check command with `--python /no/such/python` | Expected exit 2: 28 blocked, 0 exported/rejection passes |
| `.venv-oracle/bin/python -m py_compile tools/oracle/*.py` | Pass |
| `cargo package --list --allow-dirty` | Pass; 105 final entries, no cache/virtualenv/artifact paths |

The 16 exported documents retain **199 independent SBML warnings**; these were
not suppressed or counted as dimensional validation. Existing corpus acceptance
and rejection cases remain stage-separated in `tools/oracle/native-evidence.json`.
No-default-feature runs repeat coverage, not additional unique test cases.

Initial test attempts while another worker was changing the shared core failed
to compile (`InitialValue`/`f64` transition). Those were reported as WIP failures,
not passes; the stable final commands above were independently rerun successfully.

### Bounded adversarial evidence

The independent **18-case** initialization probe matrix produced **8 accepted
exact native/reference projection matches + 10 explicit rejections**, with no
accepted mismatch. Exact cases live in
`tests/hardening.rs::frozen_initialization_review_probe_acceptance`: aliases,
diamond dependencies, self/downstream cycles, negative/zero powers, negative-zero
division, intermediate overflow, forward mixed-volume species initialization,
boundary/normal dependency chains, legal reaction cycles, declaration reordering,
missing symbols, deferred compartment expressions, and negative species results.

This reproducible recipe reads the tracked matrix and independently reinspects
accepted SBML; it does not normalize arithmetic or promote native expectations
into reference goldens:

```sh
.venv-oracle/bin/python - <<'PY'
import re, sys
from pathlib import Path
sys.path.insert(0, 'tools/oracle')
import native_check, oracle
source = Path('tests/hardening.rs').read_text()
body = source.split('fn frozen_initialization_review_probe_acceptance()', 1)[1].split('];', 1)[0]
rows = re.findall(r'\(\s*"([^"]+)",\s*"([^"]+)",\s*(true|false),?\s*\)', body)
assert len(rows) == 18
exe = Path('target/debug/examples/export_sbml').resolve()
matched = rejected = 0
for name, text, accepted in rows:
    native = native_check.native_export(exe, text)
    if accepted == 'false':
        assert native['status'] == 'REJECTED', (name, native)
        rejected += 1
        continue
    assert native['status'] == 'EXPORTED', (name, native)
    reference = oracle.run('convert', text)
    inspection = oracle.run('inspect', native['xml'])
    assert reference['status'] == inspection['status'] == 'PASS', name
    assert not native_check.differences(reference['projection'], inspection['projection']), name
    matched += 1
print(matched, 'exact matches;', rejected, 'explicit rejections')
PY
```

Hardening loops are assertions within eight tests, **not extra test counts**:
64 generated numeric models; deterministic insert/delete mutations at UTF-8 scalar
boundaries; 256 seeded 48-character strings exercised twice; every scalar-boundary
prefix of four inputs; 8 threads × 16 distinct models compiled repeatedly; bounded
source/token/entity/diagnostic/depth guards; a 48-symbol forward/reverse dependency
chain with cycle/limit variants. Initialization tests separately exercise a
10,000-edge dependency chain and cycle. This is bounded regression testing, not
measured coverage, prolonged fuzzing, a performance benchmark or formal verification.

### CI validation and limits

`.github/workflows/native.yml` configures Linux/macOS/Windows × Rust 1.96.0/stable,
20-minute jobs, locked build/tests/lint/docs, and offline rebuild after fetching.
It uses read-only repository permissions, no persisted checkout credentials, a
SHA-pinned official checkout action, and no Python/oracle/publishing steps.
YAML parsed locally with Ruby's standard YAML library. **All six remote jobs are
NOT_RUN**; no remote repository or workflow execution was created.

The configuration follows official [GitHub workflow syntax](https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-syntax),
[checkout guidance](https://github.com/actions/checkout), and
[rustup toolchain usage](https://rust-lang.github.io/rustup/basics.html).
Checkout v6 was resolved through its official tag API and pinned at
`d23441a48e516b6c34aea4fa41551a30e30af803`; no third-party toolchain action is used.

## 4. What's Next (Concrete Next Steps)

Stop after this initialization/regression increment. Next separately authorized
work can select one remaining family, such as units or broader initialization
forms, with reference fixtures first. Compartment-expression dependencies need an
explicit amount/concentration/volume design before acceptance. Remote CI, other
platforms/toolchains, broader fuzzing, numerical comparison, release approval and
licensing remain pending. Keep graph documentation current with `graphify update .`
if a codebase graph is adopted.

## 5. Novel Opportunities (Innovation & Monetization)

A small native compiler that retains auditable initialization expressions and
ships a reproducible compatibility ledger can become a useful integration layer
for Rust scientific tooling. The defensible value is explicit semantics and
measured translation evidence—not full-language, simulation, clinical, speed, or
official-endorsement claims. Distribution/commercialization remains subject to
separate licensing and authorization.

## Key Learnings:

1. Evaluation order, source occurrence order and SBML serialization order serve
   different purposes; dependency sorting must not discard original assignments.
2. Initialization cycles are not reaction-network cycles, and boundary species
   affect downstream parameter flags without becoming immutable SBML species.
3. A configured cross-platform workflow is not a passed cross-platform test run.
