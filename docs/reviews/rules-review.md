# Independent assignment and rate rules review (`rules-core-v0.4-slice`)

**Current status: independently signed off — native 4-configuration matrix, 200-case
reference-frozen rules corpus, 2 closed-form numerical rule trajectory models,
core-only API snippet, and all 14 maintained compatibility/numerical gates PASS.**
Profile: `rules-core-v0.4-slice`. Historical receipts (`tools/oracle/conditional-evidence.json`
SHA256 `c042fe437562f512bd2cfdb345cbdb86158b12504af9fc85d75f97792e7a0cbe`,
`tools/oracle/rational-evidence.json` SHA256 `34a6e31980e2682f542c5e54065c318cfd9e86727b8d18c8c99d015496e44bf4`)
and immutable binary directories (`artifacts/conditionals-phase3b/bin/`) are preserved byte-for-byte.

This is a bounded flat-model implementation of species and parameter assignment (`:=`)
and rate (`'=`) rules, not full Antimony compatibility or an official standard.
Structural validity does not imply complete initial state or simulation readiness;
no solver is added to the crate.

## 1. Frozen Native Verification Matrix

All four Cargo feature configurations, strict all-target Clippy (`--all-features` and
`--no-default-features` with `-D warnings`), `cargo fmt --all -- --check`, and
`RUSTDOCFLAGS="-D warnings" cargo doc --all-features --no-deps` passed with zero
warnings, zero failures, and zero ignored tests:

| Feature configuration | Rust unit + integration tests | Doctests | Failed / ignored |
|---|---:|---:|---:|
| `--all-features` (`sbml` + `cli`) | 455 | 2 | 0 / 0 |
| Default (`sbml`) | 410 | 2 | 0 / 0 |
| Core-only (`--no-default-features`) | 395 | 2 | 0 / 0 |
| CLI without SBML (`--no-default-features --features cli`) | 440 | 2 | 0 / 0 |

The all-feature build was frozen once into `artifacts/rules-phase4/bin/` with
`artifacts/rules-phase4/bin/manifest.json` binding all 14 downstream gate runs:

- `antimony` CLI: `cff4c510199d53bb26393fb77a4eda057a54322d23d7d8a0165b2845d7bfd382`
- `export_sbml`: `975814c564b20e9ae3e51d2d975d1f28d3de8705f02b6bb6e8bbf59c6197d92f`
- `format_antimony`: `8f31e394051ed0d156a60d084544f89eef43366449b6fef03fee4c734128aa39`

## 2. Reconciled Contract and Adversarial Audit

The native implementation was audited against the 10 verified discovery findings in
[`docs/RULES_REFERENCE.md`](../RULES_REFERENCE.md) and [`docs/reviews/rules-discovery-review.md`](rules-discovery-review.md):

1. **Distinct `Rule` IR and `<listOfRules>` ownership:** `ValidatedModel::rules()`
   returns `&[Rule]` (`RuleKind::{Assignment, Rate}`, `target()`, `expression()`,
   `source_span()`). Rules are never lowered into `InitialAssignment`s.
2. **Target first-seen ordering:** Rules in `ValidatedModel::rules()` and SBML
   `<listOfRules>` follow the target symbol's first appearance in source (`first_seen_order`),
   not rule-statement or topological order.
3. **Same-kind replacement and retained inferred symbols:** Repeating `x := expr` or
   `x' = expr` replaces the formula while retaining symbols inferred by superseded
   RHS expressions (`a := u; a := 2;` retains parameter `u`). Mixing `:=` and `'=`
   or placing `=` after `:=` rejects with `DiagnosticCode::DeclarationConflict`.
   `:=` clears prior `=` initializers; `'=` retains independent `=` initializers.
4. **Lexical scalar-export classification:** `lexical_rule_scalar` inspects the
   unnormalized parsed `ExprKind` before parenthesis/sign collapsing so only bare
   finite numeric literals (`2.5`) and unary-minus bare numeric literals (`-2`)
   populate `Parameter::value()` / `Species::initial_concentration()` for `:=` targets.
   Grouped `(2.5)`, `+2.5`, `(1/2)`, `true`, `pi`, and general expressions leave the
   scalar attribute as `None`.
5. **Source-order-sensitive species boundary promotion:** An inferred rule target later
   declared `species` (`a := 2; species a;` or inline `species S := 2;`) is promoted
   to `boundaryCondition="true"`, whereas a species declared before its rule
   (`species S; S := 2;`) stays `boundaryCondition="false"` unless marked `const` or `$`.
   A subsequent `var species` declaration clears `boundaryCondition` back to `false`.
   Non-boundary ruled species in reaction stoichiometries reject with
   `DiagnosticCode::DeclarationConflict`.
