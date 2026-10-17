# User-defined functions: bounded native slice

**Verified bounded checkpoint (2026-10-14).** The implemented profile is
`functions-core-v0.5-slice`; all sixteen maintained gates passed on immutable reviewed binaries.
This extends the separately bounded arithmetic, symbols, numeric builtins,
rational, conditional, and assignment/rate rule slices, not the complete Antimony language or a solver.

## What you can write

- **User-defined function declarations (`function name(params) body end`):**
  `function mm(s, vmax, km) vmax * s / (km + s) end`, `function hill(s, vmax, k, n) vmax * s^n / (k^n + s^n) end`,
  `function base_drive() 1 end`, both at the top level (before/after `model ... end` or in implicit `__main` files)
  and inside a `model ... end` block.
- **Zero or more formal parameters (`<bvar>`):** `function c0() 42 end`, `function sq(x) x^2 end`,
  `function bi_mm(a, b, vm, ka, kb) vm * a * b / ((ka + a) * (kb + b)) end`.
- **Single-expression bodies with full expression vocabulary:** Arithmetic, rationals (`(1/2)`),
  constants (`pi`, `exponentiale`), Booleans (`true`, `false`, `True()`, `False()`), numeric builtins
  (`sin`, `cos`, `exp`, `ln`, `log`, `sqrt`, `min`, `max`, ...), comparisons, logical operators,
  `piecewise(...)`, and calls to previously declared user-defined functions.
- **Calls in all model expression contexts:** Species/parameter `=` initializers (`k = f(2, 3);`),
  assignment rules (`y := affine_obs(2, S, 1);`), rate rules (`u' = base_drive();`), and reaction
  kinetic laws (`J: S => P; mm(S, Vmax, Km);`).
- **SBML L3V2 export:** Exported in declaration order under `<listOfFunctionDefinitions>` as
  `<functionDefinition id="...">` with `<math><lambda><bvar><ci>...</ci></bvar>...<body></lambda></math>`,
  and called via `<apply><ci>name</ci>...</apply>` (`AST_FUNCTION`).

## Lexical isolation, ordering, evaluation, and units

1. **Pure lexical isolation (`<bvar>` scope):** Function bodies may reference only their own formal
   parameters (`SymbolKind::FunctionParameter`), literals, constants (`pi`, `exponentiale`), builtins,
   and previously declared user-defined functions. Referencing `time`, undeclared free symbols, or
   global model symbols (species, parameters, compartments) inside a function body rejects
   (`DiagnosticCode::DeclarationConflict`). Formal parameter names shadow same-named model symbols
   locally (`function mm(S, Vmax, Km) ... end` alongside `species S = 4; Vmax = 2; Km = 1;`) without
   creating or mutating model parameters or species.
2. **Declaration-order non-recursive calls:** Functions must be declared before they are called.
   Calling an undeclared function or calling a function before its declaration rejects
   (`DiagnosticCode::UnsupportedFeature`). Direct recursion (`f` calling `f`) rejects
   (`DiagnosticCode::DeclarationConflict`), and mutual recursion rejects (`DiagnosticCode::UnsupportedFeature`).
3. **Global `SId` uniqueness and exact arity:** Duplicate function declarations (same or different arity),
   duplicate formal parameter names, formal parameter names matching the function's own name or an earlier
   function's name, and collisions between a function name and the model name, `__main`, any species,
   parameter, compartment, reaction (including generated `_J0`), or `unit` definition reject
   (`DiagnosticCode::DeclarationConflict`). Calling a declared function with the wrong number of arguments
   rejects (`DiagnosticCode::DeclarationConflict`). Using a function name as a bare variable or assignment/rule
   target rejects (`DiagnosticCode::Syntax`).
4. **Initial-value evaluation and dependency cycles:** Call-site arguments participate in static all-branch
   and transient dependency cycle checks (`a = id(b); b = id(a);` and `a := id(b); b := id(a);` reject with
   `DiagnosticCode::InitializationCycle`). When a `=` initializer calls a user-defined function with known
   argument values, the body is evaluated so downstream initializers and domain checks see the result;
   demonstrated division by zero or domain errors during evaluation (`1 / x` at `0`, `ln(x)` at `-1`) reject
   (`DiagnosticCode::DeclarationConflict`). Assignment rules (`:=`) with a function-call RHS (`a := f(1);`)
   leave `Parameter::value()` / `Species::initial_concentration()` as `None`.
5. **Conservative call-site unit checking:** At each call site, caller argument unit trees are substituted
   into the callee's formal parameters (recursively across nested user calls). Dimensional mismatches inside
   the function body or against the target symbol's unit reject (`DiagnosticCode::UnitsInconsistency`).

## Try your own ordinary file

