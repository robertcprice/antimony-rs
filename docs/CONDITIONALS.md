# Conditional expressions: bounded native slice

**Verified bounded checkpoint (2026-10-14).** The implemented profile is
`conditional-core-v0.3-slice`; all twelve maintained gates passed on immutable reviewed binaries.
This extends the separately bounded arithmetic, symbols, numeric builtins and
rational slices, not the complete Antimony language or frozen v0.1 target.

## What you can write

- Boolean leaves: `true`, `True`, `TRUE`, `false`, `False`, `FALSE`.
  Exactly `True()`, `TRUE()`, `False()`, `FALSE()` are additional zero-argument
  aliases. Lowercase calls and every nonzero-argument Boolean call reject.
- Binary `< <= > >= == !=`; direct `eq/equals/gt/geq/lt/leq` calls with at
  least two operands, and `neq` with exactly two. N-ary calls compare successive
  pairs, not Boolean left folds.
- Symbolic `! && ||`, functional `not(x)`, `and(...)`, `or(...)`.
  Empty `and()` is true and empty `or()` false, retaining their AST identities.
- Ordered `piecewise(value, condition, ..., otherwise)`, with optional otherwise;
  otherwise-only `piecewise(value)` is supported.

`&&` and `||` have equal precedence and associate left. An outer infix logical
operator appends to a same-kind left node (even one made by a function), but an
outer function does not flatten nested arguments; right nesting is retained.
Comparison-chain syntax is deliberately excluded, including normalized left
comparison nodes or ANDs with two or more direct comparison children. Use explicit
functional comparisons or logical composition rather than chains.

## Initialization is conservative, not a solver

Finite Boolean/numeric conversion uses 1/0 and nonzero/zero. Unset symbols and
`time` remain unknown; unknown is never guessed to mean zero or false.
Piecewise evaluates conditions in order and only the selected value. Logical
AND/OR evaluate sequentially, stopping at a decisive Boolean or the first unknown.
A first unknown returns `None` even if a later operand would decide the result.
Comparisons demand every argument and still reject known domain errors beside
unknown operands. Known no-match piecewise without otherwise rejects an initializer,
but remains structurally representable in a reaction law.

All branches still participate in reference resolution, cycles, units and finite
literal bounds. Lazy domain evaluation is not permission for an inactive cycle,
unit conflict, nonfinite literal or zero-denominator rational atom.
Bare numeric and rational literals have **Unspecified** dimensions; Boolean results
are known dimensionless. See the [historical prose erratum](MATH_UNITS_ERRATUM.md).

## Try your own ordinary file

```antimony
model conditional_input
  species S = 0;
  J: => S; piecewise(time, time < 5, 5);
end
```

Save as `conditional_input.ant`, then use the general CLI (not a fixture dispatcher):

```sh
cargo run --features cli --bin antimony -- check conditional_input.ant
cargo run --features cli --bin antimony -- format conditional_input.ant
cargo run --features cli --bin antimony -- export conditional_input.ant
```

The crate parses, analyzes, formats and exports SBML; it does **not** integrate
ODEs. The separate one-model numerical gate tests the implicit-model form of this
continuous rate: `S(t)=t²/2` until 5 and `5t−12.5` thereafter, on 101 points from
0 to 10 including 5. Both rate and state are continuous at the join. It proves
neither discontinuous-input behavior nor event timing or arbitrary model dynamics.
Unspecified physical units may still produce reference warnings.

## Explicit exclusions and evidence

Textual infix/prefix logical aliases, infix comparison chains, XOR/implies,
rules/events, user functions, delay, hierarchy and compartment expressions remain
outside this slice. Existing scalar precision/rational restrictions still apply.
The core-only [Rust API guide](CONDITIONAL_API_NOTES.md) demonstrates typed node
inspection and optional initializer values. The [reference note](CONDITIONALS_REFERENCE.md)
records discovery, not a substitute for completed native gates.

The frozen structural inventory contains 248 cases plus a separately hash-bound
14-case Boolean-call supplement: 159 positive exports and 103 expected negatives.
Positive evidence requires non-null exact ASTs and the closed MathML vocabulary;
reference load success with null math earns no supported-export credit.
The twelve passing maintained gates retain all prior structural/numerical corpora,
add this conditional inventory and one independent analytic anchor. Historical
receipts and original source/expected snapshots remain unchanged.

Verification is recorded in the [independent review](reviews/conditionals-review.md)
and [compact evidence](../tools/oracle/conditional-evidence.json). Python suites passed
**441 tests** (267 oracle, 92 numerical, 51 workloads, 31 verification), zero
failures/skips. The 17-command native matrix passed: all-feature 446, default 401,
core-only 386 and CLI-only 431 tests, plus two doctests in each configuration.
These are overlapping configuration runs, not distinct test inventories. All 64
source hashes and the three reviewed binaries remained unchanged. The named
ordinary-file example above separately passed exact reference projection, no
libSBML Error/Fatal, byte-idempotent formatting and identical reexport.

| Maintained gate | Verified result |
|---|---|
| Conditional + alias supplement | 262:159 exact exports,103 expected rejections |
| Rational corrective | 124:73 exact,51 rejections |
| Original / formatter / unified CLI | 90 each:56 supported,34 rejections |
| Arithmetic | 54:48 exact,6 rejections |
| Symbols | 66:54 exact,12 rejections |
| Builtins | 109:66 exact,43 rejections |
| Workloads | 86:74 exact,12 rejections |
| Retained analytic / builtin-time / continuous piecewise | 8/8,2/2,1/1 |

Structural inventories retain numerical NOT_RUN; their sizes overlap and must
not be summed as unique-model or dynamics coverage.

## Reproduce the bounded checks

Development reference environments are pinned separately; no runtime Python or C++
dependency is introduced into the Rust core. With trusted, prebuilt binaries:

```sh
export ANTIMONY_IMPLEMENTATION_PROFILE=conditional-core-v0.3-slice
.venv-numerical/bin/python tools/oracle/conditional_check.py \
  --trusted-native --cli artifacts/conditionals-phase3b/bin/antimony
.venv-numerical/bin/python tools/numerical/piecewise_check.py \
  --trusted-native --native artifacts/conditionals-phase3b/bin/export_sbml
```

The original/arithmetic/formatter/CLI/workload gates additionally require
`--current-profile` to opt into the hash-bound current overlay. Unknown identities
and missing/wrong overlays do not earn PASS. Old `.2`/`.2.1` configurations remain
explicitly selectable for their historical binaries; the conditional gate itself
requires `.3`. The old full-source receipt runner hardcodes earlier inventories
and cannot certify this extension. This milestone does not claim hermetic build
reproduction, official upstream endorsement, publication or a licensing decision.
