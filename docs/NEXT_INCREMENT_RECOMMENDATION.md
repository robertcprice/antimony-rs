# Next Increment Recommendation: Post-Phase-5 Robustness & External-Model Assessment

> **Status:** Updated after independent verification of Phase 4 (`rules-core-v0.4-slice`) and Phase 5 (`functions-core-v0.5-slice`).
> **Boundary:** Planning and roadmap reassessment. No solver, full Antimony compatibility, or official upstream endorsement is claimed.

## 1. Completed Milestones Summary

Across Phases 1–5, the core flat-model expression and declaration pipeline is implemented and verified against Antimony `3.2.0`, `python-libsbml` `5.21.2`, and `libroadrunner` `2.10.0`:

- **Phase 1–3A.1:** Arithmetic precedence (`Phase 1`), source-ordered symbols and unnamed reactions (`Phase 2`), numeric builtins/time and exact rational lowering (`Phase 3A`/`3A.1`, [`tools/oracle/rational-evidence.json`](../tools/oracle/rational-evidence.json)).
- **Phase 3B (`conditional-core-v0.3-slice`):** Conditional, relational, logical, and `piecewise(...)` expressions ([`docs/CONDITIONALS.md`](CONDITIONALS.md), [`tools/oracle/conditional-evidence.json`](../tools/oracle/conditional-evidence.json)).
- **Phase 4 (`rules-core-v0.4-slice`):** Bounded flat parameter and species assignment (`:=`) and rate (`'=`) rules with target first-seen ordering, lexical scalar-export fidelity, source-order boundary promotion, and transient prefix cycle detection ([`docs/RULES.md`](RULES.md), [`tools/oracle/rules-evidence.json`](../tools/oracle/rules-evidence.json)).
- **Phase 5 (`functions-core-v0.5-slice`):** Declaration-ordered non-recursive user-defined functions (`function name(params) body end`) with `<bvar>` lexical isolation, global `SId` collision checks, static initializer evaluation, and call-site unit propagation ([`docs/FUNCTIONS.md`](FUNCTIONS.md), [`tools/oracle/functions-evidence.json`](../tools/oracle/functions-evidence.json)).

With `functions-core-v0.5-slice` complete, Herbert Sauro and external users can author, check, canonical-format, and export flat kinetic models combining reactions, reusable user-defined rate-law functions, `:=` observables/conserved totals, `'=` direct ODE rate equations, `piecewise(...)` forcing terms, and `has` unit checks.

## 2. Reassessed Candidate Increments After Phase 5

| Candidate | Value for Arbitrary User Models | Readiness & Risk Profile | Recommendation |
| :--- | :--- | :--- | :--- |
| **1. Robustness, Fuzzing & Curated External-Model Testing** | **Highest immediate assurance value.** Tests how independently written flat models behave end-to-end across parser, symbol resolution, unit checking, formatting roundtrips, and SBML L3V2 export ([`docs/reviews/value-priorities.md`](reviews/value-priorities.md)). | Can immediately exercise the completed Phase 1–5 feature set and verify that out-of-scope constructs fail cleanly with `DiagnosticCode::UnsupportedFeature` and bounded resource usage. | **Recommend next** (`robustness-and-external-corpus`). |
| **2. Richer Units (`substanceOnly`, `in compartment` scaling, plural aliases)** | **Moderate-to-high.** Improves dimensional certification and amount-vs-concentration fidelity on published models ([`docs/IMPLEMENTATION_PLAN.md`](IMPLEMENTATION_PLAN.md)). | Requires reference-first discovery on how Antimony 3.2.0 couples `substanceOnly` species, compartment sizes, and reaction stoichiometry in kinetic laws. | **Second** (after external-model audit identifies highest-impact unit/species patterns). |
| **3. Events (`at ...:`) & Delays** | **High biological expressiveness** for state-reset protocols, cell-cycle switches, and pulsed dosing. | Introduces event triggers, delay/priority/persistent attributes, and event-assignment ordering (`<listOfEvents>`). Must not be conflated with continuous `piecewise(...)` expressions. | **Keep explicitly scoped (`UnsupportedFeature`)** until a dedicated reference-first event slice is scheduled. |
| **4. Dynamic Compartments (`c := ...`, `c' = ...`)** | **Specialized.** Needed only for growing-volume / dilution models. | Interacts subtly with concentration vs. substance-amount ODEs in SBML L3V2 and RoadRunner. | **Keep explicitly scoped (`UnsupportedFeature`)** with clear diagnostics. |
| **5. Submodels, Imports & Hierarchy (`model ...`, `import`, `is`)** | **Large architectural expansion.** | Requires multi-module namespace resolution, port mapping, and flattening contracts. | **Defer** out of the flat-model milestone; reject explicitly. |

## 3. Recommended Next Work: Robustness & External-Model Evaluation

1. **Curated Independent Flat-Model Evaluation:**
   - Assemble an independently sourced set of hand-written and textbook pathway models (glycolysis motifs, covalent modification cycles, Goldbeter-Koshland switches, repressilator/feedback oscillators, conserved-moiety signaling cascades using `function`, `:=`, `'=`, and `piecewise`) that were not used to design the Phase 1–5 fixtures.
   - Verify exact structural parity against Antimony `3.2.0` / `libSBML` `5.21.2`, byte-idempotent `format_antimony` roundtrips, and closed-form or invariant numerical checks in the development-only RoadRunner harness.
2. **Adversarial & Resource-Bound Stress Testing:**
   - Expand property/adversarial tests around deep call chains of user-defined functions, interleaved `:=` rule replacements, large stoichiometry expressions, and malformed inputs to guarantee deterministic, panic-free termination within `ParseLimits` and `ValidationLimits`.
3. **Explicit Scope Guard Audit:**
   - Audit every deferred construct (`at` events, `0 = ...` algebraic rules, `c := ...` / `c' = ...` dynamic compartments, reaction-ID rule replacement `J0 := ...`, `import`, submodel instantiation, `is` annotations, and `rateOf`) to ensure each produces a clear, actionable `DiagnosticCode::UnsupportedFeature` diagnostic rather than a generic syntax failure or silent drop.