6. **Bounded iterative transient cycle checking:** `check_transient_cycle` runs an
   explicit work-stack reachability check at each assignment statement whenever rules
   are active, rejecting transient cycles (`a := b; b := a; a := 1;`, `a = b; b := a; a = 1;`)
   immediately (`DiagnosticCode::InitializationCycle`) without recursion.
7. **Oracle projection gaps closed:** `tools/oracle/oracle.py::projection_limits`
   rejects unprojected attributes on `assignmentRule`, `rateRule`, and `algebraicRule`
   (`'Rule metadata is outside projection v1'`), `tools/oracle/conditional_check.py::require_represented_math`
   inspects `projection['rules']` for non-null `math`, and `tools/oracle/rules_check.py`
   enforces target existence, `constant == False`, uniqueness, and absence of dual
   `InitialAssignment` on assignment-rule targets.

## 3. All 14 Maintained Compatibility and Numerical Gates

All 14 gates executed against the frozen binaries in `artifacts/rules-phase4/bin/`
and recorded `status: "PASS"` in `artifacts/rules-phase4/gates/*.json`:

| Maintained gate | Total cases | Verified outcome | Failures / blocked |
|---|---:|---|---:|
| `rules` (`tools/oracle/rules_check.py`) | 200 | 114 exact exports / 86 expected rejections | 0 |
| `rules-numerical` (`tools/numerical/rule_check.py`) | 2 | 2 closed-form ODE/rule trajectories PASS | 0 |
| `conditional` (`tools/oracle/conditional_check.py`) | 262 | 159 exact exports / 103 expected rejections | 0 |
| `piecewise-numerical` (`tools/numerical/piecewise_check.py`) | 1 | 1 continuous-rate piecewise trajectory PASS | 0 |
| `rational` (`tools/oracle/rational_check.py`) | 124 | 73 exact exports / 51 expected rejections | 0 |
| `original` (`tools/oracle/native_check.py --current-profile`) | 90 | 58 exact exports / 32 expected rejections | 0 |
| `formatter` (`tools/oracle/roundtrip_check.py --current-profile`) | 90 | 58 byte-idempotent roundtrips / 32 rejections | 0 |
| `cli` (`tools/oracle/cli_check.py --current-profile`) | 90 | 58 supported CLI / 32 expected rejections | 0 |
| `arithmetic` (`tools/oracle/expression_check.py --current-profile`) | 54 | 48 exact exports / 6 expected rejections | 0 |
| `symbols` (`tools/oracle/symbols_check.py`) | 66 | 54 exact exports / 12 expected rejections | 0 |
| `builtins` (`tools/oracle/builtins_check.py`) | 109 | 66 exact exports / 43 expected rejections | 0 |
| `workload` (`tools/workloads/check.py --current-profile`) | 86 | 74 supported (6 motifs + 68 generated) / 12 rejections | 0 |
| `numerical` (`tools/numerical/check.py`) | 8 | 8 original analytic trajectories PASS | 0 |
| `builtin-numerical` (`tools/numerical/builtin_check.py`) | 2 | 2 builtin/time analytic trajectories PASS | 0 |

In the original 90-case corpus (and formatter/CLI routes), `rejected_assignment_rule`
and `rejected_rate_rule` are intentionally promoted to supported exports via the
hash-bound overlay `fixtures/manifests/rules-core-v4-overlay.json` (moving 56/34 to 58/32
under `--current-profile`), while historical manifests and receipts remain unchanged.

The two new numerical rule anchors (`coupled_rate_and_assignment_rule` and
`boundary_species_assignment_and_rate_rule`) match their hand-derived closed-form
solutions and invariants across all 101 grid points on $[0, 10]$ with zero differential
error between native and reference SBML exports.

## 4. Python Suites and Core-Only API Verification

- **Python unit tests:** **467 passed, 0 failed, 0 skipped** (`282` in `tools/oracle`,
  `103` in `tools/numerical`, `51` in `tools/workloads`, `31` in `tools/verification`).
- **Core-only Rust API snippet:** Extracted from [`docs/RULES_API_NOTES.md`](../RULES_API_NOTES.md)
  and executed with `default-features = false`, `--locked --offline` in
  `artifacts/rules-phase4/api-guide/receipt.json` (`exit_code: 0`).

**Independent signoff: PASS for `rules-core-v0.4-slice`.** Next recommended increment:
Phase 5 user-defined functions (`function ... end`).
