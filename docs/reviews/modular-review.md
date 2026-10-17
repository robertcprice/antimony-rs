# Independent Phases 6–13 Review (`modular-core-v0.8-slice`)

**Current status: independently signed off — native 4-configuration matrix, 48-case
reference-frozen modular/events/Phase-6–9 corpus, 55-case bidirectional SBML $\leftrightarrow$ Antimony
roundtrip gate, 2 closed-form numerical trajectory models, wild BioModels SBML L2/L3 import tests,
multi-file `import "..."` composition tests, pure-Rust stoichiometry, ODE/event simulator, steady-state,
and MCA elasticity matrix tests, standalone `events_and_modules`, `import_sbml`, and `ode_trajectory_csv`
examples, and all 20 maintained compatibility/numerical gates PASS.**
Profile: `modular-core-v0.8-slice`. Historical receipts (`artifacts/collaboration-phase6/collaboration-evidence.json`,
`tools/oracle/functions-evidence.json`, `tools/oracle/rules-evidence.json`,
`tools/oracle/conditional-evidence.json`, `tools/oracle/rational-evidence.json`)
and immutable binary directories (`artifacts/conditionals-phase3b/bin/`, `artifacts/rules-phase4/bin/`,
`artifacts/functions-phase5/bin/`) are preserved byte-for-byte.

## 1. Frozen Native Verification Matrix

| Feature configuration | Rust unit + integration tests | Doctests | Failed / ignored |
|---|---:|---:|---:|
| `--all-features` (`sbml` + `cli`) | 485 | 2 | 0 / 0 |
| Default (`sbml`) | 440 | 2 | 0 / 0 |
| Core-only (`--no-default-features`) | 421 | 2 | 0 / 0 |
| CLI without SBML (`--no-default-features --features cli`) | 466 | 2 | 0 / 0 |

Frozen binaries in `artifacts/modular-phase8/bin/manifest.json`:
- `antimony` CLI: `6e05ad81a1352ea5bdbe1f6333a9f375e50cc59d0d2a31ffa63eb6d120c3f2de`
- `export_sbml`: `0d66c4e43b91eae2a912f9551cbf1ee3d56cfdc073b9abf086ffd97fc2a01c30`
- `format_antimony`: `01ad12abab61c21ae8bdf433b8ac0be85322fbb7b02c47b54714c024feb7776d`
- `import_sbml`: `a298a18d72799f734ac56793791169e384034ee796eb8b7b32b060b1cef5423e`
- `ode_trajectory_csv`: `7ee397f34177c97a131a6737f77a95a20afbbf8a873c5c069212edbd294c8caa`

## 2. All 20 Maintained Compatibility, Roundtrip, and Numerical Gates

| Maintained gate | Total cases | Verified outcome (`modular-core-v0.8-slice`) | Failures / blocked |
|---|---:|---|---:|
| `modular` (`tools/oracle/modular_check.py`) | 48 | 33 exact exports (31 complete + 2 incomplete) / 15 expected rejections | 0 |
| `sbml-roundtrip` (`tools/oracle/sbml_roundtrip_check.py`) | 55 | 55 byte-identical SBML $\leftrightarrow$ Antimony roundtrips + 49/49 RoadRunner trajectories PASS | 0 |
| `modular-numerical` (`tools/numerical/modular_check.py`) | 2 | 2 closed-form Phase 6–8 analytic trajectories PASS | 0 |
| `collaboration` (`tools/oracle/collaboration_check.py`) | 36 | 26 exact exports (22 complete + 4 incomplete) / 10 expected rejections | 0 |
| `functions` (`tools/oracle/functions_check.py`) | 164 | 71 exact exports / 93 expected rejections | 0 |
| `functions-numerical` (`tools/numerical/function_check.py`) | 2 | 2 closed-form ODE/function trajectories PASS | 0 |
| `rules` (`tools/oracle/rules_check.py`) | 200 | 122 exact exports / 78 expected rejections | 0 |
| `rules-numerical` (`tools/numerical/rule_check.py`) | 2 | 2 closed-form ODE/rule trajectories PASS | 0 |
| `conditional` (`tools/oracle/conditional_check.py`) | 262 | 160 exact exports / 102 expected rejections | 0 |
| `piecewise-numerical` (`tools/numerical/piecewise_check.py`) | 1 | 1 continuous-rate piecewise trajectory PASS | 0 |
| `rational` (`tools/oracle/rational_check.py`) | 124 | 73 exact exports / 51 expected rejections | 0 |
| `original` (`tools/oracle/native_check.py --current-profile`) | 90 | 60 exact exports / 30 expected rejections | 0 |
| `formatter` (`tools/oracle/roundtrip_check.py --current-profile`) | 90 | 60 byte-idempotent roundtrips / 30 rejections | 0 |
| `cli` (`tools/oracle/cli_check.py --current-profile`) | 90 | 60 supported CLI / 30 expected rejections | 0 |
| `arithmetic` (`tools/oracle/expression_check.py --current-profile`) | 54 | 48 exact exports / 6 expected rejections | 0 |
| `symbols` (`tools/oracle/symbols_check.py`) | 66 | 55 exact exports / 11 expected rejections | 0 |
| `builtins` (`tools/oracle/builtins_check.py`) | 109 | 66 exact exports / 43 expected rejections | 0 |
| `workload` (`tools/workloads/check.py --current-profile`) | 86 | 74 supported (6 motifs + 68 generated) / 12 rejections | 0 |
| `numerical` (`tools/numerical/check.py`) | 8 | 8 original analytic trajectories PASS | 0 |
| `builtin-numerical` (`tools/numerical/builtin_check.py`) | 2 | 2 builtin/time analytic trajectories PASS | 0 |

- **Python unit tests:** **520 passed, 0 failed, 0 skipped** (`318` in `tools/oracle`, `120` in `tools/numerical`, `51` in `tools/workloads`, `31` in `tools/verification`).

**Independent signoff: PASS for `modular-core-v0.8-slice` (Phases 6–13).**
