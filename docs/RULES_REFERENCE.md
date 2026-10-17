# Assignment and Rate Rules Reference Behavior (`rules-core-v0.4-slice`)

This document records the reconciled reference contract for flat parameter and species assignment rules (`:=`) and rate rules (`'=`) against upstream Antimony `3.2.0` and libSBML `5.21.2` (`.venv-oracle/bin/python`).

## 1. Raw Discovery Evidence

Reference behavior was probed before any native rule implementation exists:

- **Native-owner discovery:** [`artifacts/rules-phase4/discovery-native-owner/reference.json`](../artifacts/rules-phase4/discovery-native-owner/reference.json) (77 models) and [`artifacts/rules-phase4/discovery-native-owner/followup.json`](../artifacts/rules-phase4/discovery-native-owner/followup.json) (13 models), totaling **90 probed models**.
- **Independent discovery:** [`artifacts/rules-phase4/independent/reference.json`](../artifacts/rules-phase4/independent/reference.json) (76 executions), [`artifacts/rules-phase4/independent/followup.json`](../artifacts/rules-phase4/independent/followup.json) (4 executions), and [`artifacts/rules-phase4/independent/summary.json`](../artifacts/rules-phase4/independent/summary.json), totaling **80 executions** across **78 distinct sources** (**51 valid SBML exports**, **29 loader rejections**).
- **Oracle projection audit:** [`artifacts/rules-phase4/evidence-design/projection-audit.json`](../artifacts/rules-phase4/evidence-design/projection-audit.json) (9 read-only assertions).

## 2. Reconciled Findings vs Preliminary Planning Assumptions

1. **Same-kind rule replacement vs mixed-kind conflict:**
   - Repeated same-kind rules (`a := 1; a := 2;` or `a' = 1; a' = 2;`) are valid upstream (`assign_repeat_same_kind`, `rate_repeat_same_kind`, `repeat_preserve_order`) and replace the target's rule formula in place.
   - Mixing assignment and rate rules on the same target in either order (`a := 1; a' = 2;` or `a' = 1; a := 2;`) is rejected upstream (`assign_then_rate_same_target`, `rate_then_assign_same_target`, `conflict_assign_then_rate`, `conflict_rate_then_assign`). Native diagnostic: `DiagnosticCode::DeclarationConflict`.

2. **Exported `<listOfRules>` ordering:**
   - Upstream Antimony does **not** emit `<listOfRules>` in rule-statement order or topological dependency order. Instead, rules are emitted grouped by target class (**species first, then parameters**), and within each class in **symbol first-seen order** (`rule_order_decl_order`, `rule_order_reverse_dep`, `repeat_preserve_order`, `species_assign_rule`, `species_rate_rule`).

3. **Interaction between rules (`:=`, `'=`) and ordinary initialization (`=`):**
   - An assignment rule after a scalar or expression initialization (`a = 1; a := 2;`, `a = 1 + 1; a := 3;`, `a = 9; a := u + 1;`) clears the prior `=` initialization: no `<initialAssignment>` is emitted, and any prior scalar attribute is cleared (`init_then_assign`, `init_expr_then_assign`, `assign_clears_prior_scalar`).
   - An ordinary initialization (`=`) after an assignment rule (`a := 2; a = 1;`, `a := 3; a = 1 + 1;`) is rejected upstream (`assign_then_init`, `assign_then_init_expr`, `conflict_assign_then_init`). Native diagnostic: `DiagnosticCode::DeclarationConflict`.
   - Rate rules (`'=`) retain independent `=` initialization whether `=` appears before or after `'=` (`init_then_rate`, `rate_then_init`, `init_expr_then_rate`, `rate_then_init_expr`, `rate_expr_with_init_expr`).

4. **Lexical scalar attribute population on assignment-rule targets:**
   - When an assignment rule's right-hand side is a bare numeric literal or unary minus followed by a numeric literal (allowing spaces/block comments after `-`, within the finite 15-digit `[1e-307, 1e308]` or zero envelope), upstream populates the target's SBML `value` (for parameters) or `initialConcentration` (for species) attribute alongside the `<assignmentRule>` (`assign_scalar`, `scalar_neg`, `scalar_sci`, `scalar_zero`, `species_assign_rule`, `assign_comment_in_neg_rhs`).
   - Grouped literals `(2)`, `(-2)`, unary plus `+2`, double negative `-(-2)`, rationals `1/2`, `-(1/2)`, arithmetic `1+1`, predefined constants `pi`/`exponentiale`, Booleans `true`/`false`/`True()`, and function calls `sin(0)` do **not** populate the scalar attribute on the target parameter or species (`scalar_paren`, `scalar_neg_paren`, `scalar_unary_plus`, `scalar_double_neg`, `scalar_rational`, `scalar_neg_rational`, `scalar_pi`, `scalar_true`, `scalar_false`, `scalar_sin0`), even when their normalized MathML is a constant.
   - Rate rules (`'=`) never populate the target's scalar attribute from the derivative RHS (`rate_scalar`, `species_rate_rule`); their initial scalar or `<initialAssignment>` comes solely from an independent `=` initializer.

