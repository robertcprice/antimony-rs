# Independent numeric builtins review

**Status: source-frozen native matrix, independent model review and all nine maintained evidence gates PASS.**

## What this increment enables

The proposed `math-core-v0.2-slice` adds typed numeric calls, mathematical constants
and symbolic time to the existing arbitrary-input native pipeline. This is not
conditions/piecewise, user-defined functions, runtime assignment/rate rules, a
simulator, or full Antimony. The implementation does not gain conformance credit
merely by accepting a file or successfully formatting it.

The reviewer previously implemented Phase 2 symbols but did not implement this
Phase 3A native change. Reviewer ownership is limited to
`tests/builtins_independent.rs` and this review, plus local scratch probes/logs.
A separate native owner implements the code; an evidence owner maintains the
reference harness; Agy supplies a separate ordinary-authoring test file and design
advice. Agy test execution is performed locally, not assumed from generated code.

## Independent contract investigation

`artifacts/builtins-phase3/independent/reference.json` retains **65 original,
gap-targeted reference probes: 45 reference loads accepted and 20 rejected** using
Antimony 3.2.0 and libSBML 5.21.2. These counts are not a compatibility percentage.

Confirmed source semantics include unary `log` as natural log, binary
`log(base, value)`, `log10`, `ceil`/`ceiling`, `pow`/`power`, one-or-more argument
`min`/`max`, constants `pi`/`exponentiale`, and equivalent `time`/`time()`.
The native boundary deliberately rejects empty min/max and demonstrably invalid
finite-real initial calculations, even though upstream may retain them symbolically.

A notable reference hazard: `time(2)` loads upstream but exports an
InitialAssignment without MathML. Load success alone is therefore insufficient
conformance evidence. Native rejection is retained; no equivalence credit is
claimed for that case. Actual newlines inside function argument lists reject;
ordinary source comments remain lexical trivia.

## Adversarial native tests

The reviewer authored **21 Rust tests** covering:

- Typed alias resolution, constants, symbolic time and no fabricated parameters.
- Analytic initialization values with retained InitialAssignments.
- Context-sensitive sum/product grouping inside call arguments.
- Unknown inputs without algebraic shortcuts or invented zero.
- Known-invalid domain subtrees despite unknown siblings, including logarithm
  base/value constraints and finite intermediate overflow.
- Initialization cycles, malformed arity/arguments and reserved namespaces.
- Reaction modifier collection, source-order generated-ID reservation and
  mutability propagation through call arguments.
- Conservative dimensional knowledge, provable conflicts, square roots and
  dimension-preserving operations.
- Depth, width, token/entity limits and located malformed-input diagnostics.

The implementation owner discovered a recursive-parser stack overflow when a
caller raised the configured depth ceiling. The fix replaces recursive Pratt
execution with an explicit continuation stack rather than silently lowering the
published depth limit. The reviewer independently exercises 3,000 nested calls,
parentheses, unary operators, right-associative powers and mixed contexts with a
configured depth of 10,000. Each rejects with `ResourceLimit` in a subprocess
without increasing its test-thread stack; a 96-call control succeeds. A subprocess
isolates any future stack-overflow abort so it becomes a reported test failure.

The focused **21/21 tests pass**, zero failures/ignored tests, and are included
in the final source-frozen matrix below (20 run without SBML). Logs are under
`artifacts/builtins-phase3/independent/`.

## Independently authored whole-model exercises

A separate original-source script combines supported builtins, initialized and
unset operands, nested calls, contextual arithmetic and reaction/time models.
Across **203 inputs**, the final immutable CLI yields **164 exact
reference-matched exports and 39 expected rejections**, with no unexpected outcomes.
Each positive also checks independent libSBML validation, supported projection
vocabulary, formatting idempotence and re-export identity. The same outcomes were
observed after the nonrecursive-parser rewrite and again using final immutable
artifacts. The prior **191 independent arithmetic inputs** also replayed with
**183 exact exports and eight expected rejections**, preserving historical
precedence/grouping behavior after the shared parser rewrite.

These exercises use the ordinary CLI; neither native input nor expectations are
restricted by fixture IDs. They are a deliberately targeted sample, not fuzzing,
published-model validation, random coverage estimation or trajectory validation.

## Final source-frozen native matrix

**17 commands passed**, with zero failed/ignored tests and **54 Cargo, source,
test and example file hashes unchanged** before and after the matrix. Commands
ran offline and locked using the existing local toolchain and dependency cache,
with an isolated review target directory. This is local macOS verification, not
remote multi-platform CI or hermetic/reproducible compilation.

| Feature configuration | Unit/integration tests | Doctests | Build and Clippy |
|---|---:|---:|---|
| All features | 365 passed | 2 passed | PASS |
| Default | 320 passed | 2 passed | PASS |
| Core-only | 309 passed | 2 passed | PASS |
| CLI without SBML | 354 passed | 2 passed | PASS |

Each configuration executes build/test/Clippy with `--all-targets`; Clippy uses
`-D warnings`. Doctests run separately. `cargo fmt --all -- --check` is the
seventeenth command. Full argv, elapsed times, log hashes and source inventories
are retained in `artifacts/builtins-phase3/independent/matrix.json`.

