# Adversarial Acceptance Checklist: Flat Assignment (`:=`) & Rate (`'=`) Rules

> **Status:** Advice-only checklist for proposed `rules-core-v0.4-slice` ([`docs/NEXT_INCREMENT_RECOMMENDATION.md`](NEXT_INCREMENT_RECOMMENDATION.md)).
> **Boundary:** Planning advice only—not implementation or test evidence. Other agents are running upstream reference probes; no reference outcomes are invented here. Every item marked **`[NEEDS-REF-VERIFICATION]`** requires isolated Antimony 3.2.0 / libSBML 5.21.2 / RoadRunner 2.10.0 verification before freezing semantics.

## 1. Core Semantic Distinctions & Ownership Invariants

- **Initial value (`=`) vs. ongoing assignment (`:=`) vs. derivative (`'=`):**
  - `x = expr` sets a $t = 0$ initialization (scalar attribute or `<initialAssignment>`).
  - `x := expr` defines a continuous algebraic invariant (`<assignmentRule>`) active for all $t \ge 0$.
  - `x' = expr` defines an ODE time derivative $dx/dt = \text{expr}$ (`<rateRule>`).
  - Never lower `:=` or `'=` into `InitialAssignment` (`ValidatedModel::initial_assignments()`), and never synthesize a solver in the crate.
- **Ownership & mutability (`src/model.rs`, `src/semantic/initialization.rs`):**
  - Rule targets must resolve to species or parameters (`constant = false` in SBML).
  - **`[NEEDS-REF-VERIFICATION]`** Verify when `Parameter::value()`, `Species::initial_concentration()`, and `initial_expression()` return `None` vs. `Some(f64)` / `Some(ExpressionId)` for `:=` and `'=` targets and their downstream `=` dependents.

## 2. Prioritized Minimal Adversarial Cases (No Invented Outcomes)

1. **Literal vs. Expression `:=` and Unknown Getters (`[NEEDS-REF-VERIFICATION]`)**
   - Candidates: `a = 1; a := 2;` | `a = (1 + 1); a := 3;` | `a = 2; b := a + 1; c = b + 1;` | `species S = 1; S := 2;`
   - Check whether bare literals, signed/rational literals, or constant expressions in `:=` populate scalar `value`/`initialConcentration` or clear a prior `<initialAssignment>`, and verify that non-static `:=` targets leave `value()` / `initial_concentration()` as `None` (never `0.0`).
2. **Same-Symbol `=`, `:=`, and `'=` Collisions & Ordering (`[NEEDS-REF-VERIFICATION]`)**
   - Candidates: `x := 1; x = 2;` | `x' = 1; x = 2;` | `x = 2; x' = 1;` | `x := 1; x := 2;` | `x' = 1; x' = 2;` | `x := 1; x' = 1;`
   - Check which combinations accept (e.g., initial value paired with `'=`) vs. reject as duplicate/conflicting rules or post-`:=` initialization, and whether statement order changes the outcome.
3. **Reaction-Participant Species & Qualifier Interactions (`[NEEDS-REF-VERIFICATION]`)**
   - Candidates: `species S; S -> ; k1*S; S := 2;` | `species S; S := 2; S -> ; k1*S;` | `species S; S' = 0.5; S -> ; k1*S;` | `const a := 1;` | `var species S; S := 1;` | `$S := 1;`
   - Check whether species in reactions automatically promote `boundaryCondition="true"` under `:=` and `'=` regardless of declaration order or role (reactant/product/modifier), and whether `const` conflicts with `:=`/`'=`.
4. **Algebraic Dependency Cycles vs. Legitimate ODE Feedback (`[NEEDS-REF-VERIFICATION]`)**
   - Candidates: `a := b; b := a;` | `a = b; b := a;` | `x = 1; x' = -k * x; k = 0.5;` | `x = 1; y := 2 * x; x' = -y;` | `x' = y; y := x';`
   - Reject algebraic cycles across `=` and `:=` (`DiagnosticCode::InitializationCycle` in `src/semantic/initialization.rs`), while permitting ODE state feedback where `x'` references `x` directly or through an acyclic `:=` helper (`y := 2 * x; x' = -y`).