5. **Source-order-sensitive species boundary promotion and reaction conflicts:**
   - If a symbol already has a rule (`:=` or `'=`) as an inferred or declared parameter and is **subsequently promoted to a species** (via `species S`, `var species S`, inline `species S := 2`, `var species S := 2`, or reaction participation `S := 2; J: S => P; k;`), upstream promotes `S` with `boundaryCondition="true"` (`assign_before_species_decl`, `species_inline_assign`, `var_species_inline_assign`, `species_rule_then_var_species`, `species_rate_then_var_species`, `species_rule_then_reaction_participant`).
   - If a symbol is **already declared as a species** before the rule is attached (`species S; S := 2;` or `species S; S' = 2;`), upstream does **not** promote `S` to a boundary species (`boundaryCondition="false"`), unless the rule target has an explicit `$` prefix (`species S; $S := 2;` or `$S := 2; species S;`) (`species_assign_rule`, `species_rate_rule`, `species_Boundary_dollar_assign`, `species_Boundary_dollar_rate`).
   - If a symbol is already a species, a later explicit `var species S;` sets `boundaryCondition="false"` (`S := 2; species S; var species S;` in `species_rule_then_species_then_var`), whereas a later bare `species S;` preserves the existing `boundaryCondition` (`S := 2; species S; species S;` in `species_rule_then_species_twice`).
   - A non-boundary species (`boundaryCondition="false"`) with an assignment or rate rule cannot participate as a **reactant or product** in any final reaction (`species_assign_and_reaction`, `species_rate_and_reaction`, `species_rule_then_var_then_reaction`), and rejects with `DiagnosticCode::DeclarationConflict`.
   - Boundary species (`$S`) with rules **may** participate as reactants or products (`boundary_species_assign_and_reaction`, `boundary_species_rate_and_reaction`), and non-boundary species with rules **may** appear in reaction kinetic laws as modifiers (`species_assign_modifier_in_reaction`, `species_rate_modifier_in_reaction`). If a conflicting reaction is overwritten by a later reaction with the same id that no longer uses the species as a reactant/product, the model succeeds (`assignment_reaction_replaced_out`).
   - Species referenced only inside a parameter rule (`a := S; J: P => Q; a;`) are **not** added to `J`'s `<listOfModifiers>` (`assign_indirect_species_in_kinetics`).

6. **Transient and final algebraic dependency cycles:**
   - Algebraic dependency edges arise from active `=` initializations and `:=` assignment rules across all branches (including inactive `piecewise` branches: `cycle_assign_piecewise_dead_branch`).
   - Rate rules (`'=`) define time derivatives $dx/dt$ and do **not** create outgoing algebraic dependency edges from their target (`rate_self_ref`, `cycle_rate_two`, `assign_depends_on_rate_target`, `init_depends_on_rate_target`, `rate_depends_on_assign_target` are all valid). However, an algebraic cycle on the initial values of rate-rule targets (`a' = a; a = a + 1;` or `b' = a; a = b + 1; b = a + 1;`) is rejected (`rate_with_self_init_expr`, `rate_with_init_cycle`).
   - Transient algebraic cycles (`a := b; b := a; a := 1;`) are rejected immediately upstream when the cycle closes (`cycle_assign_resolved_by_overwrite`), even though the final rule graph is acyclic. Therefore, checking only the final rule graph is insufficient; native semantic analysis must check whether a target $v$ is reachable from its existing outgoing algebraic dependencies before overwriting an active `=` or `:=` definition on $v$.

7. **Symbol persistence across overwritten rule expressions:**
   - When an assignment or rate rule is overwritten (`a := u; a := 2;`), symbols referenced only in the earlier overwritten expression (`u`) remain in the model's symbol table as inferred parameters, while only the final rule expression is emitted and checked for domain/unit validity (`assign_overwrite_infers`, `assign_overwrite_domain`).