Agy's **14 ordinary-authoring tests** were actually run and pass with and without
SBML. Its initial unused import and core-only borrowed-temporary helper errors
were corrected before the matrix; test coverage was not removed. The native
owner's 17 builtin tests and reviewer's tests are included in the totals, not
additional counts to add again.

After the matrix, one all-feature build generation supplied exclusive immutable
copies at `artifacts/builtins-phase3/bin/`. Those paths are never overwritten after
handoff. The manifest retains:

- CLI: `8e48d4ed7e41c4c57cac0275d47e173cb91c84c7a02f500185b633977f376bae`
- SBML example: `521f3be8a3ce29d89b4db23ac67bd9f8d3fc38578cac73f42a84ab07388a2388`
- Formatter example: `28efb97ba753e08f5a68ae9547eac604630cc0d5b97b95df3e8bc18e64d08e21`

The final 203-model and 191-arithmetic reports record this same CLI hash.
Independent inspection rehashed all three binaries, rechecked all 54 native source
inputs, verified per-report artifact identity, inspected every case outcome and
confirmed no report recorded changed inputs. The inspection receipt is
`artifacts/builtins-phase3/independent/gate-inspection.json`.

## Maintained gates and their precise scope

| Gate | Verified outcome |
|---|---|
| Original 90-case native gate | 56 exact exports; 34 expected rejections |
| Same 90 through formatter | 56 exact/idempotent roundtrips; 34 expected rejections |
| Same 90 through CLI | 56 supported; 34 expected rejections |
| Arithmetic 54-case current profile | 48 exact exports; six expected rejections |
| Symbols 66-case gate | 54 exact exports; 12 expected rejections |
| Numeric builtins 109-case gate | 66 exact exports; 43 expected rejections |
| Structural workloads | Six motifs + 68 generated positives; 12 expected rejections |
| Historical analytic models | Eight passed |
| New builtin/time analytic models | Two passed |

These overlap; they must not be summed as distinct validated biological models.
All structural gate cases retain numerical `NOT_RUN` status. Separately,
**370 Python harness tests passed** with zero skips: 208 oracle/CLI, 80 numerical,
51 workload and 31 source-receipt tests. The historical full source receipt was
not regenerated and is not current-source proof.

### Transparent correction of initial gate assumptions

The first maintained run had two failing suites, not nine successes. Historical
arithmetic `deferred_builtin` still expected rejection of now-supported `sin`;
an explicit current-profile overlay fixes that expectation without altering the
original source, snapshot or manifest. Five new builtin positives accidentally
used retained unsupported constructs: empty-both-sides reactions, a negative
initial species concentration, or redefinition of builtin `seconds` units.

Their original sources and reference snapshots remain unchanged as five negative
neighbors. Five separately authored supported companions add a genuine product
species, use a negative **parameter** in an unevaluated runtime rate, or define a
non-reserved custom unit. The reviewer independently approved all five against
stated profile scope and live reference parsing/libSBML validation before the
final rerun. Initial failed reports and the original 104-case manifest are
preserved in `gates-initial/` and `corpus-initial/` beneath the phase artifact
directory. No native behavior was weakened or expanded to turn these gates green.
A structurally accepted rate such as `sqrt(q)` with `q=-1` still does not certify
runtime validity or simulation readiness.

### Two bounded numerical anchors

The new development-only trajectories independently check:

- `dS/dt = exp(-t)`, `S(0)=0`, against `S(t)=1-exp(-t)`.
- `dS/dt = 1+sin(t)`, `S(0)=0`, against `S(t)=t+1-cos(t)`.

Each native/reference path runs at 101 requested output times from 0 to 10 using
the pinned CVODE/RoadRunner development stack. Both paths agree exactly with each
other in this run and pass the separate analytic tolerances; maximum analytic
absolute errors are approximately `2.63e-9` and `2.68e-8`. Fourteen libSBML warnings
per path are retained. This validates these two exp/sin/time source models only,
not every builtin/domain, complete units, or the general modeling language.

The ordinary-user input-plus-loss example for the builtin guide also passed an
independent exact-reference projection, libSBML validation, format idempotence and
identical re-export. Its separate receipt is `independent/user-model-review.json`;
it has **numerical NOT_RUN**, because it is not either analytic anchor above.

## Architecture and safety inspection

Read-only review traced new call/constant/time variants through parsing,
normalization, initialization dependencies and evaluation, source-order symbol
inference, dimensional analysis, public IR, reaction modifiers and SBML emission.
Call arguments use the shared normalization/evaluation tree, not raw-string math.
Core source contains no fixture-ID dispatch, oracle invocation, host-function
execution or new filesystem/network dependency. The existing optional CLI alone
owns file I/O. Unknown values remain absent or symbolic.

The native owner's [supplemental architecture audit](builtins-architecture-review.md)
is useful implementation evidence, **not an independent review**. Later lazy
conditionals cannot simply reuse the current eager numeric-call evaluator: inactive
branches require their own semantics and verification.

## Practical implications and next step

Typed calls and a stack-safe shared parser provide foundations for later
conditions and reusable functions, but those features require separate semantics
and verification. Preserve the present bounded, explicit contracts; do not market
this increment as the official Antimony Rust standard.
