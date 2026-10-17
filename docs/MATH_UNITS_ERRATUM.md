# Erratum: dimensions of bare numeric and rational literals

**2026-10-14 — documentation correction, not a runtime behavior change.**

The captured `math-core-v0.2.1-slice` profile and rational evidence guide contain
an incorrect descriptive sentence saying a rational atom is dimensionless.
The implemented and independently tested contract is:

- A **bare numeric or rational literal** has `DimensionalStatus::Unspecified`.
  Unspecified dimensional knowledge must not be treated as known dimensionless.
- An explicitly annotated quantity obtains dimensional information from its
  annotation and the existing conservative inference rules.
- The separately approved conditional increment makes Boolean results and finite
  Boolean-to-numeric conversion dimensionless. That new conditional contract is
  not evidence that ordinary numeric/rational leaves were already dimensionless.

## Existing API and executed test evidence

`ValidatedModel::expression_dimensions(ExpressionId)` returns an
`Option<DimensionalStatus>`. The independent test
[`rational_dimensions_remain_unspecified_like_bare_numeric_literals`](../tests/rational_independent.rs)
analyzes `a=(1/2);x=2;x has second;b=(1/2)*x;` and explicitly asserts that the
rational initializer of `a` is `Some(DimensionalStatus::Unspecified)`. It also
checks that incompatible known units elsewhere still reject.

The [`assert_rational_atom` helper](../tests/rational_ergonomics.rs) independently
asserts the same dimensional status when checking rational public IR nodes.
[`tests/rational_literals.rs`](../tests/rational_literals.rs) contains another
explicit `Unspecified` assertion. These tests were included in the independently
executed Phase 3A.1 matrix: all-feature400, default355, core342 and CLI-only387
unit/integration tests, plus two doctests per configuration, all passing.
The [matrix receipt](../artifacts/rational-phase3a1/independent/matrix.json) pins
the exact tested source, and the [all-feature log](../artifacts/rational-phase3a1/independent/all-tests.log)
records the executions. These are prior verified observations, not newly run
conditional tests or a new build claim.

## Why the historical files remain unchanged

The captured [profile](../profiles/math-core-v0.2.1.json),
[guide](RATIONAL_CALLS_EVIDENCE.md) and [evidence receipt](../tools/oracle/rational-evidence.json)
remain historical bytes. Rewriting them would obscure what was actually recorded.
Their erroneous sentence is superseded by this erratum; the already implemented
and tested dimensional behavior has not changed. The new conditional profile
links this correction and states the conservative distinction explicitly.
