# Verified Bounded API Notes: `conditional-core-v0.3-slice`

> **Status:** Verified bounded user guide for `conditional-core-v0.3-slice` (`IMPLEMENTED_PROFILE`).
> **Independent bounded verification passed (2026-10-14).** Twelve maintained gates passed; this is not a full-language release signoff or simulation readiness claim. See the [verified scope](CONDITIONALS.md) and [independent review](reviews/conditionals-review.md). See [`src/lib.rs`](../src/lib.rs), [`src/model.rs`](../src/model.rs), and [`docs/CONDITIONALS_REFERENCE.md`](CONDITIONALS_REFERENCE.md) for the crate boundary and post-rational reference contract.

## Scope and Supported Conditional Syntax

- **Boolean constants:** `true`, `True`, `TRUE`, `false`, `False`, `FALSE` (`ExpressionKind::Boolean(bool)`). Mixed-case spellings such as `TrUe` resolve as ordinary symbol identifiers.
- **Boolean call aliases:** Exactly `True()`, `TRUE()`, `False()`, and `FALSE()` with zero arguments are Boolean leaves. Lowercase calls and all nonzero-argument forms reject `Syntax`; upstream load success with null MathML is not valid positive export evidence.
- **Relational comparisons:** Infix `<`, `<=`, `>`, `>=`, `==`, `!=` and direct functional calls `eq`, `equals`, `gt`, `geq`, `lt`, `leq` (2 or more arguments) and `neq` (1st-class `ComparisonOp::NotEqual`, exactly 2 arguments) lower to `ExpressionKind::Compare { op, args }`.
- **Logical operators and calls:** Symbolic unary `!` and functional `not(x)` lower to `ExpressionKind::Not { child }`. Symbolic `&&` / `||` and functional `and(...)` / `or(...)` lower to `ExpressionKind::Logical { op, args }`. Empty `and()` (`1.0`) and `or()` (`0.0`) preserve their zero-argument AST identities; double `not` is not simplified away.
- **Shared call-opening rational guard:** `not(1/2)` and `not (1/2)` reject as upstream rational-call syntax errors (`DiagnosticCode::Syntax`), whereas `not((1/2))` (exact `Rational` child) and `not((1)/2)` (division child) are accepted.
- **Piecewise expressions:** `piecewise(val1, cond1, ..., [otherwise])` lowers to `ExpressionKind::Piecewise { branches, otherwise }` with ordered `PiecewiseBranch { value, condition }` pairs and an optional fallback (`Option<ExpressionId>`), including single-argument otherwise-only `piecewise(default_val)`.

## Precedence, Normalization, and Deferred Constructs

- **Equal-precedence left-associative `&&` and `||`:** Relational operators bind tighter than `&&`/`||`, and unary `!` binds tighter than comparisons (`!1 < 2` parses as `(!1) < 2`). Because `&&` and `||` share the same precedence and associate left, `true || false && false` normalizes as `(true || false) && false` (`0.0`), while `true || (false && false)` retains its right group (`1.0`).
- **Call versus infix normalization:** An outer infix `&&` or `||` appends to a same-kind left logical node (`and(true, false) && true` has 3 children; `and() && true` has 1 child), but never flattens a right child (`true && and(false, true)` stays 2-arg nested) and an outer functional call (`and(and(true, false), true)`) never flattens its arguments.
- **Explicitly deferred (`DiagnosticCode::UnsupportedFeature`):**
  - Textual logical aliases (`and`, `or`, `not` used as bare infix/prefix words, e.g., `a and b`, `not x`), `xor(...)`, and `implies(...)`.
  - Infix comparison chains (`1 < 2 < 3`, `(1 < 2) < 3`, `1 < 2 <= 3`, `1 != 2 != 3`, `lt(1, 2) < 3`, `and(lt(1, 2), gt(3, 2)) < 4`), whereas right-nested `1 < (2 < 3)` is a valid Boolean-to-numeric coercion (`0.0`).
  - Assignment/rate rules, events, user-defined functions, `delay`, imports, and compartment expressions.

