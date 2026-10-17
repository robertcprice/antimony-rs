# Verified Bounded API Notes: `rules-core-v0.4-slice`

> **Status:** Verified bounded user guide for `rules-core-v0.4-slice` (`IMPLEMENTED_PROFILE`).
> **Independent bounded verification passed (2026-10-14).** Fourteen maintained gates passed; this is not a full-language release signoff or simulation readiness claim. See the [verified scope](RULES.md) and [independent review](reviews/rules-review.md). See [`src/lib.rs`](../src/lib.rs), [`src/model.rs`](../src/model.rs), and [`docs/RULES_REFERENCE.md`](RULES_REFERENCE.md) for the crate boundary and reconciled reference contract.

## Scope and Supported Rule Syntax

- **Assignment rules (`:=`):** `x := expr;`, `species S := expr;`, `var p := expr;`, `const S := expr;`, `species $S := expr;` lower to [`Rule`](../src/model.rs) entries with [`RuleKind::Assignment`](../src/model.rs) targeting a species or parameter.
- **Rate rules (`'=`):** `x' = expr;`, `species S' = expr;`, `var p' = expr;`, `const S' = expr;`, `species $S' = expr;` lower to [`Rule`](../src/model.rs) entries with [`RuleKind::Rate`](../src/model.rs) targeting a species or parameter.
- **Lexical operator boundaries:** `:` and `=` in `:=`, and `'` and `=` in `'=`, must be contiguous tokens without intervening whitespace or comments. A newline immediately before `:=`, before `'=`, or immediately after `:=` / `'=` before the RHS expression rejects as a syntax error (`DiagnosticCode::Syntax`), whereas a newline inside parenthesized or multi-line RHS expressions after the first RHS token is valid.
- **Public IR accessors on [`ValidatedModel`](../src/model.rs):**
  - `ValidatedModel::rules(&self) -> &[Rule]` returns ordered rules in upstream target first-seen order.
  - `Rule::kind(&self) -> RuleKind` returns `RuleKind::Assignment` or `RuleKind::Rate`.
  - `Rule::target(&self) -> &str` returns the target species or parameter identifier.
  - `Rule::expression(&self) -> ExpressionId` returns the model-scoped root expression ID in `ValidatedModel::expression(id)`.
  - `Rule::source_span(&self) -> Span` returns the source span of the authoritative rule statement.

## Ownership, Ordering, Scalar Attributes, and Boundary Promotion

1. **Rules never become `InitialAssignment`s:** Ongoing assignment (`:=`) and rate (`'=`) rules are stored exclusively in `ValidatedModel::rules()` and exported under SBML `<listOfRules>`. An assignment rule clears any earlier ordinary `=` initialization on the same target. An ordinary `=` after `:=` rejects (`DiagnosticCode::DeclarationConflict`). A rate rule (`'=`) retains an independent ordinary `=` initial value or `InitialAssignment` (`x = 4; x' = -k * x;` or `x' = -k * x; x = 4;`).
2. **Same-kind replacement and target first-seen ordering:** Repeating a rule of the **same** kind (`x := 1; x := 2;` or `x' = 1; x' = 2;`) replaces the authoritative formula while retaining the target's first-seen position and any symbols inferred from superseded expressions (`a := u; a := 2;` retains parameter `u`). Mixing `:=` and `'=` on the same target rejects (`DiagnosticCode::DeclarationConflict`). Exported rules follow the target symbol's first appearance in the source, not rule-statement or dependency order.
3. **Lexical scalar-export metadata vs. normalized expression:** Upstream Antimony populates the SBML scalar attribute (`value` on `<parameter>`, `initialConcentration` on `<species>`) for an assignment rule target **only** when the source RHS is a bare numeric literal (`a := 2.5;`) or unary-minus bare numeric literal (`a := -2;`). Grouped literals (`a := (2.5);`), unary plus (`a := +2.5;`), rationals (`a := (1/2);`), Booleans (`a := true;`), constants (`a := pi;`), and general expressions (`a := b + 1;`) leave `Parameter::value()` / `Species::initial_concentration()` as `None`, even when the normalized MathML tree is identical. Rate rules never populate target initial values from their derivative RHS.
4. **Source-order-sensitive species boundary promotion:** A symbol first created as an inferred parameter by a rule and subsequently promoted to a species (`a := 2; species a;` or inline `species S := 2;`) becomes a boundary species (`boundary_condition() == true`), unless a subsequent `var species a;` clears the boundary flag back to `false`. A species declared **before** its rule (`species S; S := 2;`) remains a floating species (`boundary_condition() == false`) unless marked `const` or `$`. Non-boundary ruled species may appear as reaction modifiers (`J: A -> B; S * A;`) but cannot appear as reactants or products (`DiagnosticCode::DeclarationConflict`).
5. **Transient and static dependency cycles:** Assignment rules participate with `=` initializers in static all-branch dependency cycle checking, including prefix checks at each assignment statement so transient cycles (`a := b; b := a; a := 1;` or `a = b; b := a; a = 1;`) reject immediately (`DiagnosticCode::InitializationCycle`). Rate rules represent `dx/dt` and therefore allow self- and mutual feedback (`x' = -k * x;`), while an assignment rule may depend on a rate-rule target (`y := 2 * x;`).
6. **Explicitly deferred (`DiagnosticCode::UnsupportedFeature`):** Empty rules (`a := ;`, `a' = ;`), algebraic rules (`0 = x + y;`), compartment rules (`compartment C = 1; C := 2;`), reaction kinetic-law replacement (`J := ...`), events, user-defined functions, `delay`, and submodel hierarchy.

## Core-Only Rust Example (`--no-default-features`)

`Diagnostic` does not implement `std::error::Error`; handle results explicitly or use `.expect(...)` in fixed examples rather than `?` into `Box<dyn std::error::Error>`.

```rust
use antimony::{
    ExpressionKind, IMPLEMENTED_PROFILE, Limits, RuleKind, SourceFile, SourceId, analyze, parse,
};

fn main() {
    let text = "\
model rules_demo
  species S = 0;
  x = 4;
  k = 0.5;
  y := 2 * x;
  literal_rule := -2.5;
  grouped_rule := (-2.5);
  x' = -k * x;
  J: => S; y;
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
    assert_eq!(param("x").value(), Some(4.0));
    assert!(!param("x").constant());
    assert_eq!(param("y").value(), None);
    assert!(!param("y").constant());
    assert_eq!(param("literal_rule").value(), Some(-2.5));
    assert_eq!(param("grouped_rule").value(), None);

    let rules = model.rules();
    assert_eq!(rules.len(), 4);
    assert_eq!(
        rules
            .iter()
            .map(|r| (r.kind(), r.target()))
            .collect::<Vec<_>>(),
        vec![
            (RuleKind::Rate, "x"),
            (RuleKind::Assignment, "y"),
            (RuleKind::Assignment, "literal_rule"),
            (RuleKind::Assignment, "grouped_rule"),
        ]
    );
    assert!(matches!(
        model
            .expression(rules[0].expression())
            .expect("rate rule expression")
            .kind(),
        ExpressionKind::Binary { .. }
    ));
}
```
