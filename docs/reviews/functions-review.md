# Independent user-defined functions review (`functions-core-v0.5-slice`)

**Current status: independently signed off — native 4-configuration matrix, 164-case
reference-frozen functions corpus, 2 closed-form numerical function trajectory models,
core-only API snippet, and all 16 maintained compatibility/numerical gates PASS.**
Profile: `functions-core-v0.5-slice`. Historical receipts (`tools/oracle/rules-evidence.json`
SHA256 `9dc74ab8eed565e6059a5504ea3e3df6c49c181f2853259147c1a6445f22daef`,
`tools/oracle/conditional-evidence.json` SHA256 `c042fe437562f512bd2cfdb345cbdb86158b12504af9fc85d75f97792e7a0cbe`,
`tools/oracle/rational-evidence.json` SHA256 `34a6e31980e2682f542c5e54065c318cfd9e86727b8d18c8c99d015496e44bf4`)
and immutable binary directories (`artifacts/conditionals-phase3b/bin/`, `artifacts/rules-phase4/bin/`)
are preserved byte-for-byte.

This is a bounded flat-model implementation of declaration-ordered non-recursive
user-defined functions (`function name(params) body end`), not full Antimony
compatibility or an official standard. Structural validity does not imply complete
initial state or simulation readiness; no solver is added to the crate.

## 1. Frozen Native Verification Matrix

All four Cargo feature configurations, strict all-target Clippy (`--all-features` and
`--no-default-features` with `-D warnings`), `cargo fmt --all -- --check`, and
`RUSTDOCFLAGS="-D warnings" cargo doc --all-features --no-deps` passed with zero
warnings, zero failures, and zero ignored tests:

| Feature configuration | Rust unit + integration tests | Doctests | Failed / ignored |
|---|---:|---:|---:|
| `--all-features` (`sbml` + `cli`) | 459 | 2 | 0 / 0 |
| Default (`sbml`) | 414 | 2 | 0 / 0 |
| Core-only (`--no-default-features`) | 399 | 2 | 0 / 0 |
| CLI without SBML (`--no-default-features --features cli`) | 444 | 2 | 0 / 0 |

The all-feature build was frozen once into `artifacts/functions-phase5/bin/` with
`artifacts/functions-phase5/bin/manifest.json` binding all 16 downstream gate runs:

- `antimony` CLI: `3a9af801b4e03919fbb13b9649a1a0c2f1c86b82af87495fcf56719205118bd2`
- `export_sbml`: `d1dde4a39e374db741d44bb463f242e69efd17c9240f583bb781e0977a97d4cc`
- `format_antimony`: `73afedc3b0866968c21228658a927bf57c0f3d7cc1327be3baa9de4d6ca6e38f`

## 2. Reconciled Contract and Adversarial Audit

The native implementation was audited against the reference findings in
[`docs/FUNCTIONS_REFERENCE.md`](../FUNCTIONS_REFERENCE.md) and [`docs/reviews/functions-discovery-review.md`](functions-discovery-review.md):

1. **Distinct `FunctionDefinition` IR and `<listOfFunctionDefinitions>` ownership:**
   `ValidatedModel::function_definitions()` returns `&[FunctionDefinition]` (`id()`,
   `parameters()`, `body()`, `source_span()`). Functions are exported in source
   declaration order under `<listOfFunctionDefinitions>` with MathML 2.0 `<lambda>`
   and `<bvar><ci>...</ci></bvar>` children.
2. **Pure `<bvar>` lexical isolation and shadowing:** Function bodies resolve formal
   parameters to `SymbolKind::FunctionParameter` and reject `time`, undeclared free
   symbols, and global model symbols (`DiagnosticCode::DeclarationConflict`). Formal
   parameter names shadow same-named model species/parameters without creating or
   mutating model symbols.
3. **Declaration-order call graph and recursion rejection:** Calls to user-defined
   functions lower to `ExpressionKind::UserCall { name, args }` (`AST_FUNCTION`).
   Functions must be declared before use; undeclared or forward calls reject with
   `DiagnosticCode::UnsupportedFeature`, and direct recursion rejects with
   `DiagnosticCode::DeclarationConflict`.
4. **Global `SId` uniqueness and exact arity:** Collisions between function names and
   the model ID, `__main`, compartments, species, parameters, reactions (including
   generated `_J0`), or unit definitions reject with `DiagnosticCode::DeclarationConflict`.
   Wrong argument counts reject with `DiagnosticCode::DeclarationConflict`, and bare
   function-name references or assignments/rules targeting a function name reject with
   `DiagnosticCode::Syntax`.
5. **Initial-value evaluation, dependency cycles, and call-site unit propagation:**
   Call-site arguments participate in static all-branch and transient dependency cycle
   checks (`DiagnosticCode::InitializationCycle`). `=` initializers evaluate user-function
   calls by binding formal parameters (`eval_with_env`), catching division-by-zero and
   domain errors (`DiagnosticCode::DeclarationConflict`). Call-site unit substitution
   (`lower_expr_with_env`) propagates caller argument unit trees through nested user-function
   bodies and rejects dimensional inconsistencies (`DiagnosticCode::UnitsInconsistency`).
6. **Oracle projection guards:** `tools/oracle/oracle.py::projection_limits` rejects
   `<functionDefinition>` elements without `<math>` (`PROJECTION_UNSUPPORTED`) or with
   unprojected metadata, `tools/oracle/conditional_check.py::require_represented_math`
   inspects `function_definitions`, `tools/oracle/rational_reference.py` audits nonfinite
   literals in `<functionDefinition>` math (`REFERENCE_NONFINITE`), and
   `tools/oracle/functions_check.py` enforces `AST_LAMBDA` (`267`), unique `<bvar>`
   parameters, and global `SId` disjointness.

## 3. All 16 Maintained Compatibility and Numerical Gates

All 16 gates executed against the frozen binaries in `artifacts/functions-phase5/bin/`
and recorded `status: "PASS"` in `artifacts/functions-phase5/gates/*.json`:

| Maintained gate | Total cases | Verified outcome | Failures / blocked |
|---|---:|---|---:|
| `functions` (`tools/oracle/functions_check.py`) | 164 | 68 exact exports / 96 expected rejections | 0 |
| `functions-numerical` (`tools/numerical/function_check.py`) | 2 | 2 closed-form ODE/function trajectories PASS | 0 |
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

The original 90-case corpus (and formatter/CLI routes), arithmetic, and workload suites bind
to `functions-core-v0.5-slice` via the hash-bound overlay
`fixtures/manifests/functions-core-v5-overlay.json` (`antimony-native-overlay-v7`, retaining
58/32 under `--current-profile`), while historical manifests and receipts remain unchanged.

The two new numerical function anchors (`function_kinetic_law_and_observable` and
`nested_function_rate_and_boundary_rule`) match their hand-derived closed-form
solutions and invariants across all 101 grid points on $[0, 10]$ with zero differential
error between native and reference SBML exports.

## 4. Python Suites and Core-Only API Verification

- **Python unit tests:** **490 passed, 0 failed, 0 skipped** (`294` in `tools/oracle`,
  `114` in `tools/numerical`, `51` in `tools/workloads`, `31` in `tools/verification`).
- **Core-only Rust API snippet:** Extracted from [`docs/FUNCTIONS_API_NOTES.md`](../FUNCTIONS_API_NOTES.md)
  and executed with `default-features = false`, `--locked --offline` in
  `artifacts/functions-phase5/api-guide/receipt.json` (`exit_code: 0`).

**Independent signoff: PASS for `functions-core-v0.5-slice`.**
