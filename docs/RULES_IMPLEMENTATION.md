# Assignment and rate rules implementation — Phase 4

Status: independently verified bounded checkpoint (2026-10-14). The four-configuration
native Rust test matrix and all 14 immutable-artifact gates passed; Python 467 tests
passed. See the [current evidence guide](RULES.md) and
[independent final review](reviews/rules-review.md). The implemented identity is
`rules-core-v0.4-slice`, not full Antimony compatibility. Reference discovery and
contract reconciliation are recorded in [RULES_REFERENCE.md](RULES_REFERENCE.md) and
[RULES_EVIDENCE_PLAN.md](RULES_EVIDENCE_PLAN.md). Historical receipts, fixtures, and
immutable binaries remain preserved.

## Contract and public representation

The public model (`src/model.rs`, re-exported by `src/lib.rs`) adds:
- `RuleKind::{Assignment, Rate}`
- `Rule` with accessors `kind()`, `target()`, `expression()`, and `source_span()`
- `ValidatedModel::rules(&self) -> &[Rule]`

Rules are stored separately from `InitialAssignment`s and never converted into
`InitialAssignment` entries. In SBML export (`src/sbml.rs`), `ValidatedModel::rules()`
is emitted under `<listOfRules>` as `<assignmentRule variable="...">` and
`<rateRule variable="...">` elements immediately before `<listOfReactions>`.

## Syntax, lexical boundaries, and canonical formatting

- **Lexer (`src/syntax/lexer.rs`):** Adds `TokenKind::Apostrophe` (`'`).
- **Parser (`src/syntax/parser.rs`, `src/syntax/mod.rs`):** Adds `Statement::Rule { name, kind, expression, qualifier, span }`.
  - `:=` requires contiguous `:` and `=` tokens (`colon.span.end() == eq.span.start()`) with no newline between target and `:`.
  - `'=` requires contiguous `'` and `=` tokens (`apos.span.end() == eq.span.start()`) with no newline between target and `'`.
  - Inline rule declarations (`species S := expr;`, `var p' = expr;`, `const S := expr;`, `species $S := expr;`) desugar into `[Statement::Rule, Statement::Declaration]` in source order so rule-target inference and subsequent species/qualifier promotion match upstream Antimony.
  - Empty rule RHS (`a := ;`, `a' = ;`) is explicitly rejected with `DiagnosticCode::UnsupportedFeature` rather than silently clearing/discarding a rule.
  - Algebraic rules (`0 = expr;`), compartment rules, and reaction rules are explicitly rejected with `DiagnosticCode::UnsupportedFeature`.
  - Reserved builtin identifiers (`time := 1;`, `time' = 1;`) reject with `DiagnosticCode::UnsupportedFeature`.
- **Formatter (`src/format/antimony.rs`):** Emits canonical `target := expr;` and `target' = expr;`, preserving qualifier prefixes and byte-idempotence on reparse.

## Source-order symbol resolution, lexical scalars, and transient cycle checking

`src/semantic/symbols.rs` resolves statements sequentially in source order:
1. **First-seen ordering and same-kind replacement:** `first_seen_order: Vec<String>` records the first appearance of every symbol. `rules: BTreeMap<String, RuleRecord>` stores the authoritative `RuleRecord { kind, expression, lexical_scalar, span }`. Repeating a rule of the same `RuleKind` updates the `RuleRecord` while retaining `first_seen_order` and keeping any symbols inferred by the superseded expression (`infer_expression` runs before replacement). Mixing `RuleKind::Assignment` and `RuleKind::Rate` on the same target rejects (`DiagnosticCode::DeclarationConflict`).
2. **Interaction with ordinary `=` initializers:**
   - An assignment rule (`:=`) removes any prior entry for that target in `assignments`.
   - An ordinary `=` statement after `:=` on the same target rejects (`DiagnosticCode::DeclarationConflict`).
   - A rate rule (`'=`) preserves prior or subsequent ordinary `=` initializers in `assignments`.
3. **Lexical scalar-export classification (`src/semantic/expression_normalization.rs`):**
   `lexical_rule_scalar(document, expr_id)` inspects the **unnormalized** parsed `ExprKind` before normalization collapses parentheses or unary signs. Only `ExprKind::Number` or `ExprKind::Unary { op: UnaryOp::Minus, child }` wrapping `ExprKind::Number` yields `Some(f64)`. Grouped `(2.5)`, unary `+2.5`, rationals `(1/2)`, Booleans, constants, and arithmetic expressions yield `None`.
4. **Source-order species boundary promotion (`src/semantic/symbols.rs`):**
   - When a symbol already in `rules` as a non-species (`Symbol::Parameter`) is later declared `Symbol::Species` (without an explicit `Qualifier::Var`), `boundary.insert(name)` promotes it to `boundaryCondition="true"`.
   - A later `var` or `var species` declaration on a ruled species removes it from `boundary`, clearing `boundaryCondition` back to `false`.
   - A pre-declared floating species (`species S; S := 2;`) was already `Symbol::Species` before the rule, so it remains `boundaryCondition="false"` unless marked `const` or `$`.
   - Any non-boundary (`!boundary.contains(name)`) species with a rule that appears as a reaction reactant or product (either before or after the rule) rejects with `DiagnosticCode::DeclarationConflict`.
5. **Bounded iterative transient and final dependency cycle checks (`src/semantic/symbols.rs`, `src/semantic/initialization.rs`):**
   - `check_transient_cycle` performs an iterative work-stack reachability walk over active `=` and `:=` edges whenever a rule is present, rejecting transient cycles (`a := b; b := a; a := 1;` or `a = b; b := a; a = 1;`) immediately with `DiagnosticCode::InitializationCycle`.
   - `src/semantic/initialization.rs` includes active `RuleKind::Assignment` expressions alongside `=` initializers in the static Kahn topological sort (`DiagnosticCode::InitializationCycle`), evaluates initializers in topological order so `=` initializers can read lexical scalar values of `:=` targets, and populates `Parameter::value()` / `Species::initial_concentration()` for `:=` targets strictly from `lexical_scalar`.

## Conservative units and SBML export

- **Units (`src/semantic/unit_mapping.rs`, `src/semantic/units.rs`):**
  - Rule expressions are lowered and dimension-checked alongside initializers and reaction rates.
  - For `RuleKind::Assignment`, known target and RHS dimensions must match (`target == value`).
  - For `RuleKind::Rate`, when `time_unit` is known (`second` by default or explicit model `time_unit`), known target and RHS dimensions must satisfy `value == target - time_dim` (`DiagnosticCode::UnitsInconsistency`).
- **SBML export (`src/sbml.rs`):**
  - Targets of `Assignment` and `Rate` rules are exported with `constant="false"`.
  - `<listOfRules>` is emitted in target `first_seen_order` using `<assignmentRule variable="...">` and `<rateRule variable="...">` with full MathML 2.0 payloads.
