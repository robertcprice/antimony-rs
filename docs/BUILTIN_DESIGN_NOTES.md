# Phase 3 Increment 1 Design Notes: Numeric Builtins, `time`, and Constants

> **Archived proposal; superseded by the [verified builtin guide](BUILTINS.md).**
> This note predates later pinned probes and implementation review. In particular,
> zero-argument `time()` is supported, the expression parser is iterative rather
> than recursive, and time dimensions follow the verified model-default unit
> contract. Do not use the older suggestions below as current implementation
> requirements or evidence of support.


Status: **Design proposal** for Phase 3 Increment 1 (`docs/FLEXIBLE_MODELING_PLAN.md:144-167`). All Rust types and signatures below are proposals until implemented and verified. Relational/boolean operators and `piecewise` follow as Increment 2; rules (Phase 4) and user-defined functions (Phase 5) remain out of scope.

## 1. Verified Reference Baseline vs Unverified Candidates

Live probes in `artifacts/flexibility-audit/expressions/probes.json` (Antimony `3.2.0`, libSBML `5.21.2`) establish only the following expression behaviors:

| Source spelling | Probe ID | Reference libSBML AST | Reference MathML emission | Verified note |
| :--- | :--- | :--- | :--- | :--- |
| `sin(x)` | `rate_09` | `type: 300` (`AST_FUNCTION_SIN`), 1 child | `<apply><sin/>...</apply>` | Accepted; already in `oracle.py::MATH_TAGS` |
| `exp(x)` | `rate_10` | `type: 290` (`AST_FUNCTION_EXP`), 1 child | `<apply><exp/>...</apply>` | Accepted; blocked in oracle by `<exp/>` |
| `sqrt(x)` | `rate_11` | `type: 297` (`AST_FUNCTION_ROOT`), children `[int(2), x]` | `<apply><root/><degree><cn type="integer">2</cn></degree>...</apply>` | Reference emits explicit `<degree>` |
| `log(x)`, `ln(x)` | `rate_12`, `rate_14` | `type: 293` (`AST_FUNCTION_LN`), 1 child | `<apply><ln/>...</apply>` | **Both map to natural log** (`<ln/>`) |
| `log10(x)` | `rate_13` | `type: 294` (`AST_FUNCTION_LOG`), children `[int(10), x]` | `<apply><log/><logbase><cn type="integer">10</cn></logbase>...</apply>` | Reference emits explicit `<logbase>` |
| `pow(x, y)` | `rate_15` | `type: 296` (`AST_FUNCTION_POWER`), 2 children | `<apply><power/>...</apply>` | Same MathML `<power/>` tag as `x ^ y` |
| `time` | `rate_10`, `rate_17`, `initializer_time` | `type: 262` (`AST_NAME_TIME`), `name: "time"` | `<csymbol encoding="text" definitionURL="http://www.sbml.org/sbml/symbols/time">time</csymbol>` | Accepted in rates and `a=time+1` |
| `pi` | `rate_18` | `type: 265` (`AST_CONSTANT_PI`), 0 children | `<pi/>` | Accepted; blocked in oracle by `<pi/>` |

### Reserved Identifier Catalogue (`src/syntax/reserved.rs:6-115`) — CANDIDATES NOT VERIFIED
Do **not** assert unprobed spellings, alias equivalences, or arities as upstream facts without live oracle probes:
- **Unprobed numeric function candidates (require individual probes before enabling)**:
  - *Unary/algebraic/rounding*: `abs`, `ceil`, `ceiling`, `factorial`, `floor`, `sqr`.
  - *Binary/n-ary arithmetic/root*: `power`, `root`, `divide`, `minus`, `plus`, `times`, `quotient`, `max`, `min`, `rem`.
  - *Trig/hyperbolic & inverse*: `cos`, `tan`, `sec`, `csc`, `cot`, `sinh`, `cosh`, `tanh`, `sech`, `csch`, `coth`, `acos`, `arccos`, `acosh`, `arccosh`, `acot`, `arccot`, `acoth`, `arccoth`, `acsc`, `arccsc`, `acsch`, `arccsch`, `asec`, `arcsec`, `asech`, `arcsech`, `asin`, `arcsin`, `arcsinh`, `atan`, `arctan`, `atanh`, `arctanh` (note: `asinh` is not in `reserved.rs`).
- **Unprobed or deferred constants/symbols**:
  - `exponentiale`: candidate constant (`<exponentiale/>`), unprobed.
  - `avogadro` (`rate_19`): reference emits `type: 261` (`AST_NAME_AVOGADRO`) csymbol; keep deferred until csymbol projection and dimensional/numerical policy are specified.
  - `inf`, `INF`, `infinity`, `NaN`, `nan`, `NAN`, `notanumber`: conflict with finite-real slice invariants (`src/semantic/initialization.rs:175`); keep rejected.
- **Deferred to Phase 3 Increment 2 (typed boolean/condition extension)**:
  - `piecewise` (`rate_16` probed with `time > 1`), `and`, `not`, `or`, `xor`, `eq`, `equals`, `geq`, `gt`, `leq`, `lt`, `neq`, `implies`, `true`, `True`, `TRUE`, `false`, `False`, `FALSE`.