8. **Syntax, token coalescing, and empty rule clearing:**
   - Upstream coalesces `:` + `=` into `:=` and `'` + `=` into `'=` across spaces, tabs, and block comments (`a : = 2;`, `a ' = 2;`, `a :/*c*/= 2;`, `a '/*c*/= 2;`, `a :/*\n*/= 2;`), whereas actual newline tokens block coalescing (`a:\n=2;`, `a'\n=2;`, `a://x\n=2;`, `a\n:=2;`, `a\n'=2;` all fail as syntax errors).
   - `a:==2;`, `a'==2;`, `a''=2;`, and prime in expression context (`y := x';`) are syntax errors (`assign_double_eq`, `rate_double_eq`, `rate_double_prime`, `prime_in_expr`).
   - Empty rule syntax (`a := ;`, `a' = ;`, `a :=` at EOF, `formula a := , b = 1;`, `a := \n b = 1;`) loads upstream (`status = 1`) and clears any prior rule without emitting a rule (`empty_assign_rhs`, `empty_rate_rhs`, `empty_assign_cleared`, `empty_assign_eof`, `list_empty_assign_middle`, `assign_rhs_newline_next_stmt`). Under `rules-core-v0.4-slice`, empty rule clearing is explicitly rejected with `DiagnosticCode::UnsupportedFeature` rather than silently discarding rules, while a newline followed by a stranded expression (`a:=\n2;`) or `end` on the same line (`model m\na:=end`) rejects with `DiagnosticCode::Syntax`.

9. **Non-parameter/species rule targets and algebraic rules:**
   - Assignment to a reaction id (`J: A -> B; k; J := 2;` or `J := 2; J: A -> B; k;`) replaces the reaction's kinetic law in upstream Antimony rather than emitting a `<rule>` (`reaction_assign_rule`, `assign_rule_before_reaction`). Under `rules-core-v0.4-slice`, reaction `:=` is explicitly deferred with `DiagnosticCode::UnsupportedFeature`.
   - Rate rules on a reaction id (`J' = 2;`) are rejected upstream (`reaction_rate_rule`, `rate_rule_before_reaction`) and fail with `DiagnosticCode::DeclarationConflict`.
   - Rules on compartments (`compartment c = 1; c := 2;`, `c' = 2;`, `c := 2; compartment c = 1;`) load upstream as dynamic compartments (`compartment_assign_rule`, `compartment_rate_rule`, `assign_before_compartment`) and are explicitly deferred under `rules-core-v0.4-slice` with `DiagnosticCode::UnsupportedFeature`.
   - Rules on unit definitions (`unit u = 10 second; u := 2;`, `u' = 2;`) reject upstream (`unit_assign_rule`, `unit_rate_rule`) and fail with `DiagnosticCode::DeclarationConflict`.
   - Algebraic rules (`0 = x + y - 1;`) load upstream (`algebraic_rule_zero_eq`) and are explicitly deferred under `rules-core-v0.4-slice` with `DiagnosticCode::UnsupportedFeature`.

10. **Unit checking on rules:**
    - Upstream Antimony accepts dimensional mismatches in assignment and rate rules (`unit_mismatch_assign`, `unit_mismatch_rate`), whereas `antimony-rs` enforces dimensional consistency when dimensions are explicitly declared (`target == value` for `AssignmentRule`; `value == target - time` for `RateRule`), rejecting mismatches with `DiagnosticCode::UnitsInconsistency`. This intentional stricter native validation is documented and tracked as a known semantic policy difference.

## 3. Native Contract Summary (`rules-core-v0.4-slice`)

- **Public IR (`src/model.rs`, `src/lib.rs`):**
  - `RuleKind { Assignment, Rate }`
  - `Rule { kind: RuleKind, target: String, expression: String, span: Span }`
  - `ValidatedModel::rules(&self) -> &[Rule]`
  - `Parameter::value(&self) -> Option<f64>` and `Species::initial_concentration(&self) -> Option<f64>` return `Some(f64)` for `:=` targets only when the rule RHS is a verified lexical finite scalar (`number` or `-number`); `Parameter::initial_expression()` / `Species::initial_expression()` is `None` for `:=` targets and reflects only independent `=` expression initializers for `'=` targets.
  - No unknown-to-zero fallback or numerical solver inside the core crate.
