# Independent flexible-expression review

Date: 2026-10-14. Scope: **`arithmetic-v0.1-slice`**, Phase 1 of the
[flexible modeling plan](../FLEXIBLE_MODELING_PLAN.md). This is not completion of
that plan, full Antimony support, an upstream endorsement, or a release.

## What this increment enables

Users can write nested arithmetic, reference-valid unary sign combinations and
small/large scientific literals in ordinary source files or stdin. The existing
native library and CLI remain unrestricted by fixture identities. Formatting
preserves source tokens, including parentheses and numeric spellings; semantic
analysis follows the pinned Antimony arithmetic representation instead of
requiring users to manually fit an artificially narrow grouping grammar.

The numeric range is still deliberately bounded: at most 15 significant digits,
zero or magnitude **[1e-307, 1e308]**. Nonzero literal underflow is rejected rather
than replaced with zero. This is not arbitrary-precision or all-number support.
Scientific MathML uses source-derived decimal mantissa/exponent; expression-value
queries use the corresponding binary64 interpretation. That interpretation can
differ by one ULP from parsing the complete decimal directly. Scalar and unit
policies are not silently replaced by expression policies.

## Defects caught and resolved during independent review

A first implementation passed ten reviewer-authored Rust tests but failed eight
of 63 independently authored source models. It normalized binary subexpressions
too early. Upstream constructs n-ary groups first, and its treatment of grouping
is contextual:

- `a+(b+c)` normalizes to `(a+b)+c`.
- `(a+(b+c))+d` retains the inner `b+c` group.

This is not merely cosmetic. `(1e16+(-1e16+1))+1` evaluated to **2** under the first
implementation but to **1** under the reference-normalized tree. A multiplication
variant exposed a corresponding underflow difference. The revised implementation
constructs groups before normalizing them and feeds the same bounded result to
initialization, unit analysis, public model queries and SBML export. The exact
comparison policy was **not weakened** to hide mismatches.

Another review finding concerned comment removal. Upstream accepts spaced or
comment-separated negative signs (`- -2`, `-/* comment */-2`) but rejects contiguous
`--2`. Formatting now retains the separator instead of creating invalid source.
Newline-separated unary operands remain rejected. Signed-zero expression results
are checked against the exported literal interpretation.

## Verified results

### Native feature matrix

All commands used the existing local Rust toolchain and dependency cache, with
`--offline --locked`. This was not a clean-machine install or hermetic build.
For each feature configuration, the reviewer executed `cargo build --all-targets`,
`cargo test --all-targets`, `cargo test --doc`, and
`cargo clippy --all-targets -- -D warnings`; `cargo fmt --all -- --check` was also run.

| Features | Unit/integration tests | Doctests | Build | Strict Clippy |
|---|---:|---:|---|---|
| Default (`sbml`) | 229 passed | 2 passed | PASS | PASS |
| All features | 274 passed | 2 passed | PASS | PASS |
| No default features | 221 passed | 2 passed | PASS | PASS |
| CLI without SBML | 266 passed | 2 passed | PASS | PASS |

**17 commands passed; zero failed tests or ignored tests.** These configurations
repeat many tests and must not be added together as distinct test cases. The
13 independent regressions in `tests/expression_independent.rs` are included in
these counts, not additional to them. They exercise grouping-sensitive numerical
results, precedence, comment/sign formatting, extreme literals, 2,000-digit
exponent spellings, signed zero, resource limits and arbitrary model names.

The reviewer fingerprinted **46 Cargo/source/test/example files** before and
after this matrix: all hashes were unchanged. The local command/log/hash record
is `artifacts/flexible-expressions-independent/matrix.json`, SHA-256
`e597fd5a69cc0ab4d249821d6c030d25bcec566ca69e2d21dbc0420dc05ac724`.
This is a scoped review record, **not** regeneration of the earlier whole-project
current-source receipt, dependency inventory or toolchain provenance.

### Open-ended independent source exercise