- **Out-of-scope impure / temporal / package builtins (keep rejected as `UnsupportedFeature`)**:
  - `delay`, `rateOf`, `normal`, `truncatedNormal`, `uniform`, `exponential`, `truncatedExponential`, `gamma`, `truncatedGamma`, `poisson`, `truncatedPoisson`, `bernoulli`, `binomial`, `cauchy`, `chisquare`, `laplace`, `lognormal`, `rayleigh`.

## 2. Proposed Typed Representation and Shared Metadata Table

Avoid stringly-typed call nodes or merging builtins with Phase 5 user functions:
- **Proposed enums (`src/syntax/mod.rs`, re-exported in `src/model.rs`)**:
  - `BuiltinFunction`: initially verified variants (`Sin`, `Exp`, `Sqrt`, `Ln`, `Log10`, `Pow`), extended only as candidate probes land.
  - `BuiltinConstant`: `Pi` (plus `ExponentialE` once probed).
- **Proposed arena variants**:
  - `src/syntax/mod.rs::ExprKind` (`L101`): add `Time`, `Constant(BuiltinConstant)`, and `Call { func: BuiltinFunction, args: Vec<usize> }`.
  - `src/model.rs::ExpressionKind` (`L307`): add `Time`, `Constant(BuiltinConstant)`, and `Call { func: BuiltinFunction, args: Vec<ExpressionId> }`.
- **Single shared metadata table (`src/syntax/reserved.rs`)**:
  - Replace bare `builtin(&str) -> bool` with a static table of `BuiltinSpec { func, spellings, arity, mathml_shape, dim_rule }` plus `classify_ident(&str)` distinguishing: (a) supported `BuiltinFunction`, (b) supported `Time` / `BuiltinConstant`, and (c) recognized-but-unsupported `reserved::builtin` names (preserving `DiagnosticCode::UnsupportedFeature` vs `Syntax` / `UnresolvedSymbol`).
  - Both `"log"` and `"ln"` resolve at parse time to `BuiltinFunction::Ln` while source tokens remain untouched for `src/format/antimony.rs::format_antimony` (`L31`).

## 3. Exact Source Walkers Requiring Integration

Every walker must remain iterative (explicit worklist or postorder index loop) to preserve resource bounds:
1. **`src/syntax/parser.rs::Parser::expression` (`L481-L558`)**:
   - At `Kind::Ident(text)` (`L500`): if `text == "time"` or a supported `BuiltinConstant`, reject trailing `Kind::LParen` (not callable) and push `ExprKind::Time` / `ExprKind::Constant` with `depth: 1`.
   - If `text` maps to a supported `BuiltinFunction`: require `Kind::LParen` (reject bare function identifier without `(`), parse comma-separated argument expressions via `self.expression(0, recursion + 1)?`, enforce exact `spec.arity`, compute `depth = args.iter().map(|&i| self.expressions[i].depth).max().unwrap_or(0) + 1`, and push `ExprKind::Call`.
   - Keep `Parser::name` (`L75-L117`) rejecting all `reserved::builtin` and `"time"`/`"avogadro"` in declarations.
2. **`src/semantic/expression_normalization.rs::normalize` (`L24-L176`)**:
   - **Critical**: `ExprKind::Call` must **not** fall into `Shape::Atom(atom.clone())` (`L57`), which leaves child indices un-remapped and skips inner group normalization.
   - Add `Shape::Call(BuiltinFunction, Vec<usize>)`: remap `args` through `aliases` (`L28-L58`), push `args` in reverse onto `pending` during the postorder associative-group pass (`L89-L117`), and lower `args` through `lowered` in the arena pass (`L121-L170`) with `depth = lowered_args_max_depth + 1` checked by `push` (`L180-L201`). `Time` and `Constant` have no children and stay `Shape::Atom`.
3. **`src/semantic/symbols.rs::infer_expression` (`L246-L264`)**:
   - Push `ExprKind::Call { args, .. }` children onto `pending` (in reverse order so source discovery order is preserved) so undeclared symbols inside arguments infer `Symbol::Parameter`. Ignore `Time` and `Constant`.
4. **`src/semantic.rs::analyze_document` (`L31-L321`)**:
   - Linear scans over `&document.expressions` (`L35-L45` reaction-symbol rejection, `L104-L109` `first_seen`, and `src/semantic/unit_mapping.rs:257-261`) automatically visit `ExprKind::Symbol` nodes inside call arguments.
   - Lower `Time`, `Constant`, and `Call` into `model.expressions` (`L197-L230`).
   - Reaction modifier walk (`L252-L272`): push `ExpressionKind::Call { args, .. }` onto `pending` so species inside `sin(S)` or `pow(S, 2)` are added to `Reaction::modifiers`.
