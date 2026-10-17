> **Historical design proposal — superseded.** The paused/pending statements below record the earlier planning checkpoint, not current blockers. The rational repair is verified in [its evidence guide](RATIONAL_CALLS_EVIDENCE.md); current conditional scope and verification status live in [CONDITIONALS.md](CONDITIONALS.md) and the [independent review](reviews/conditionals-review.md).

# Phase 3B Conditional Expressions — Concise Design & Execution Note

**Status:** Execution-oriented design note (**not implemented**, **Phase 3B implementation PAUSED**, **tests `NOT_RUN`**).  
**Date:** 2026-10-14  
**Authority:** [`docs/CONDITIONALS_REFERENCE.md`](CONDITIONALS_REFERENCE.md) is the authoritative record of pinned reference probes (`Antimony 3.2.0` / `libSBML 5.21.2` / `RoadRunner 2.10.0`) and normative SBML Level 3 Version 2 Core evidence.

> [!WARNING]
> **Current Blocker — Phase 3B Implementation is PAUSED:**  
> Reference discovery uncovered a compatibility mismatch on `sin(1/2)` (upstream rejects as rational-notation syntax inside unary calls, whereas native currently accepts) alongside `not(1/0)` / `not(1 / 0)` vs `!(1 / 0)`. The exact upstream rational-notation grammar and broader unary/function call boundary are under active investigation (to be documented in a future `docs/RATIONAL_CALLS.md` note once created). **Do not prescribe a guessed string-pattern workaround or claim Phase 3B is ready or implemented.**

---

## 1. Verified Reference Findings (from [`docs/CONDITIONALS_REFERENCE.md`](CONDITIONALS_REFERENCE.md))

1. **Normative L3V2 Numeric/Boolean Contextual Conversion (Correction to Prior Draft):**
   - SBML L3V2 Core (Sections 3.4.4, 3.4.11–3.4.12) and pinned probes explicitly permit bidirectional finite numeric/Boolean contextual conversion:
     - **Boolean -> Numeric:** `true` $\to 1$, `false` $\to 0$ (dimensionless; e.g., `true + 2 == 3`, `sin(true)`, `-true == -1`, `true^2 == 1`, `piecewise(true, true, 9) == 1`).
     - **Numeric -> Boolean:** `0` $\to \text{false}$, any finite nonzero real (e.g., `2`, `-2`, `0.5`) $\to \text{true}$ (e.g., `piecewise(7, 0, 9) == 9`, `piecewise(7, -2, 9) == 7`, `not(0) == 1`, `not(0.5) == 0`).
   - Prior speculative advice proposing strict Boolean-only / numeric-only type rejection is withdrawn. Exported MathML preserves `<true/>`/`<false/>` and Boolean operator elements rather than rewriting them to numeric literals.
2. **Verified Spellings, Arities, and MathML:**
   - **Boolean constants:** `true`, `True`, `TRUE` (`<true/>`, AST `266`) and `false`, `False`, `FALSE` (`<false/>`, AST `264`). Mixed cases like `TrUe` are ordinary symbols.
   - **Relational operators:** Infix `<`, `<=`, `>`, `>=`, `==`, `!=` (single `=` inside expressions rejects); functional `eq`/`equals` (`308`), `geq` (`309`), `gt` (`310`), `leq` (`311`), `lt` (`312`) accept tested 2–3 args (reject 0–1); `neq` (`313`) accepts 2 args.
   - **Logical operators & calls:** Infix `&&`, `||`, word `and`/`or`; prefix `!` and word `not`; functional `and` (`304`), `or` (`306`), `xor` (`307`) accept 0–3+ args (`and() == 1`, `or() == 0`, `xor() == 0`); `not(x)` (`305`) takes 1 arg; `implies(a, b)` (`325`) takes 2 args. Single `&`/`|`, infix `xor`, and `And(...)` reject.
   - **`piecewise` (`295`):** Accepts $\ge 1$ args as ordered `(value, condition)` `<piece>` pairs plus optional trailing `<otherwise>` (1 arg = `<otherwise>` only; even arity omits `<otherwise>`; 0 args rejects). Emits container `<piecewise>`, never `<apply>`.
