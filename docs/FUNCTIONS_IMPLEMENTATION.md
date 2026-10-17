# User-defined functions implementation — Phase 5

Status: independently verified bounded checkpoint (2026-10-14). The four-configuration
native Rust test matrix and all 16 immutable-artifact gates passed; Python 490 tests
passed. See the [current evidence guide](FUNCTIONS.md) and
[independent final review](reviews/functions-review.md). The implemented identity is
`functions-core-v0.5-slice`, not full Antimony compatibility. Reference discovery and
contract reconciliation are recorded in [FUNCTIONS_REFERENCE.md](FUNCTIONS_REFERENCE.md) and
[FUNCTIONS_EVIDENCE_PLAN.md](FUNCTIONS_EVIDENCE_PLAN.md). Historical receipts, fixtures, and
immutable binaries remain preserved.

## Contract and public representation

The public model (`src/model.rs`, re-exported by `src/lib.rs`) adds:
- `FunctionDefinition` with accessors `id()`, `parameters()`, `body()`, and `source_span()`
- `ValidatedModel::function_definitions(&self) -> &[FunctionDefinition]`
- `SymbolKind::FunctionParameter` for `<bvar>` formal parameter references inside function bodies
- `ExpressionKind::UserCall { name: String, args: Vec<ExpressionId> }` for calls to user-defined functions

In SBML export (`src/sbml.rs`), `ValidatedModel::function_definitions()` is emitted under
`<listOfFunctionDefinitions>` as `<functionDefinition id="...">` containing MathML 2.0
`<lambda>` with `<bvar><ci>...</ci></bvar>` children and the body expression immediately
before `<listOfUnitDefinitions>`. Calls (`ExpressionKind::UserCall`) emit
`<apply><ci>name</ci>...</apply>` (`AST_FUNCTION`).

## Syntax, lexical scope, and canonical formatting

- **Parser (`src/syntax/parser.rs`, `src/syntax/expression.rs`, `src/syntax/functions.rs`, `src/syntax/mod.rs`):**
  - Adds `ParsedFunction { name, parameters, body, span }` on `ParsedDocument` and `Function::User { name, arity }` in call dispatch.
  - Accepts `function name(params) body end` both at the top level (before/after `model ... end` or in implicit `__main` files) and inside a `model ... end` block.
  - Rejects `function *name(...)` (`DiagnosticCode::Syntax`), `$param` (`DiagnosticCode::UnsupportedFeature`), `var`/`const` parameter modifiers (`DiagnosticCode::Syntax`), reserved keywords/constants/units as function or parameter names, duplicate function names, duplicate parameter names, and parameter names matching the function's own name or an earlier function's name (`DiagnosticCode::DeclarationConflict`).
  - Rejects empty function bodies (`function f(x) end`, `function f(x) ; end`) with `DiagnosticCode::UnsupportedFeature`, and multi-statement or assignment/rule/reaction bodies with `DiagnosticCode::Syntax`.
  - While parsing a function body (`active_function: Some((fn_name, param_set))`), rejects `time` (`DiagnosticCode::DeclarationConflict`), free undeclared or global model identifiers (`DiagnosticCode::DeclarationConflict`), direct recursion (`DiagnosticCode::DeclarationConflict`), and calls to undeclared/later functions (`DiagnosticCode::UnsupportedFeature`).
  - Outside and inside function bodies, rejects bare references to declared function names (`a = f;`) and assignments/rules targeting a function name (`f = 1;`, `f := 1;`) with `DiagnosticCode::Syntax`, and arity mismatches on declared user functions with `DiagnosticCode::DeclarationConflict`.
- **Formatter (`src/format/antimony.rs`):** Emits canonical `function name(p1, p2)\n  body;\nend\n` blocks before the model body and formats `ExprKind::UserCall` as `name(arg1, arg2)`, preserving byte-idempotence on reparse.

## Symbol isolation, evaluation, and conservative unit checking

1. **Formal parameter isolation (`src/semantic/symbols.rs`, `src/semantic.rs`):**
   - `infer_expression` only runs on model statements (`Initial`, `Rule`, `Reaction`), never on `document.functions`, so formal parameters never create or mutate model `Parameter` or `Species` entries.
   - `src/semantic/symbols.rs` verifies that no `FunctionDefinition` name collides with `document.name`, any model symbol in `symbols` (including generated `_J0` reaction IDs), or any `unit` definition (`DiagnosticCode::DeclarationConflict`).
   - `src/semantic.rs` tracks expression nodes belonging to each `ParsedFunction` body (`fn_nodes`) and lowers `ExprKind::Symbol` inside function bodies to `SymbolKind::FunctionParameter`.
2. **Initial-value evaluation across user calls (`src/semantic/initialization.rs`, `src/semantic/evaluation.rs`):**
   - `ExprKind::UserCall` children are traversed by `check_transient_cycle` and the static Kahn topological sort so dependency cycles through function call arguments (`a = id(b); b = id(a);`, `a := id(b); b := id(a);`) reject with `DiagnosticCode::InitializationCycle`.
   - `eval_with_env` evaluates `ExprKind::UserCall` by recursively evaluating arguments and binding the callee's formal parameters in a local `BTreeMap<&str, Option<f64>>` environment, catching division-by-zero and domain errors inside called function bodies (`DiagnosticCode::DeclarationConflict`).
3. **Call-site unit substitution (`src/semantic/unit_mapping.rs`, `src/semantic/units.rs`):**
   - `lower_expr_with_env` substitutes caller argument `UnitExpr` trees into the callee's formal parameters at each `ExprKind::UserCall` site (supporting nested calls in declaration order), so dimensional inconsistencies inside the called function body or against the target symbol's unit reject with `DiagnosticCode::UnitsInconsistency`.
