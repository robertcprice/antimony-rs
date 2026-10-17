# Independent Collaboration & Robustness Verification (`functions-core-v0.5-slice`)

> **Status:** Verified against Antimony `3.2.0`, `python-libsbml` `5.21.2`, and `libroadrunner` `2.10.0`.
> **Scope:** Executes the "Final verification and collaboration gate" of [`docs/FLEXIBLE_MODELING_PLAN.md`](FLEXIBLE_MODELING_PLAN.md) and the post-Phase-5 robustness assessment in [`docs/NEXT_INCREMENT_RECOMMENDATION.md`](NEXT_INCREMENT_RECOMMENDATION.md).
> **Boundary:** Preserves all historical receipts (`tools/oracle/functions-evidence.json`, `tools/oracle/rules-evidence.json`, `tools/oracle/conditional-evidence.json`, `tools/oracle/rational-evidence.json`) and frozen binaries byte-for-byte. No solver is added to the Rust crate, and no claim of full Antimony compatibility or official upstream endorsement is made.

---

## 1. Purpose

To ensure that Herbert Sauro and external modelers can write, edit, format, validate, and export hand-authored flat pathway models without relying on fixture-specific assumptions, this verification pass evaluates:

1. **36 independently authored flat pathway, incomplete-draft, and negative near-neighbor models** ([`fixtures/collaboration/manifest.json`](../fixtures/collaboration/manifest.json)) combining Phases 1–5:
   - Arithmetic precedence and scientific notation (`1e-6`, `2.5e-9`, `5e-12`)
   - Source-ordered symbols, compartments, boundary promotion, and unnamed reactions (`_J0`, `_J1`, ...)
   - Numeric builtins, `time`, and exact rational lowering (`(1 / 2)`, `(3 / 4)`)
   - Conditional, relational, logical, and `piecewise(...)` expressions
   - Assignment (`:=`) and rate (`'=`) rules with target first-seen ordering, lexical scalar-export fidelity, and transient prefix cycle checks
   - Declaration-ordered non-recursive user-defined functions (`function name(params) body end`) with `<bvar>` lexical isolation and call-site unit propagation
2. **Explicit separation of structural validity from simulation readiness:**
   - **18 `complete` pathway models:** Every compartment, species, and parameter has a known initial state (`size`, `initialConcentration`, `value`, `InitialAssignment`, or `AssignmentRule`). Verified both structurally (exact SBML L3V2 projection parity, byte-idempotent `format_antimony` roundtrip) and numerically in `libroadrunner` `2.10.0` over $t \in [0, 10]$ (101 points, `max_abs_diff == 0.0` between reference and native SBML exports).
   - **4 `incomplete` draft models:** Structurally valid models with unset initial concentrations, compartment sizes, or rate constants (`None` in Rust getters; omitted attributes in SBML L3V2 without fake `0.0` defaults), paired with their "later filled in" complete counterparts (`incomplete_michaelis_menten_draft` $\to$ `filled_in_michaelis_menten_draft`, `incomplete_two_stage_signaling_draft` $\to$ `filled_in_two_stage_signaling_draft`).
   - **14 `rejected` near-neighbors & deferred-scope controls:** Verified that `check`, `format`, and `export` deterministically reject without emitting partial artifacts.
3. **Rust library editing workflows & mutation/resource hardening** ([`tests/collaboration_robustness.rs`](../tests/collaboration_robustness.rs)):
   - Tests incomplete-to-complete interactive editing, deep user-function call chains (`f0`..`f15`), source-order species boundary promotion, explicit `DiagnosticCode` contracts on every deferred feature, and UTF-8 prefix truncation / token-insertion fuzzing within bounded `Limits`.

---

## 2. Coverage of Review Exercises (`docs/FLEXIBLE_MODELING_PLAN.md`)

