# Independent Review: Phase 4 Rules Discovery

**Reviewer:** Independent Discovery & Verification Audit  
**Scope:** Upstream Antimony `3.2.0` + libSBML `5.21.2` behavior for assignment (`:=`) and rate (`'=`) rules  
**Evidence Artifacts:**
- [`artifacts/rules-phase4/independent/reference.json`](../../artifacts/rules-phase4/independent/reference.json) (76 executions)
- [`artifacts/rules-phase4/independent/followup.json`](../../artifacts/rules-phase4/independent/followup.json) (4 follow-up executions)
- [`artifacts/rules-phase4/independent/summary.json`](../../artifacts/rules-phase4/independent/summary.json) (80 total executions across 78 distinct sources: 51 valid exports, 29 loader rejections)
- [`artifacts/rules-phase4/discovery-native-owner/reference.json`](../../artifacts/rules-phase4/discovery-native-owner/reference.json) + [`followup.json`](../../artifacts/rules-phase4/discovery-native-owner/followup.json) (90 native-owner probes)

## 1. Executive Summary

Independent discovery probed 78 distinct Antimony source inputs (80 total executions) spanning declaration order, repeated/conflicting rules, initialization interactions, species boundary promotion, reaction modifiers/participants, compartments, units, lexical token coalescing, algebraic/empty rules, and transient vs final dependency cycles.

All 10 findings in [`docs/RULES_REFERENCE.md`](../RULES_REFERENCE.md) were independently reproduced and verified against raw oracle executions before any native Rust rule implementation was written.

## 2. Key Independent Findings Verified

1. **Declaration-order species boundary promotion (`species_rule_then_var_species`, `species_rate_then_var_species`, `species_rule_then_species_then_var`, `species_rule_then_species_twice`):**
   - A symbol that acquires an assignment or rate rule *before* being promoted from parameter to species becomes `boundaryCondition="true"` at the point of promotion—even when promoted by `var species S` or inline `var species S := 2`.
   - Once a symbol is already a species, attaching `S := 2` or `S' = 2` does *not* alter `boundaryCondition` (unless prefixed with `$S`).
   - Subsequent explicit `var species S` on an already-promoted species clears `boundaryCondition` to `false`, whereas a bare `species S` preserves the existing `boundaryCondition`.
   - Any final non-boundary species with a rule that participates as a reaction reactant or product is rejected (`species_rule_then_var_then_reaction`), unless the reaction is later replaced by one that removes the species from reactants/products (`assignment_reaction_replaced_out`).

2. **Lexical scalar export boundary (`scalar_paren`, `scalar_neg_paren`, `scalar_unary_plus`, `scalar_double_neg`, `scalar_rational`, `scalar_neg_rational`, `scalar_pi`, `scalar_true`, `scalar_false`, `scalar_sin0`, `assign_comment_in_neg_rhs`):**
   - Only bare numeric literals (`2`, `1e-3`, `0`) and unary-minus numeric literals (`-2`, `- 2`, `-/*c*/2`, `-0`) on the RHS of `:=` populate the target's SBML `value` / `initialConcentration` attribute.
   - Parenthesized numbers `(2)`, `(-2)`, unary plus `+2`, rational constants `1/2`, named constants `pi`, and Boolean constants `true`/`false` leave the SBML scalar attribute unset (`None`), even though their MathML is constant.

3. **Transient cycle rejection (`cycle_assign_resolved_by_overwrite`):**
   - `a := b; b := a; a := 1;` fails immediately at `b := a;` with `"Error in model string, line 1:  LoopFound: Cycle of dependencies in initial assignments and assignment rules for 'a' and 'b'."`
   - A final-graph-only cycle pass would erroneously accept this model. Incremental reachability checking prior to overwriting an existing algebraic definition (`=` or `:=`) is required and sufficient.

4. **Rule ordering (`rule_order_decl_order`, `rule_order_reverse_dep`, `repeat_preserve_order`, `species_assign_rule`, `species_rate_rule`):**
   - `<listOfRules>` is ordered by target class (species first, then parameters) and within each class by the target symbol's first-seen declaration order, unaffected by subsequent same-kind rule replacements.

5. **Deferred constructs and explicit diagnostic boundaries:**
   - Empty rule clearing (`a := ;`, `a' = ;`, `a :=` at EOF, `formula a := , b = 1;`), reaction kinetic-law replacement via `J := 2;`, dynamic compartment rules (`compartment c = 1; c := 2;`), and `0 = ...` algebraic rules load in upstream Antimony 3.2.0 (`status = 1`) with specialized semantics outside flat parameter/species rules. Rejecting them explicitly with `DiagnosticCode::UnsupportedFeature` preserves honest scope boundaries without silent miscompilation.

## 3. Conclusion

The reconciled contract in [`docs/RULES_REFERENCE.md`](../RULES_REFERENCE.md) accurately captures all observed upstream state transitions and establishes clear, testable boundaries for `rules-core-v0.4-slice`.
