# Bounded structural workloads and deterministic differential grid

**Status: independently verified: 6 motifs + 64 generated supported cases + 16
required rejections; 51 workload Python tests.** The separate current-source
receipt records the final build/gate identities. No
additional numerical, full-language or fuzzing claim is made by this gate.
See the [execution plan](RELIABILITY_PLAN.md), [six model contracts](WORKLOAD_MODELS.md),
[current-source guide](SOURCE_EVIDENCE.md) and [review](reviews/workloads-review.md).

## Separate evidence populations

- **Six original structural motifs** exercise enzyme binding/catalysis, branching,
  coupled transport/catalysis, rational feedback, reversible exchange and a
  unit-annotated forward-initialization sequence. They are repository-authored
  examples, not externally curated biological models or official SBML test cases.
- **80 generated cases** are a finite ordered parameter grid: 64 supported cases
  (16 each for initialization DAGs, compartments, units and numeric forms) and
  16 required rejections (four each for cycles, dimensional conflicts, uninitialized
  parameters and the strict scalar serialization envelope).
- The previous **90 language cases** and **eight analytic models** remain separate.
  All 86 new structural cases explicitly retain `numerical: NOT_RUN`.

This is deterministic structured differential testing, **not coverage-guided
fuzzing**, random sampling or evidence that all combinations were explored.
Identifier/header/trivia differences do not establish scientific independence.
The recorded finite dimensions—not the word “generated”—define coverage.

## Independent expectations

`generate.py` emits source from a bounded recipe and does not import the SBML
expectation builder. `generated_contracts.py` independently encodes the reviewed
model structure. `motifs.py` contains independently authored complete motif
projections. Neither builder consumes Rust outputs or captures live reference
results as expected truth. Separate review checks these contracts.

Every supported case must independently match its manual contract on four paths:
original reference SBML, original native SBML, formatted-source reference SBML,
and formatted-source native SBML. Formatting twice must be byte-identical.
Native warning/informational-note sequences before/after formatting must agree; independent libSBML
warnings remain visible. Two equally wrong exporters cannot pass by agreeing.

Full projections include entities, initial scalar versus InitialAssignment mode,
first-appearance assignment ordering, expression binding, compartment membership,
boundary/constant flags, parameter units, ordered unit components, and absence of
unmodeled rules/events/packages. No algebraic simplification, floating-point
rounding tolerance or ordering normalization is introduced.

Specific pinned serialization facts are explicit, not general canonicalization:
`a=(-2)` is an InitialAssignment containing integer `-2`; `a=-2` is a scalar.
The two 16-significant-digit **negative** inputs are rounded by upstream SBML
serialization to `+/-1.23456789012346`. Native export must reject them with
`syntax/unsupported_feature`; accepting the rounded value would fail this gate.
The subnormal `1e-308` is deliberately outside this first grid because its pinned
reference MathML uses an attribute outside the existing projection vocabulary.
It is not silently converted into a passing projected model.

For negative cases, any rejection is insufficient. Both exporter and formatter
must reject with the predeclared native stage/code, leave no output artifact,
and agree with the separately specified reference status:

| Class | Reference status | Native stage/code |
|---|---|---|
| Initialization cycle | REFERENCE_REJECTED, reviewed loop diagnosis | resolve / initialization_cycle |
| Conflicting dimensions | PASS with explicit structural contract | resolve / units_inconsistency |
| Uninitialized parameter | PASS with unset-value contract | resolve / unsupported_feature |
| Scalar envelope | PASS with pinned scalar contract | syntax / unsupported_feature |

These earn rejection-test credit only, never supported-model credit.

## Try one original motif

```sh
cargo build --locked --features cli --bin antimony
target/debug/antimony check fixtures/workloads/enzyme_binding_catalysis.ant --json
target/debug/antimony export fixtures/workloads/enzyme_binding_catalysis.ant -o target/enzyme.xml --json
```

This checks/exports the original binding-and-catalysis motif; it does not simulate
it or certify a biological mechanism. Its independently authored structure is in
the [model contracts](WORKLOAD_MODELS.md).

## Reproduction and identity

Generator version is `antimony-structured-grid-v1`, root seed label
`2026-10-10-grid-01`. There is **no PRNG**: explicit family/index combinations
select a finite parameter grid. Each case records a SHA-256 seed identity derived
from version, root label, family and index, and an exact UTF-8 source hash.
`tools/workloads/generated-v1.json` freezes the ordered 80-case inventory.
Regeneration must agree exactly; it never silently overwrites the receipt.

```sh
# Print proposed deterministic inventory; review before updating a frozen receipt.
python3 tools/workloads/generate.py
python3 -m unittest discover -s tools/workloads -p 'test_*.py' -v
# Binaries must already exist; the checker never invokes Cargo or installs tools.
.venv-oracle/bin/python tools/workloads/check.py \
  --native target/debug/examples/export_sbml \
  --formatter target/debug/examples/format_antimony --trusted-native
```

For the source-bound build and all separate evidence gates, use a **new** output
directory (the runner refuses to overwrite an existing run):

```sh
.venv-oracle/bin/python tools/verification/run.py --execute \
  --target-dir target/independent-cli-review \
  --output-dir artifacts/my-current-source-run
```

Freeze source inputs first; see [the full protocol and limitations](SOURCE_EVIDENCE.md).

The orchestrator accepts only this owned inventory, sources no larger than 8 KiB,
and explicit trusted prebuilt binaries. Existing adapters use fresh temporary
working directories, no shell and 30-second per-process deadlines. Reference
execution requires pinned Antimony 3.2.0 and python-libsbml 5.21.2. Missing tools
are blocked, not success; crashes, timeouts, unknown projection constructs and
unexpected statuses fail the case. State isolation is **not an OS sandbox**.

The gate fingerprints both executables, all relevant checker/contract modules,
profile, inventories and motif sources before preparing cached sources and again
after preparation/execution; input changes or deletion
fail the gate. The separate current-source build runner supplies exact paths from
its recorded build invocation (which may reuse cached artifacts); a binary digest
alone does not prove source-to-binary linkage.

## Failure retention and reduction boundary

A failed case records original source, independent expected contract, seed/recipe,
available native/reference SBML, formatted source, subprocess diagnostics and
fingerprints in a fresh directory under ignored `artifacts/workloads-failures/`.
The report retains its path/hash. Retention is bounded to 20 failures per run and
32 MiB per bundle; exhausted budgets and write failures are explicit, not silent.
No failure automatically edits the original 90 cases, the eight analytic fixtures
or the frozen generated inventory.

**Automatic minimization is NOT_RUN/planned.** Blind statement deletion can remove
an initialization dependency, change first-appearance order or turn a semantic
mismatch into an unrelated syntax error. Before implementing reduction, require
regenerated independent expectations, preserved reference prerequisites and the
same classified stage/structural discrepancy under the same pinned tools. Retain
both original and reduced sources plus the reduction trace. Existing tiny finite
recipes already provide bounded reproductions without claiming minimization.

## Limits and next work

This batch does not exercise general units algebra, imports, rules, events,
functions, dynamic compartments, unsupported grammar, stochastic simulation or
cross-platform behavior. Representative external models still require provenance
and explicit feature eligibility. Broader generation should expand reviewed
parameter dimensions and independent contracts before raising case counts.