| Review Exercise | Collaboration Corpus Fixtures | Verification Outcome |
| :--- | :--- | :--- |
| **Unnamed conversion chain** | `unnamed_three_step_conversion_chain`, `incomplete_unnamed_chain_with_unset_rates` | Exact `_J0`..`_J4` generation, format roundtrip idempotence, RoadRunner trajectory parity (`max_abs_diff = 0.0`). |
| **Reusable saturating rate (`function ... end`)** | `reusable_michaelis_menten_two_isozyme_pathway`, `hill_cooperative_feedback_cascade`, `covalent_modification_goldbeter_koshland_cycle`, `reversible_haldane_isomerization_with_product_inhibition`, `nested_user_functions_with_shadowed_formal_parameters` | Multi-call-site reuse of `mm`, `hill_act`, `hill_inh`, `rev_mm`, `comp_inh`; `<listOfFunctionDefinitions>` `<lambda>` parity; RoadRunner trajectory parity. |
| **Small-rate-constant model** | `small_rate_constant_trace_signaling` | Trace concentrations (`2.5e-9`, `5.0e-12`) and small rate constants (`3.5e-4`, `1.0e-5`) preserved without underflow or precision loss. |
| **Time-dependent input** | `time_dependent_sinusoidal_and_exponential_forcing`, `source_order_boundary_species_and_rate_rule_driver` | `<csymbol>` `time` inside user-function arguments and `:=` rules; RoadRunner trajectory parity. |
| **Derived assignment variable (`:=`)** | `derived_assignment_conserved_moiety_and_energy_charge`, `reusable_michaelis_menten_two_isozyme_pathway`, `covalent_modification_goldbeter_koshland_cycle` | Conserved moiety totals, adenylate energy charge `(ATP + 0.5*ADP)/(ATP + ADP + AMP)`, and instantaneous flux sums exported as `<assignmentRule>`. |
| **Rate-controlled state (`'=`)** | `rate_controlled_state_coupled_to_enzymatic_sink`, `source_order_boundary_species_and_rate_rule_driver`, `unit_annotated_linear_ode_and_observable`, `repeated_rule_override_retaining_inferred_parameter` | `<rateRule>` states coupled to reactions and `:=` observables; target first-seen rule ordering preserved. |
| **Piecewise stimulus** | `piecewise_pulsed_stimulus_with_recovery` | `piecewise(high, (t >= t_on) && (t < t_off), low)` inside user-defined helper driving receptor activation/recovery. |
| **Incomplete model later filled in** | `incomplete_michaelis_menten_draft` $\to$ `filled_in_michaelis_menten_draft`, `incomplete_two_stage_signaling_draft` $\to$ `filled_in_two_stage_signaling_draft`, `incomplete_rate_rule_without_initial_state` | Incomplete drafts classified as `incomplete_export` (`NOT_SIMULATION_READY`, no fake zeros); filled-in counterparts classified as `complete_export` with full RoadRunner simulation parity. |
| **Negative near-neighbors & deferred scope guards** | `neg_forward_user_function_call`, `neg_direct_recursive_user_function`, `neg_function_capturing_global_parameter`, `neg_function_capturing_time_symbol`, `neg_transient_assignment_rule_cycle`, `neg_mixed_assign_and_rate_rule_same_target`, `neg_initial_assignment_after_assignment_rule`, `neg_nonboundary_species_with_reaction_and_rule`, `neg_deferred_event_trigger`, `neg_deferred_dynamic_compartment_assignment_rule`, `neg_deferred_dynamic_compartment_rate_rule`, `neg_deferred_reaction_id_assignment_rule`, `neg_deferred_algebraic_rule`, `neg_unit_mismatch_across_user_function_call` | All 14 rejected across `check`, `format`, and `export` with exact expected `DiagnosticCode` and zero artifact emission. |

---

## 3. Reproducible Commands

```bash
# 1. Run the Rust collaboration & robustness integration suite across feature modes
cargo test --test collaboration_robustness --all-features
cargo test --test collaboration_robustness --no-default-features

# 2. Run the oracle collaboration gate (including RoadRunner 2.10.0 trajectory checks on all 18 complete models)
.venv-oracle/bin/python tools/oracle/collaboration_check.py \
  --cli artifacts/functions-phase5/bin/antimony \
  --trusted-native \
  --roadrunner-python .venv-numerical/bin/python

# 3. Run the collaboration gate unit tests
.venv-oracle/bin/python -m unittest discover -s tools/oracle -p "test_collaboration_check.py"
```