3. **Precedence & Normalization Quirks:**
   - Prefix `!` / `not` binds tighter than comparisons (`!1 < 2` $\to$ `(!1) < 2`).
   - All relational operators (`<`, `<=`, `>`, `>=`, `==`, `!=`) share one precedence level below `+`/`-` and above `&&`/`||`.
   - `&&` and `||` have **equal precedence** and associate **left** (`true || false && false` $\to$ `(true || false) && false` $= 0$).
   - Infix `(true && false) && true` flattens to 3-child `and`, whereas `and(and(true, false), true)` and right-nested `true && (false && true)` stay nested.
   - Comparison chains (`1 < 2 < 3`, `(1 < 2) < 3`, `1 < 2 <= 3`, `1 != 2 != 3`) rewrite into n-ary `lt` or `and(...)` with duplicated/shared middle operands; reviewer also notes comparisons with a functional left operand can chain upstream, so **source call vs. infix provenance alone does not isolate comparison chaining**.

---

## 2. Proposed Native Architecture (Subject to Blocker & Pending Decisions)

### 2.1 Syntax Provenance, IR, and Contextual Finite Coercion
- Keep [`Builtin`](../src/builtins.rs) strictly for real-valued numeric functions. Add explicit variants to `ExprKind` ([`src/syntax/mod.rs`](../src/syntax/mod.rs)) and `ExpressionKind` ([`src/model.rs`](../src/model.rs)):
  - `Boolean(bool)`, `Not { child }`, `Compare { op: CompareOp, left, right }`, `Logical { op: LogicalOp, args }`, and `Piecewise { branches: Vec<PiecewiseBranch<Id>>, otherwise: Option<Id> }` (where `otherwise` alone has `branches.is_empty()` and `otherwise: Some(id)`).
- Retain source syntax provenance (infix vs. functional call, parenthesization) on syntax nodes until [`src/semantic/expression_normalization.rs`](../src/semantic/expression_normalization.rs) so `&&`/`||` left-flattening vs. functional `and(...)`/`or(...)` nesting matches upstream while token-based [`format_antimony`](../src/format/antimony.rs) stays byte-idempotent.
- Maintain typed `EvalValue::Real(Option<f64>)` and `EvalValue::Bool(Option<bool>)` internally in [`src/semantic/initialization.rs`](../src/semantic/initialization.rs) with explicit finite L3V2 conversions (`true <-> 1.0`, `false <-> 0.0`, nonzero finite real `-> true`, `0.0 -> false`). `Parameter::value()` and `Species::initial_concentration()` remain `Option<f64>` (a top-level Boolean initializer yields `Some(1.0)` or `Some(0.0)` while exporting its Boolean MathML `InitialAssignment`).