## Static All-Branch Validation vs. Demand-Driven Initialization

1. **Static validation inspects every branch and operand:** Symbol resolution, reaction modifier inference, dependency cycle detection (`DiagnosticCode::InitializationCycle`, e.g., `a = piecewise(a, false, 1)`), conservative unit/dimension checking (`DiagnosticCode::UnitsInconsistency`), and lexical/rational literal bounds (`1e309`, `(1/0)`) are checked across **all** branches, even when guarded by a known `false` condition.
2. **Lazy numeric initializer evaluation:** Initializer evaluation traverses conditions in order and demands only the selected branch. Inactive runtime domain errors such as `sqrt(-1)`, `1 / 0`, or `ln(0)` in unselected `piecewise` branches or short-circuited `&&`/`||` operands are not evaluated.
3. **Unknown is never `false` or `0.0`:** Unset symbols (`formula u;`, `species S;`) and `time` / `time()` produce a symbolic unknown state (`None` on `Parameter::value()` / `Species::initial_concentration()`), while retaining their `initial_expression()` and `InitialAssignment` AST. In `piecewise` and sequential `&&`/`||`, stopping at the first unknown condition or operand yields `None` without evaluating later potentially invalid branches or folding identical alternatives (`piecewise(5, u == 0, 5)` stays `None`). N-ary comparisons evaluate demanded arguments eagerly and still reject known domain errors alongside an unknown argument.
4. **Missing `otherwise` in initializers vs. reaction rates:** In an initializer, a `piecewise` without `otherwise` succeeds if a prior condition evaluates to true, returns `None` if condition selection is symbolic/unknown, and **rejects** if all conditions are known false (no finite initial value). In reaction rate laws, an omitted `otherwise` is preserved structurally in the IR and MathML; structural acceptance is **not** a runtime or solver readiness proof.

## Core-Only Rust Example (`--no-default-features`)

`Diagnostic` does not implement `std::error::Error`; handle results explicitly or use `.expect(...)` in fixed examples rather than `?` into `Box<dyn std::error::Error>`.

```rust
use antimony::{
    ExpressionKind, IMPLEMENTED_PROFILE, Limits, PiecewiseBranch, SourceFile, SourceId, analyze,
    parse,
};

fn main() {
    let text = "\
model conditional_demo
  formula u;
  selected = piecewise(7, true, sqrt(-1));
  time_dep = piecewise(5, time > 0, 1);
  unset_cond = piecewise(10, u > 0, 20);
end";
    let limits = Limits::default();
    let source = SourceFile::new(SourceId::new(1), "demo", text, &limits).expect("valid source");
    let doc = parse(&source, &limits).expect("parse succeeds");
    let model = analyze(&doc).expect("analyze succeeds");
    assert_eq!(model.profile_id(), IMPLEMENTED_PROFILE);

    let param = |id: &str| {
        model
            .parameters()
            .iter()
            .find(|p| p.id() == id)
            .expect("parameter exists")
    };
    assert_eq!(param("selected").value(), Some(7.0));
    assert_eq!(param("time_dep").value(), None);
    assert_eq!(param("unset_cond").value(), None);

    let expr_id = param("selected").initial_expression().expect("expression id");
    let ExpressionKind::Piecewise {
        branches,
        otherwise: Some(fallback),
    } = model.expression(expr_id).expect("expression node").kind()
    else {
        panic!("expected piecewise expression with otherwise");
    };
    let [PiecewiseBranch { value, condition }] = branches.as_slice() else {
        panic!("expected one conditional branch");
    };
    assert!(matches!(
        model.expression(*condition).expect("condition node").kind(),
        ExpressionKind::Boolean(true)
    ));
    assert!(matches!(
        model.expression(*value).expect("value node").kind(),
        ExpressionKind::Number { value, .. } if *value == 7.0
    ));
    assert!(matches!(
        model.expression(*fallback).expect("fallback node").kind(),
        ExpressionKind::Call { .. }
    ));
}
```
