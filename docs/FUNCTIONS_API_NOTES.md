# Verified Bounded API Notes: `functions-core-v0.5-slice`

> **Status:** Verified bounded user guide for `functions-core-v0.5-slice` (`IMPLEMENTED_PROFILE`).
> **Independent bounded verification passed (2026-10-14).** Sixteen maintained gates passed; this is not a full-language release signoff or simulation readiness claim. See the [verified scope](FUNCTIONS.md) and [independent review](reviews/functions-review.md). See [`src/lib.rs`](../src/lib.rs), [`src/model.rs`](../src/model.rs), and [`docs/FUNCTIONS_REFERENCE.md`](FUNCTIONS_REFERENCE.md) for the crate boundary and reconciled reference contract.

## Scope and Supported User-Defined Function Syntax

- **Function declarations (`function name(params) body end`):**
  - Top-level and inside-`model` declarations (`function mm(S, Vmax, Km) Vmax * S / (Km + S) end`, `function c0() 42 end`) lower to [`FunctionDefinition`](../src/model.rs) entries on [`ValidatedModel`](../src/model.rs) in declaration order.
  - Zero or more comma-separated formal parameters (`x`, `x, y`, `s, vmax, km`).
  - Exactly one body expression before `end` (an optional `;` after `)` and before `end`, and an optional `;` after `end`, are accepted).
- **Public IR accessors on [`ValidatedModel`](../src/model.rs):**
  - `ValidatedModel::function_definitions(&self) -> &[FunctionDefinition]` returns ordered user-defined functions in declaration order.
  - `FunctionDefinition::id(&self) -> &str` returns the function identifier (`SId`).
  - `FunctionDefinition::parameters(&self) -> &[String]` returns the ordered formal parameter names (`<bvar>`).
  - `FunctionDefinition::body(&self) -> ExpressionId` returns the root expression ID of the `<lambda>` body in `ValidatedModel::expression(id)`.
  - `FunctionDefinition::source_span(&self) -> Span` returns the source span of the `function ... end` declaration.
  - `SymbolKind::FunctionParameter` identifies formal parameter occurrences (`<bvar>` `ci` references) inside a function body expression.
  - `ExpressionKind::UserCall { name, args }` represents calls to user-defined functions (`AST_FUNCTION`) in initializers, rules, reaction kinetic laws, and subsequent function bodies.

## Lexical Scope, Ordering, Evaluation, and Unit Propagation