### 2.2 Separation of Static Dependency/Unit Validation vs. Lazy Conditional Evaluation
1. **Static Full-Tree Traversal (All Branches Inspected):**
   - **Symbol inference ([`src/semantic/symbols.rs`](../src/semantic/symbols.rs)) & reaction modifiers ([`src/semantic.rs`](../src/semantic.rs)):** Walk all `Piecewise` branches/conditions, `Compare`, `Logical`, and `Not` children in source order. Unknown symbols in inactive branches remain inferred parameters; species in conditions/branches become reaction modifiers.
   - **Cycle & `constant` propagation ([`src/semantic/initialization.rs::resolve`](../src/semantic/initialization.rs#L16-L109)):** Extract syntactic dependencies across **all** branches (active and inactive). Cycles hidden behind `false` (`a = piecewise(a, false, 1)`) reject with `DiagnosticCode::InitializationCycle`.
   - **Conservative unit checking ([`src/semantic/units.rs::check_expressions`](../src/semantic/units.rs#L366-L432)):**
     - `Compare` operands and `Piecewise` value branches (including inactive ones) reject with `DiagnosticCode::UnitsInconsistency` when two operands/branches have provably incompatible `Known(a) != Known(b)` dimensions.
     - Conditions with non-zero `Known(d)` (`d != ZERO`) reject under the documented stricter native unit check; `Unspecified` bare numbers and unannotated symbols are **never** assumed dimensionless.
     - Boolean AST nodes yield `Known(ZERO)` (dimensionless under L3V2 conversion) in `ValidatedModel::expression_dimensions`.
2. **Demand-Driven Lazy Finite Initialization ([`src/semantic/initialization.rs::evaluate`](../src/semantic/initialization.rs#L111-L203)):**
   - `Piecewise` evaluates conditions sequentially (`c_0, c_1, ...`) via an explicit stack state machine:
     - On the first condition that evaluates to `Some(true)`, evaluate **only** its corresponding `value_i` and skip all later conditions, inactive branches, and `otherwise` (so inactive `1/0`, `ln(0)`, `sqrt(-1)` do not trigger false domain errors).
     - On `Some(false)`, skip `value_i` without evaluating it and advance to the next branch (or `otherwise`).
     - On `None` (e.g. depends on `time` or an unset parameter), stop immediately and return symbolic `None` without evaluating subsequent branches or folding equal unknown branches.
     - If all conditions evaluate to `Some(false)` and `otherwise` is `None`, reject with `DiagnosticCode::DeclarationConflict` in static initializers (nonfinite `NaN` in reference runtime); rate laws preserve omitted `otherwise` structurally.
   - Active evaluated nonfinite values (`1/0`, `sqrt(-1)`, `ln(0)`, overflow) reject with `DiagnosticCode::DeclarationConflict`; never emulate RoadRunner's nonfinite `NaN -> false` / `Inf -> true` truthiness.

### 2.3 Iterative Stack Safety and Resource Budgets
- Every pass remains 100% iterative (`Vec` work stacks; zero Rust recursion on expression trees) and enforces `LimitKind::ExpressionDepth` plus hard ceiling `256`.
- In `expression_normalization.rs`, post-order lowering pushes children in reverse source order so `lowered[child] < lowered[parent]` holds across `Piecewise`, `Compare`, `Logical`, and `Not`.
- For the initial no-chain scope, `arena.len() <= source.expressions.len()` remains intact. If comparison chains are ever admitted later, middle-operand duplication requires an explicit normalized-node budget separate from source node count.

---

## 3. Complete Walker & Module Integration Inventory

| Module / Item | Phase 3B Integration Requirement |
|---|---|
| [`src/syntax/lexer.rs`](../src/syntax/lexer.rs) | Lex `==`, `!=`, `<`, `<=`, `>`, `>=`, `&&`, `||`, `!`; keep `=`, `->`, `=>` intact; keep single `&`/`|`, `:=`, `<=>`, `<==>` as `Kind::Unsupported`. |
| [`src/syntax/parser.rs::scalar`](../src/syntax/parser.rs#L413-L420) | Extend compartment trailing-operator guard so compartment conditional expressions still report `UnsupportedFeature`. |
| [`src/syntax/expression.rs`](../src/syntax/expression.rs) | Pratt binding powers (`!` above `<`/`<=`/`>`/`>=`/`==`/`!=` above equal-precedence left-assoc `&&`/`||`), Boolean constants, `piecewise(...)`, allowed call forms, and explicit unsupported diagnostics for excluded forms. |
| [`src/format/antimony.rs`](../src/format/antimony.rs#L162-L190) | Tight prefix `!`; spaced binary comparisons and `&&`/`||`/word operators; byte-idempotent roundtrip. |
| [`src/semantic/expression_normalization.rs`](../src/semantic/expression_normalization.rs) | Two-pass iterative alias/post-order remapping for `Boolean`, `Not`, `Compare`, `Logical` (infix left-flattening vs. functional preservation), and `Piecewise`. |
| [`src/semantic/symbols.rs::infer_expression`](../src/semantic/symbols.rs#L246-L265) | Push `Not`, `Compare`, `Logical`, and `Piecewise` (`value`, `condition`, `otherwise`) children in reverse stack order to preserve source declaration order. |
| [`src/semantic.rs::analyze_document`](../src/semantic.rs#L197-L279) | Map `ExprKind` $\to$ `ExpressionKind`; push `Not`, `Compare`, `Logical`, and `Piecewise` children in reaction `modifiers` discovery. |
| [`src/semantic/initialization.rs`](../src/semantic/initialization.rs) | Full-tree syntactic dependency walk in `resolve`; lazy `Piecewise` stepping, short-circuit `&&`/`||`, and finite L3V2 coercion in `evaluate`. |
| [`src/semantic/units.rs::check_expressions`](../src/semantic/units.rs#L366-L432) | Post-order dimensional compatibility checks across all `Compare` operands, conditions, and `Piecewise` value branches. |
| [`src/model.rs`](../src/model.rs) & [`src/lib.rs`](../src/lib.rs) | Public `CompareOp`, `LogicalOp`, `PiecewiseBranch`, and `ExpressionKind` variants; update `IMPLEMENTED_PROFILE` only at signoff. |
| [`src/sbml.rs::math`](../src/sbml.rs#L254-L361) | Iterative stack tasks (`EndPiecewise`, `StartPiece`, `EndPiece`, `StartOtherwise`, `EndOtherwise`) plus `<true/>`, `<false/>`, and `<apply>` operators. |
| [`tools/oracle/oracle.py::MATH_TAGS`](../tools/oracle/oracle.py#L80) | Widen `MATH_TAGS` **only** for verified conditional tags alongside negative controls, preserving finite/non-null AST guards. |

---

## 4. Explicit Exclusions vs. Decisions Pending Before Implementation

### 4.1 Intentional Initial Exclusions (Report `DiagnosticCode::UnsupportedFeature`, Not Syntax Error)
1. **Infix comparison chains and grouped-left relations:** `a < b < c`, `a == b == c`, `(a < b) < c`, and chained comparisons with a functional left operand (`lt(a, b) < c` — since functional left operands can also chain upstream, parser/normalizer must explicitly reject relational chaining rather than assuming function provenance prevents it).
2. **`xor` and `implies`:** Excluded from the initial stimulus slice (`xor(...)`, infix `xor`, `implies(...)`).
3. **Functional `not(...)` and prefix word `not`:** Excluded/held pending resolution of the upstream rational-call lexical quirk (`not(1/0)` / `sin(1/2)` blocker).
4. **Broader out-of-slice features:** Assignment/rate rules, events, user-defined functions, `delay`, compartment expressions, and nonfinite constants (`inf`, `NaN`).

### 4.2 Decisions Pending Root Signoff
1. **Unary/function call rational-notation blocker (`sin(1/2)`, `not(1/0)`):** Must be investigated and resolved across numeric and conditional calls before unpausing Phase 3B.
2. **Direct n-ary relational function calls (`lt(a, b, c)`, `gt`, `leq`, `geq`, `eq`, `equals` with $\ge 3$ args):** Under decision whether to exclude in first 3B alongside infix comparison chains or admit with explicit n-ary normalization.
3. **AND/OR demand-driven evaluation:** Confirm left-to-right short-circuiting (`false && invalid`, `true || invalid`) vs. strict operand evaluation when all static symbol/unit checks still inspect all operands.
4. **Missing-`otherwise` and numeric condition unit strictness:** Confirm `DeclarationConflict` on statically exhausted no-`otherwise` initializers and whether dimensional conditions (`piecewise(1, x_mole, 0)`) reject as `UnitsInconsistency`.

---

## 5. Original Proof & Rollout Gates

1. **Gate 0 — Phase 3A Freeze & Blocker Resolution:** Complete Phase 3A signoff and resolve the `sin(1/2)` / rational-call grammar boundary (future `docs/RATIONAL_CALLS.md`) before editing `src/` for Phase 3B.
2. **Gate 1 — Scope & Decision Lock:** Root locks pending decisions in Section 4.2.
3. **Gate 2 — Native Implementation & 4-Matrix Test Gate:** Pass positive/negative tests (precedence, L3V2 finite conversions, lazy active/inactive domain pairs, inactive cycles, unit mismatches, omitted `otherwise`, depth limits, excluded chains/operators) across `--all-features`, default, `--no-default-features`, and `--no-default-features --features cli`.
4. **Gate 3 — Oracle AST & Independent SBML Gate:** Exact AST projection match (including child order and `<otherwise>` presence), `doc.checkConsistency()` zero errors, byte-idempotent formatting, and zero regression on existing corpora.