5. **`src/semantic/initialization.rs::{resolve, evaluate}` (`L16-L187`)**:
   - `resolve` (`L31-L45`): push `ExprKind::Call` `args` onto `pending`; `Time` and `Constant` add no symbol dependency edges.
   - `evaluate` (`L110-L187`): push `(id, true)` then `args` in reverse (`(arg, false)`) so `operands` pops evaluated `Option<f64>` arguments in forward order.
6. **`src/semantic/units.rs::check_expressions` (`L356-L416`)**:
   - Postorder arena loop (`dimensions[*arg]` already computed): enforce dimensional rules for `Time`, `Constant`, and `Call`.
7. **`src/sbml.rs::math` (`L254-L316`)**:
   - Emit `<csymbol encoding="text" definitionURL="http://www.sbml.org/sbml/symbols/time">time</csymbol>` for `Time`, `<pi/>` for `Constant(Pi)`, and `<apply>` for `Call` (emitting `<degree><cn type="integer">2</cn></degree>` before child for `Sqrt`, and `<logbase><cn type="integer">10</cn></logbase>` before child for `Log10`).

## 4. Invariants: Initialization, `time`, Dimensions, Purity, and Caps

- **Initial-state semantics for `time` and unset symbols (`probes.json::initializer_time`)**:
  - Upstream accepts `a = time + 1`, emits `<initialAssignment symbol="a">`, keeps `<parameter id="a" constant="true"/>`, and omits the scalar `value` attribute (libSBML reports `NaN`).
  - Therefore, `evaluate` must return `Ok(None)` for `ExprKind::Time` (never substituting `t = 0.0`), while `resolve` (`L69-L78`) keeps `constant: true` when all referenced symbols are constant.
  - **Domain vs unknown-propagation order**: In `evaluate`, concrete subexpressions are evaluated postorder before parent operations combine `Option<f64>`. Thus `time + (1 / 0)` or `unset_param + sqrt(-1)` still fails with `DiagnosticCode::DeclarationConflict` on the invalid subexpression, while `sqrt(unset_param)` or `ln(time)` cleanly propagates `Ok(None)` without guessing values.
- **Finite real domain checks (`src/semantic/initialization.rs:145-181`)**:
  - When all `Call` arguments evaluate to `Some(v)`: reject `Sqrt(x)` for `x < 0.0`, `Ln(x)`/`Log10(x)` for `x <= 0.0`, `Pow(x, y)` for `(x < 0.0 && y.fract() != 0.0) || (x == 0.0 && y <= 0.0)`, and any non-finite result (`!res.is_finite()`, e.g. `exp(1000.0)`) with `DiagnosticCode::DeclarationConflict`. Canonicalize `-0.0` to `0.0` (`L184`).
- **Dimensional rules (`src/semantic/units.rs:356-416`)**:
  - `Constant(Pi)`: `Known(ZERO)` (dimensionless; catches `pi + S` when `S` has known non-zero dimensions, while `pi + 1` stays `Indeterminate` because bare numbers are `Unspecified`).
  - `Time`: `Known(Dimensions { time: 1, ..ZERO })` unless overridden to `ZERO` by an explicit `unit time = dimensionless` model default (`src/semantic/unit_mapping.rs:331-354`).
  - Transcendental unary calls (`Sin`, `Exp`, `Ln`, `Log10`): if argument is `Known(d)` with `d != ZERO`, return `Err(DiagnosticCode::UnitsInconsistency)`; if `Known(ZERO)`, return `Known(ZERO)`; otherwise `Indeterminate`.
  - `Sqrt`: if `Known(d)`, require even exponents (`d.substance % 2 == 0 && d.time % 2 == 0 && d.length % 2 == 0`), returning `Known(d / 2)` or `Err(DiagnosticCode::UnitsInconsistency)`; otherwise `Indeterminate`.
  - `Pow`: match `BinaryOp::Power` (`L401-L409`)—reject `Known(b != ZERO)` exponent; `Known(ZERO)` if both `Known(ZERO)`; else `Indeterminate`.
- **Purity & resource caps**:
  - Only pure, stateless `f64 -> f64` (or `(f64, f64) -> f64`) math functions enter Increment 1. User functions (Phase 5) forbid global capture, `time`, forward calls, and recursion (`probes.json:1718-1908`).
  - Enforce `LimitKind::ExpressionDepth` and `HARD_DEPTH = 256` in both `Parser::expression` (`L462`) and `expression_normalization::push` (`L187`).

## 5. Oracle XML Vocabulary vs Semantic Projection (`tools/oracle/oracle.py`)

- `tools/oracle/oracle.py::MATH_TAGS` (`L78`) currently allows only `math apply ci cn sep plus minus times divide power sin gt csymbol`.
- Increment 1 implementation must widen `MATH_TAGS` with `exp root degree ln log logbase pi` **only** in the same commit that implements and tests those nodes with positive and negative controls (`docs/FLEXIBLE_MODELING_PLAN.md:163`).
- `oracle.py::ast` (`L49-65`) already projects `type`, `name` (for `time`, `type: 262`), `integer` (for `<degree>2</degree>` and `<logbase>10</logbase>`), and `children` without conflating distinct builtins.