The reviewer authored 63 source models without selecting fixture IDs, then added
128 fixed-seed depth-four expression constructions. The final **191 sources**
produced:

- **183 supported exports:** exact whole-model reference projection match,
  independent libSBML consistency checking without errors, format idempotence,
  and identical native export before/after formatting.
- **8 expected syntax rejections:** both reference and native reject accidentally
  contiguous `--` operator sequences in the generated expressions.
- **Zero unexpected failures.** The original 63 are included in the 191.

Reference environment: Antimony **3.2.0**, python-libSBML **5.21.2**. This is a
bounded, gap-targeted structural sample, not a compatibility percentage, fuzz
coverage claim, real-model corpus or trajectory benchmark. It is separate from
the maintained 54-case arithmetic gate; conceptual overlaps must not be counted
as wholly distinct scientific coverage.

Final local results: `artifacts/flexible-expressions-independent/expanded-models.json`.
Initial eight failures remain recorded in `open-models-initial-failures.json`.
The final gate used the copied native CLI at
`artifacts/arithmetic-phase1/bin/antimony`, SHA-256
`8cba492f9fffbdd7f4e0682262f6c9a6cfec423778182e4adbd6bba73f66af5c`.
The same directory's `manifest.json` identifies stable CLI, exporter and formatter
copies for subsequent reference gates. Build outputs were restored to all-feature
configuration after the matrix; no feature-dependent binary swapping is assumed
safe while evidence tools execute.

### Maintained gates rerun by the evidence owner

The evidence owner subsequently executed six gates against those same three
copied artifacts; this reviewer inspected `artifacts/arithmetic-phase1/gate-run.json`
and the reports. All six exited successfully, and all binary hashes were unchanged:

- New arithmetic gate: **54 cases**, 47 supported exports and 7 expected rejections.
- Existing native, formatter and CLI gates: **90 cases each**, the same 48
  supported and 42 rejected sources replayed through three interfaces—not 270
  distinct models.
- Existing numerical gate: **8 selected analytic models** passed again.
- Existing workload gate: **86 structural workloads** passed again.

The evidence owner also reports **162 Python tests passed**. Its intentionally
missing-reference run correctly returned a failing aggregate with **54 blocked
cases and zero passes**. The new arithmetic 54-case gate, original 90-case corpus
and 86 workloads retain numerical `NOT_RUN`; the eight analytic anchors are a
separate, bounded numerical result, not trajectory coverage for the new syntax.
Detailed reports are `artifacts/arithmetic-phase1/*-report.json`. These reruns do
not regenerate the older whole-project source receipt or make its hashes current.

## Quality and safety assessment

Inspection found no fixture/source-name allowlist, native Python/reference call,
new normal dependency, unsafe-code allowance or reduction in resource ceilings.
The native core remains separate from development-only reference tooling.
Compared with the pre-increment packaged source, Cargo dependencies, configured
limits and unit-mapping/unit-analysis modules are unchanged. Expression
normalization is iterative, does not duplicate subtrees, and checks normalized
depth in addition to parser depth; the hard depth ceiling remains 256. Bounded
source/token/entity limits and validated-model export gating remain in force.
The formatter checks generated output again rather than returning unchecked text.

Residual risks remain: exact compatibility beyond these cases is not proven;
reference normalization and binary64 interpretation are intentionally documented
rather than advertised as source-parenthesis IEEE evaluation guarantees. No new
trajectory validation, cross-platform run, clean-machine packaging assessment or
full source receipt was performed by this review. Existing historical evidence
must remain explicitly historical unless rerun.

## Next steps and opportunities

Proceed to the separately scoped symbol/declaration/unnamed-reaction foundation;
then builtins/time, conditions and genuine runtime rules. Missing initial values
must not become invented zeros, and rules must not become initial assignments.
Those capabilities are still deferred, so the broad independent-testing milestone
is **not yet complete**. The shared normalized expression arena is a useful base
for future model inspection and editor tooling, without embedding a simulator.
