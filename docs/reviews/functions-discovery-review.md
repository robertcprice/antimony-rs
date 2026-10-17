# Independent Review: Phase 5 User-Defined Functions Discovery

**Reviewer:** Independent Discovery & Verification Audit  
**Scope:** Upstream Antimony `3.2.0` + libSBML `5.21.2` behavior for user-defined functions (`function name(params) body end`)  
**Evidence Artifacts:**
- [`artifacts/functions-phase5/discovery/reference.json`](../../artifacts/functions-phase5/discovery/reference.json) (78 discovery probes)
- [`fixtures/functions/manifest.json`](../../fixtures/functions/manifest.json) (164 frozen corpus cases: 68 positive exports, 96 expected rejections)

## 1. Executive Summary

Reference discovery probed 78 distinct Antimony inputs across declaration placement, parameter lists, body expressions, call sites, lexical scoping, global symbol collisions, recursion, rational call ambiguity, empty bodies, nonfinite literals, and unit propagation, then expanded into the 164-case frozen corpus (`fixtures/functions/manifest.json`) before native verification.

All findings in [`docs/FUNCTIONS_REFERENCE.md`](../FUNCTIONS_REFERENCE.md) were verified against pinned Antimony `3.2.0` and libSBML `5.21.2`.

## 2. Key Findings Verified

1. **Strict `<bvar>` lexical isolation and shadowing:**
   - Function bodies in SBML `<lambda>` may only reference their own `<bvar>` formal parameters, constants (`pi`, `exponentiale`), literals, builtins, and previously declared user-defined functions.
   - `time` and global model symbols (`species`, `parameter`, `compartment`) are rejected inside function bodies (`DiagnosticCode::DeclarationConflict`).
   - Formal parameters shadow same-named model symbols locally without polluting the model's symbol table.

2. **Declaration-order call resolution and recursion rejection:**
   - Functions must be declared prior to any call site in another function or in the model. Forward calls and mutual recursion reject (`DiagnosticCode::UnsupportedFeature`), and direct self-recursion rejects (`DiagnosticCode::DeclarationConflict`).

3. **Global `SId` namespace collisions including generated `_J0`:**
   - Upstream Antimony places `<functionDefinition id="...">` in the global SBML `SId` namespace alongside `<model>`, `<compartment>`, `<species>`, `<parameter>`, `<reaction>`, and checks collisions with `<unitDefinition>`.
   - Notably, when `function _J0(x) x + 1 end` is declared and an unnamed reaction `A -> ; 0.1 * A;` is present in `__main`, upstream generates reaction ID `_J0` and rejects the collision (`reject_collide_fn_then_generated_reaction_id`).

4. **Single-argument rational call syntax ambiguity:**
   - Upstream Antimony's rational-literal parser consumes `(1/2)` immediately after an identifier, so `f(1/2)` fails as a syntax error (`reject_single_arg_rational_call_ambi`), whereas `f((1/2))` or `f(1/2, 3)` succeeds.

5. **Empty bodies and nonfinite literals inside function bodies:**
   - `function f(x) end` and `function f(x) ; end` load upstream and emit a `<functionDefinition>` without `<math>` (`PROJECTION_UNSUPPORTED`), and `inf`/`nan`/`(1/0)` inside function bodies emit nonfinite MathML (`REFERENCE_NONFINITE`). Rejecting both explicitly with `DiagnosticCode::UnsupportedFeature` preserves sound fail-closed boundaries.

## 3. Conclusion

The reconciled contract in [`docs/FUNCTIONS_REFERENCE.md`](../FUNCTIONS_REFERENCE.md) and [`docs/FUNCTIONS_EVIDENCE_PLAN.md`](../FUNCTIONS_EVIDENCE_PLAN.md) provides a complete, reference-grounded specification for `functions-core-v0.5-slice`.