1. **Pure lexical isolation (`<bvar>` scope):** Function bodies may reference only their own formal parameters (`SymbolKind::FunctionParameter`), numeric literals/rationals, mathematical constants (`pi`, `exponentiale`), Boolean literals (`true`, `false`, `True()`, `False()`), supported numeric/logical/comparison/`piecewise` builtins, and previously declared user-defined functions. Referencing `time`, undeclared free symbols, or model parameters/species/compartments inside a function body rejects (`DiagnosticCode::DeclarationConflict`). Formal parameter names shadow same-named model symbols locally without leaking into `ValidatedModel::parameters()` or `ValidatedModel::species()`.
2. **Declaration-order non-recursive call graph:** A function must be declared before any call site in the model or in another function's body. Calling an undeclared function or calling a function before its declaration rejects (`DiagnosticCode::UnsupportedFeature`). Direct recursion (`f` calling `f`) rejects (`DiagnosticCode::DeclarationConflict`); mutual recursion requires a forward call and rejects (`DiagnosticCode::UnsupportedFeature`).
3. **Global `SId` uniqueness and arity enforcement:** Duplicate function declarations (same or different arity), duplicate formal parameter names within a header, formal parameters matching the function's own name or an earlier function's name, and collisions between a function name and the model name, `__main`, any species/parameter/compartment/reaction (including generated `_J0`), or any `unit` definition reject (`DiagnosticCode::DeclarationConflict`). Calling a declared function with the wrong argument count rejects (`DiagnosticCode::DeclarationConflict`). Using a function identifier as a bare variable or assignment/rule target rejects (`DiagnosticCode::Syntax`).
4. **Initial-value evaluation and dependency cycles:** Call-site arguments participate in static all-branch dependency and transient cycle checking (`a = id(b); b = id(a);` and `a := id(b); b := id(a);` reject with `DiagnosticCode::InitializationCycle`). When a `=` initializer calls a user-defined function whose arguments all have known initial values, `src/semantic/initialization.rs` evaluates the call body by binding formal parameters (`eval_with_env`) so downstream initializers and domain checks see the evaluated result; demonstrated division by zero or domain errors during evaluation (`f(0)` for `1 / x`, `f(-1)` for `ln(x)`) reject (`DiagnosticCode::DeclarationConflict`). Assignment rules (`:=`) whose RHS is a user-defined function call (`a := f(1);`) are non-literal expressions and leave `Parameter::value()` / `Species::initial_concentration()` as `None`.
5. **Conservative call-site unit and dimension checking:** At each `ExpressionKind::UserCall`, `src/semantic/unit_mapping.rs` substitutes the caller's argument unit trees into the callee's formal parameters (recursively across nested user calls in declaration order). Dimensional mismatches inside the body (`x + y` called with `second` and `metre`, `sin(x)` called with `second`, or return dimension incompatible with a target's declared unit) reject with `DiagnosticCode::UnitsInconsistency`.
6. **Explicitly deferred (`DiagnosticCode::UnsupportedFeature`):** Empty function bodies (`function f(x) end`, `function f(x) ; end`), `$` modifiers on formal parameters (`function f($x) x end`), nonfinite literals (`inf`, `nan`, `(1/0)`) inside function bodies, compartment size initialization or rules via function calls, reaction kinetic-law replacement (`J := f(...)`), events, `delay`, and submodel hierarchy.

## Core-Only Rust Example (`--no-default-features`)

`Diagnostic` does not implement `std::error::Error`; handle results explicitly or use `.expect(...)` in fixed examples rather than `?` into `Box<dyn std::error::Error>`.

```rust
use antimony::{
    ExpressionKind, IMPLEMENTED_PROFILE, Limits, RuleKind, SourceFile, SourceId, SymbolKind,
    analyze, parse,
};

fn main() {
    let text = "\
function base_drive() 1 end
function mm(S, Vmax, Km) Vmax * S / (Km + S) end
model functions_demo
  species S = 4, P = 0;
  Vmax = 2;
  Km = 1;
  v0 = mm(4, Vmax, Km);
  rate_obs := mm(S, Vmax, Km) + base_drive();
  J0: S => P; mm(S, Vmax, Km);
end";
    let limits = Limits::default();
    let source = SourceFile::new(SourceId::new(1), "demo", text, &limits).expect("valid source");
    let doc = parse(&source, &limits).expect("parse succeeds");
    let model = analyze(&doc).expect("analyze succeeds");
    assert_eq!(model.profile_id(), IMPLEMENTED_PROFILE);

    let fns = model.function_definitions();
    assert_eq!(fns.len(), 2);
    assert_eq!(fns[0].id(), "base_drive");
    assert!(fns[0].parameters().is_empty());
    assert_eq!(fns[1].id(), "mm");
    assert_eq!(
        fns[1].parameters(),
        &["S".to_owned(), "Vmax".to_owned(), "Km".to_owned()]
    );

    // Formal parameter `S` inside `mm` is `SymbolKind::FunctionParameter`, while model `S` remains a species.
    let body_root = model.expression(fns[1].body()).expect("mm body");
    if let ExpressionKind::Binary { left, .. } = body_root.kind() {
        let num = model.expression(*left).expect("numerator");
        if let ExpressionKind::Binary { right, .. } = num.kind() {
            assert!(matches!(
                model.expression(*right).expect("S param").kind(),
                ExpressionKind::Symbol {
                    name,
                    kind: SymbolKind::FunctionParameter,
                } if name == "S"
            ));
        }
    }

    assert_eq!(model.species().len(), 2);
    assert_eq!(model.rules().len(), 1);
    assert_eq!(model.rules()[0].kind(), RuleKind::Assignment);
    assert_eq!(model.rules()[0].target(), "rate_obs");
}
```