```antimony
function lin_rate(k, x)
  k * x
end
function affine_obs(m, x, b)
  m * x + b
end
model function_kinetic_input
  species S = 4, P = 0;
  k = 0.5;
  y := affine_obs(2, S, 1);
  J: S => P; lin_rate(k, S);
end
```

Save as `function_kinetic_input.ant`, then use the general CLI (not a fixture dispatcher):

```sh
cargo run --features cli --bin antimony -- check function_kinetic_input.ant
cargo run --features cli --bin antimony -- format function_kinetic_input.ant
cargo run --features cli --bin antimony -- export function_kinetic_input.ant
```

The crate parses, analyzes, formats and exports SBML; it does **not** integrate ODEs.
The separate two-model numerical functions gate tests the closed-form solutions of
`function_kinetic_law_and_observable` ($S(t)=4e^{-0.5t}$, $P(t)=4(1-e^{-0.5t})$,
$y(t)=8e^{-0.5t}+1$) and `nested_function_rate_and_boundary_rule`
($u(t)=t$, $B(t)=1+0.2t$, $S(t)=t+0.1t^2$) on 101 grid points over $[0,10]$.
It is not a general DAE/ODE solver certificate or simulation readiness claim.

## Explicit exclusions and evidence

Empty function bodies (`function f(x) end`, `function f(x) ; end`), `$` modifiers on formal
parameters (`function f($x) x end`), forward or mutually recursive function calls, compartment
initialization/rules via function calls, reaction kinetic-law replacement (`J := f(...)`),
algebraic rules (`0 = x + y;`), events (`at(...)`), `delay`, and submodel hierarchy remain
outside this slice and reject explicitly (`DiagnosticCode::UnsupportedFeature`).
The core-only [Rust API guide](FUNCTIONS_API_NOTES.md) demonstrates
`ValidatedModel::function_definitions()`, `FunctionDefinition`, `SymbolKind::FunctionParameter`,
and `ExpressionKind::UserCall`. Reference discovery and contract reconciliation are documented in
[FUNCTIONS_REFERENCE.md](FUNCTIONS_REFERENCE.md) and [FUNCTIONS_EVIDENCE_PLAN.md](FUNCTIONS_EVIDENCE_PLAN.md).

The frozen functions corpus (`fixtures/functions/manifest.json`) contains **164 cases**:
**68 exact positive exports** and **96 expected rejections**. Positive evidence requires
exact projection equality, valid `AST_LAMBDA` (`267`) and `AST_FUNCTION` (`268`) MathML trees,
disjoint `<bvar>` parameters, global `SId` uniqueness across functions and model symbols,
no libSBML Error/Fatal messages, and byte-idempotent formatting roundtrips.

Verification is recorded in the [independent review](reviews/functions-review.md) and
[compact evidence](../tools/oracle/functions-evidence.json). Python suites passed **490 tests**
(294 oracle, 114 numerical, 51 workloads, 31 verification), zero failures/skips.
The four-configuration Rust test matrix passed: all-feature **459**, default **414**,
core-only **399** and CLI-only **444** tests, plus **2 doctests** per configuration.

| Maintained gate | Verified result |
|---|---|
| Functions (`functions-core-v0.5-slice`) | 164: 68 exact exports, 96 expected rejections |
| Rules (`rules-core-v0.4-slice`) | 200: 114 exact exports, 86 expected rejections |
| Conditional + alias supplement | 262: 159 exact exports, 103 expected rejections |
| Rational corrective | 124: 73 exact, 51 rejections |
| Original / formatter / unified CLI | 90 each: 58 supported, 32 rejections |
| Arithmetic | 54: 48 exact, 6 rejections |
| Symbols | 66: 54 exact, 12 rejections |
| Builtins | 109: 66 exact, 43 rejections |
| Workloads | 86: 74 exact, 12 rejections |
| Retained analytic / builtin-time / piecewise / rules / functions numerical | 8/8, 2/2, 1/1, 2/2, 2/2 |

The original 90-case corpus (and formatter/CLI routes), arithmetic, and workload suites bind
to `functions-core-v0.5-slice` under the explicit hash-bound overlay
`fixtures/manifests/functions-core-v5-overlay.json` (`antimony-native-overlay-v7`, retaining
58/32 on the original 90-case corpus when `--current-profile` is selected), while historical
manifests and receipts remain untouched.

## Reproduce the bounded checks

With trusted, prebuilt binaries:

```sh
export ANTIMONY_IMPLEMENTATION_PROFILE=functions-core-v0.5-slice
.venv-oracle/bin/python tools/oracle/functions_check.py \
  --trusted-native --cli artifacts/functions-phase5/bin/antimony
.venv-oracle/bin/python tools/numerical/function_check.py \
  --trusted-native --native artifacts/functions-phase5/bin/export_sbml \
  --python .venv-numerical/bin/python --reference-python .venv-oracle/bin/python
```