5. **Inactive Conditional Branches & Static All-Branch Validation (`[NEEDS-REF-VERIFICATION]`)**
   - Candidates: `a := piecewise(a, false, 1);` | `a := piecewise(missing, false, 1);` | `a := piecewise(sqrt(-1), false, 2);` | `a := piecewise(1, false);` | `x = 1; x' = piecewise(1/0, false, -x);`
   - Per `docs/CONDITIONAL_API_NOTES.md`, enforce static symbol resolution, cycle detection, and unit checks across all branches (even `false`-guarded), and verify upstream behavior for inactive domain errors and omitted `otherwise` in `:=` / `'=`.
6. **Time (`time`) and Dimensional / Rate-Unit Checking (`[NEEDS-REF-VERIFICATION]`)**
   - Candidates: `a := time + 1;` | `b = a; a := time;` | `unit sec = 1 second; unit mM = 1e-3 mole / litre; unit rate_u = mM / sec; S has mM; model.time_unit = sec; S := 2 sec;` | `S has mM; model.time_unit = sec; S' = 2 mM;` | `S has mM; model.time_unit = sec; S' = 2 rate_u;`
   - Check that `time` in `:=`/`'=` keeps static getters `None`, `:=` requires `expr_dim == target_dim`, and `'=` requires `expr_dim == target_dim / time_dim` (`DiagnosticCode::UnitsInconsistency` on known mismatch; warning when unspecified).
7. **Name Resolution, Redeclarations & Indirect Reaction Modifiers (`[NEEDS-REF-VERIFICATION]`)**
   - Candidates: `J0: -> S; k * S; k := 2;` | `species A, B; k := A; J0: -> B; k;` | `J0: A -> B; 1; J0 := 2;` | `formula f := 3; species f;`
   - Verify implicit parameter creation on first-seen `:=`/`'=`, kind-redeclaration and reaction-ID target rejections (`J0 := 2`), and whether a species inside a `:=` parameter referenced by a reaction rate is inferred into `<listOfModifiers>`.
8. **Canonical Formatting, SBML `<listOfRules>` Order & CLI Parity (`[NEEDS-REF-VERIFICATION]`)**
   - Candidates: `b' = 1; a := 2; c' = 3;` | `x' = -(1/2) * x; y := piecewise(1, x > 0 && time >= 0, 0);`
   - Verify whether `<listOfRules>` preserves source order, symbol order, or rule-kind grouping; ensure `format_antimony` roundtrips `:=` and `'=` byte-identically; verify CLI `check`, `format`, and `export --json` expose rules distinctly from `initial_assignments`.
9. **Unsupported Compartment Rules, Events & Algebraic Rules (`[NEEDS-REF-VERIFICATION]`)**
   - Candidates: `compartment c = 1; c := 2;` | `compartment c = 1; c' = 0.1;` | `at (time > 1): x = 2;` | `0 = x + y - 1;`
   - Distinguish valid-but-unsupported Antimony from malformed syntax: reject compartment `:=`/`'=`, events, and algebraic rules with `DiagnosticCode::UnsupportedFeature` (never silently dropped).
10. **Resource Limits & Iterative Dependency Traversal**
    - Candidates: `a0 = 1; a1 := a0 + 1; ... aN := aN_1 + 1;` | deeply nested `piecewise(...)` or binary trees on the RHS of `:=` / `'=`.
    - Enforce `Limits` (`max_bytes`, `max_tokens`, `max_statements`, `max_expr_depth`, `max_entities`) and keep dependency resolution iterative (`Kahn` in `src/semantic/initialization.rs`) with no recursion overflow or $O(N^2)$ blowup.
