# User-Defined Functions Reference Behavior (`functions-core-v0.5-slice`)

This document records the reconciled reference contract for user-defined functions (`function name(params) body end`) against upstream Antimony `3.2.0` and libSBML `5.21.2` (`.venv-oracle/bin/python`).

## 1. Raw Discovery Evidence

Reference behavior was probed against pinned Antimony `3.2.0` + libSBML `5.21.2` before native implementation:

- **Primary discovery:** [`artifacts/functions-phase5/discovery/reference.json`](../artifacts/functions-phase5/discovery/reference.json) (**78 probed models** spanning declaration syntax, formal parameter scope, call sites, ordering, collisions, recursion, units, and edge cases).
- **Frozen functions corpus:** [`fixtures/functions/manifest.json`](../fixtures/functions/manifest.json) (**164 frozen cases**: **68 positive exports**, **96 expected rejections**) with exact reference snapshots in `fixtures/functions/reference/*.json`.

## 2. Reconciled Reference Findings

1. **Declaration placement and SBML emission order:**
   - Upstream Antimony accepts `function name(p1, p2, ...) body end` before `model ... end`, after `model ... end`, inside `model ... end`, or in implicit `__main` files without a `model` wrapper.
   - All declared functions (including unused ones) are emitted in `<listOfFunctionDefinitions>` in **source declaration order** as `<functionDefinition id="...">` containing `<math><lambda><bvar><ci>...</ci></bvar>...<body></lambda></math>` (`AST_LAMBDA` = `267`).
   - Calls to user-defined functions emit `<apply><ci>name</ci>...</apply>` (`AST_FUNCTION` = `268`).

2. **Header syntax, separators, and single-expression bodies:**
   - Zero or more comma-separated formal parameter identifiers are permitted (`function f() 42 end`, `function f(x, y) x + y end`). Trailing commas (`function f(x,) x end`), leading commas, or missing parentheses reject upstream (`DiagnosticCode::Syntax`).
   - Optional semicolons/newlines after `)` and before `end`, and an optional semicolon after `end`, are valid (`function f(x); x + 1; end;`).
   - Multi-statement bodies (`x; x + 1`), assignments (`y = x`), rules (`y := x`, `y' = x`), or reactions inside a function body reject upstream (`DiagnosticCode::Syntax`).
   - Empty function bodies (`function f(x) end`, `function f(x) ; end`) load in Antimony `3.2.0` and emit a `<functionDefinition>` without a `<math>` child (`PROJECTION_UNSUPPORTED`). Under `functions-core-v0.5-slice`, empty function bodies are explicitly rejected with `DiagnosticCode::UnsupportedFeature`.
   - `function *f(x) x end` is rejected upstream (`DiagnosticCode::Syntax`).

3. **Pure lexical scope (`<bvar>`) and global symbol isolation:**
   - Inside a function body, only the function's own formal parameters, numeric/rational/Boolean literals, mathematical constants (`pi`, `exponentiale`), supported builtins, and **previously declared** user-defined functions are in scope.
   - Referencing `time`, undeclared free identifiers, or model-level parameters/species/compartments inside a function body is rejected upstream (`DiagnosticCode::DeclarationConflict`).
   - Formal parameter names are strictly local `<bvar>` bindings: they can shadow global species, parameters, compartments, or reactions (`function mm(S, Vmax, Km) ... end` with `species S = 4;`) without creating or mutating any model symbols.

4. **Declaration-order call graph and recursion:**
   - Functions must be declared before they are called (`reject_forward_call_in_model`, `reject_forward_call_top_level`, `reject_forward_call_between_functions`). Calling an undeclared or not-yet-declared function rejects with `DiagnosticCode::UnsupportedFeature`.
   - Direct recursion (`function f(x) f(x) end`, even inside `piecewise`) is rejected upstream (`DiagnosticCode::DeclarationConflict`). Mutual recursion requires a forward call and rejects (`DiagnosticCode::UnsupportedFeature`).

5. **Global `SId` uniqueness and arity checks:**
   - Redefining a function name (with the same or different arity) is rejected upstream (`DiagnosticCode::DeclarationConflict`).
   - Duplicate formal parameter names (`function f(x, x)`), formal parameter names matching the function's own name (`function f(f)`), or formal parameter names matching an earlier function's name (`function f(x) x end; function g(f) f end`) are rejected upstream (`DiagnosticCode::DeclarationConflict`).
   - Function names share the SBML `SId` namespace with the model ID (`__main` or explicit `model m`), compartments, species, parameters, reactions (including auto-generated `_J0`), and unit definitions; any collision in either declaration order rejects (`DiagnosticCode::DeclarationConflict`).
   - Calling a declared function with too few or too many arguments, or calling a formal parameter / model symbol as a function, rejects (`DiagnosticCode::DeclarationConflict`).
   - Referencing a declared function name as a bare variable (`a = f;`) or assigning/ruling a function name (`f = 1;`, `f := 1;`) rejects (`DiagnosticCode::Syntax`).
   - Single-argument call with rational syntax `f(1/2)` is rejected by upstream Antimony's rational-token parser (`DiagnosticCode::Syntax`); grouped `f((1/2))` or `f((1)/2)` is valid.

6. **Initial-value evaluation, scalar attributes, and dependency cycles:**
   - Assignment rules (`:=`) with a function-call RHS (`a := f(1);`) are non-literal expressions and therefore leave the target's SBML scalar attribute (`value` / `initialConcentration`) unset (`None`).
   - Algebraic dependency cycles through function call arguments in `=` initializers (`a = id(b); b = id(a);`) or `:=` assignment rules (`a := id(b); b := id(a);`, `a := id(a);`, and transient cycles `a := id(b); b := id(a); a := 1;`) reject (`DiagnosticCode::InitializationCycle`).
   - Demonstrated division by zero or domain errors when evaluating a `=` initializer call (`function f(x) 1 / x end; a = f(0);` or `ln(-1)`) reject (`DiagnosticCode::DeclarationConflict`).

7. **Conservative call-site unit checking:**
   - Substituting caller argument units into the callee's formal parameters at each call site detects dimensional mismatches inside the function body (`x + y` called with `second` and `metre`, `sin(x)` called with `second`, `x ^ y` with dimensioned exponent) and between the function's return dimension and the target symbol's unit (`DiagnosticCode::UnitsInconsistency`).

## 3. Native Contract Summary (`functions-core-v0.5-slice`)

- **Public IR (`src/model.rs`, `src/lib.rs`):**
  - `FunctionDefinition` (`id()`, `parameters()`, `body()`, `source_span()`)
  - `ValidatedModel::function_definitions(&self) -> &[FunctionDefinition]`
  - `SymbolKind::FunctionParameter`
  - `ExpressionKind::UserCall { name: String, args: Vec<ExpressionId> }`
