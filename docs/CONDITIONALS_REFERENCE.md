# Phase 3B conditional-expression reference discovery

**Discovery only, 2026-10-14. No native conditional support is claimed.**
Phase 3A passed its prior independent gates, but supplemental rational-token
discovery has reopened compatibility review; see [RATIONAL_CALLS.md](RATIONAL_CALLS.md). This note proposes
an implementation boundary for a later decision; it does not change profiles,
native source, test expectations, or existing evidence.

## Evidence and reproducibility

Original inputs, scripts, complete exported XML, loader errors, independent
libSBML messages, raw AST projections, and runtime observations are in
[`artifacts/conditionals-phase3b/discovery`](../artifacts/conditionals-phase3b/discovery).

- [`reference.json`](../artifacts/conditionals-phase3b/discovery/reference.json):
  **174** isolated Antimony 3.2.0 / independent libSBML 5.21.2 processes,
  **134 loader accepts, 40 loader rejects**, no process failure/timeouts.
- [`followup.json`](../artifacts/conditionals-phase3b/discovery/followup.json):
  **10** additional isolated probes, **9 accepts, 1 reject**.
- Across the **143 accepted exports**, no libSBML Error/Fatal message or null
  expression MathML was observed. Warnings remain in the records; this is not a
  claim that every accepted initializer is finite or executable.
- [`runtime.json`](../artifacts/conditionals-phase3b/discovery/runtime.json):
  **103** isolated RoadRunner 2.10.0 processes, all process exits zero;
  **98 model loads accepted, 5 caught missing-parameter exceptions**. Four accepted
  models produced a recorded nonfinite parameter result, not a passing finite
  initialization. RoadRunner bundles libSBML 5.21.1; validation above separately
  uses 5.21.2. No native/reference trajectory-equivalence gate was run.
- One discovery-worker defect initially lost the result for an infinite AST leaf:
  the existing finite-only `oracle.ast` helper raised. The prior 166-case record is
  retained as `reference-before-worker-fix.json`. The worker now records projection
  exceptions separately while preserving XML, loader status, and validation.
  No oracle code or accepted vocabulary was widened.

Run from the repository root, using existing environments only:

```sh
python3 artifacts/conditionals-phase3b/discovery/probe.py
python3 artifacts/conditionals-phase3b/discovery/followup_probe.py
python3 artifacts/conditionals-phase3b/discovery/runtime_probe.py
```

Scripts use absolute pinned interpreter paths internally and one subprocess per
input. Inputs are trusted original strings without imports; isolation is not an
OS sandbox. Runtime rows are observations, not native validation fixtures.
`provenance.json` records versions/source references; `hashes.json` fingerprints
retained discovery files. No package installation, binary download, or source
checkout was performed.

## Normative sources versus observed behavior

The [pinned Antimony tutorial](https://github.com/sys-bio/antimony/blob/f3f5c6b7f3f22ab48f8e382a9318ef50bce2b4b2/doc/AntimonyTutorial.md)
explains ordered piecewise alternatives and the final fallback. The
[pinned registry](https://github.com/sys-bio/antimony/blob/a4cb7549a03beef569ce21a0a7342e315ca0ed43/src/registry.cpp)
lists relational/logical function spellings and the six Boolean constant aliases.
These source pins are documentation baselines, not proof of wheel/source identity.

[SBML L3V2 Core Release 2, sections 3.4.4, 3.4.11–12](https://sbml.org/specifications/sbml-level-3/version-2/core/release-2/sbml-level-3-version-2-release-2-core.pdf)
explicitly permits Boolean/numeric context conversion: true/false become
1/0, zero becomes false, and nonzero numeric values become true. Converted values
are dimensionless. Numeric conditions should be dimensionless, and piecewise
alternatives should have compatible units. These unit recommendations are not
mandatory validity errors; a compiler may implement a stricter documented check.
The specification distinguishes Boolean-producing operators from numeric ones
without forbidding contextual conversion. A strict Boolean-only argument rule
would therefore be an intentional native restriction, not the upstream language.

The installed libSBML parser documentation confirms equal precedence for AND/OR;
its factual inspection record is `libsbml-parser-api.txt`. Do not copy precedence
from a familiar programming language. Its power-associativity prose is not used
as an Antimony oracle: actual pinned arithmetic probes take precedence.

## Verified spelling and arity inventory

All observations below are represented by named cases in `reference.json`.

| Construct | Observed contract |
|---|---|
| Relational operators | `<`, `<=`, `>`, `>=`, `==`, `!=`. A single `=` inside the formula rejects. |
| Functional relations | `eq` and `equals` alias equality; `gt`, `geq`, `lt`, `leq` accept tested 2 and 3 arguments, reject 0 and 1. `neq` accepts exactly 2 among tested arities 0–3. |
| Logical operators | `&&`, `||`, unary `!`; single `&`/`|` reject. Word `and`/`or` infix and word `not` prefix also load. Infix word `xor` rejects. |
| Logical functions | `and`, `or`, `xor` accept tested 0–3 arguments. `not` requires 1; `implies` requires 2. Uppercase `And(...)` rejects. |
| Boolean constants | `true`, `True`, `TRUE`; `false`, `False`, `FALSE`. Mixed-case `TrUe` is an ordinary inferred symbol, not an alias. |
| Piecewise | At least 1 argument. Ordered `(value, condition)` pairs plus optional final otherwise. One argument is otherwise-only. Even argument count omits otherwise. Zero arguments reject. |

The arity ranges above distinguish tested points from an unbounded guarantee.
Resource-bounded generated arity testing remains an implementation-review task.

## Precedence and normalization: implementation-critical

All relational operators occupy one level below addition and above logical AND/OR.
AND and OR have **equal precedence and associate left**:

- `true || false && false` exports `(true || false) && false`, giving 0.
- `true || (false && false)` retains the group, giving 1.
- `!1 < 2` exports `(!1) < 2`; unary NOT binds before comparison.
- `1 + 2*3 < 2^3` keeps arithmetic inside the comparison.

Comparison chains are not ordinary left-fold Boolean-to-number comparisons:

- `1 < 2 < 3` becomes one three-child `lt` AST node.
- `(1 < 2) < 3` also becomes that chain.
- `1 < 2 <= 3` becomes `and(lt(1,2),leq(2,3))`.
- `1 != 2 != 3` becomes `and(neq(1,2),neq(2,3))`, not a three-child `neq`.
- `1 < (2 < 3)` stays nested and gives 0 after numeric coercion of the inner result.

**Source operator/call provenance must survive until normalization.** Followups
show `lt(lt(1,2),3)` retains its nested left child, unlike `(1<2)<3`.
Similarly, infix `(true&&false)&&true` flattens to three children, but
`and(and(true,false),true)` preserves a nested functional call. Right-nested
logical groups remain nested. Do not reuse arithmetic's right-group flattening
rule or equate functional and infix construction prematurely.

Mixed comparison chains may share or duplicate middle operands in normalized
output. This requires explicit arena/resource accounting; the Phase 3A invariant
that normalized node count never exceeds source node count cannot simply be
assumed for that extension.

## Types, finite domains, and conditional evaluation

Pinned runtime observations confirm finite conversions:

| Source expression | Observed initial `a` |
|---|---:|
| `true+2` | 3 |
| `sin(true)` | 0.8414709848078965 |
| `-true`, `true^2` | -1, 1 |
| `piecewise(7,0,9)` | 9 |
| `piecewise(7,2,9)`, condition -2 or 0.5 | 7 |
| `not(0)`, `not(-2)`, `not(0.5)` | 1, 0, 0 |
| `and()`, `or()`, `xor()` | 1, 0, 0 |
| `piecewise(true,true,9)` | 1 |

The XML retains Boolean elements rather than replacing them with numeric leaves.
`returnsBoolean(model)` reports false for the mixed-valued piecewise case; it is
not a validity/coercion policy and must not be used to reject all numeric conditions.

### Inactive branches versus domain errors

- `piecewise(7,true,sqrt(-1))`, its false-first counterpart, and inactive `1/0` or
  `ln(0)` fallbacks yield 7.
- `piecewise(sqrt(-1),true,7)` yields NaN; active `1/0` yields infinity.
- `piecewise(7,false)` and two false alternatives without otherwise yield NaN.
- A first true alternative masks an invalid later condition in the observed result.
- `and(false,sqrt(-1))` yields 0; `or(true,sqrt(-1))` yields 1. The reverse argument
  orders also produce finite results in RoadRunner.

These pure-expression results establish observable outcomes, **not proof that a
machine instruction never evaluated an inactive operand**. A native finite-real
initializer needs lazy selected-branch evaluation so that its own domain checks
do not reject the valid inactive-branch examples. Logical short-circuit policy
must be documented separately from piecewise selection.

Nonfinite edge observations stay outside the proposed finite initializer domain:
RoadRunner treats derived `sqrt(-1)` in a condition as false (`piecewise` gives 9;
`not` gives 1), and derived/literal infinity as true. Literal `NaN` in these
Antimony expressions fails loading as an unresolved identifier. Neither fact
licenses a guessed native nonfinite truth rule. `not(1/0)` and `not(1 / 0)` reject
as rational-notation syntax, whereas `!(1 / 0)` loads and exports an infinity leaf;
this alias/lexical edge must remain an explicit unsupported boundary or receive
additional work, not be silently declared equivalent.

## Unknowns, cycles, time, and units

- Unknown symbols in either branch or conditions remain actual parameters in the
  export. No condition/branch pruning occurs in the reference XML.
- `a=piecewise(a,false,1)` and a two-symbol cycle hidden behind false both reject
  in Antimony. Dependency validation must inspect **all** branches even when
  numeric evaluation is lazy.
- `a=piecewise(u,false,1)` loads structurally, but RoadRunner rejects missing `u`.
  So does an unknown condition with identical numeric alternatives. Solver
  readiness cannot be inferred from syntactically inactive unknown references.
- `piecewise(7,time>0,9)` remains a time-containing InitialAssignment; RoadRunner's
  initial-state observation is 9. This does not change Phase 3A's conservative
  native `time -> None` policy or authorize treating all time expressions as zero.
- Mismatched branch units and mismatched comparison units load with retained
  libSBML unit warnings. The existing stricter native provable-mismatch policy can
  extend to every branch/operand, including inactive branches, but must remain
  distinct from the lazy numeric-domain policy. Unknown units are not dimensionless.

## MathML and comparator implications

Observed AST identities: false 264, true 266, piecewise 295; and/not/or/xor
304/305/306/307; eq/geq/gt/leq/lt/neq 308/309/310/311/312/313; implies 325.
Numeric type IDs alone do not describe branches: retain ordered children, node
arity, and missing-otherwise distinction.

Piecewise serializes as `<piecewise>` containing ordered `<piece>` pairs and an
optional `<otherwise>`, **not** an ordinary `<apply>` call. The AST alternates
value/condition children with an optional trailing fallback. Constants use
`<true/>`/`<false/>`; operators appear under `<apply>`.

The existing exact AST projector retains these IDs and child order, but its closed
XML vocabulary intentionally blocks most conditional tags. A future gate may add
only proven conditional tags alongside negative controls. It must compare omitted
otherwise differently from explicit zero, preserve function/infix normalization,
and reject null math instead of equating it with absence. No null expression was
seen in this discovery; earlier unrelated upstream null cases remain relevant
negative controls. Infinity still raises the finite projection guard; that guard
must not be removed merely to make conditional discovery appear to pass.

## Proposed minimum coherent native contract — decision pending

**Post-rational-repair recommendation; no native conditional implementation is
claimed or authorized by this note.** The shared call-opening rational recognizer
now makes functional `not(...)` coherent with the numeric builtin grammar.
Root proposes deferring **all textual infix/prefix aliases** (`a and b`, `a or b`,
`not x`) for the first conditional increment, rather than accepting a fragile
subset determined by reconstructed whitespace. Symbolic forms and functional
calls retain general conditional expressiveness.

### Supported syntax proposed

- Binary relational operators `< <= > >= == !=`.
- Direct resource-bounded `eq`/`equals`/`gt`/`geq`/`lt`/`leq` calls with at least
  two arguments; `neq` exactly two. These calls retain nested arguments.
- Symbolic `!`, `&&`, `||`; functional `not` exactly one argument and `and`/`or`
  with zero or more arguments. Empty AND/OR retain their observed true/false
  identities. Double NOT is **not** simplified away in exported expressions.
- Six Boolean constant spellings and arbitrary bounded piecewise value/condition
  pairs with optional otherwise, including otherwise-only.
- Existing numeric functions/constants/time/rational atoms and arithmetic remain
  available inside every supported context, subject to existing static boundaries.

`not (expr)` is still a parenthesized function call: source whitespace does not
make it the deferred bare-word form. The shared rational guard rejects
`not(1/2)` and `not (1/2)` as upstream syntax errors; `not((1/2))` accepts an exact
rational argument and `not((1)/2)` accepts a divide argument.

### Exact normalization and exclusions

AND/OR remain equal-precedence, left-associative. An **outer infix operator**
appends to a same-kind left logical node even when that node came from a function:
`and(true,false)&&true` has three children. It does not flatten the right child:
`true&&and(false,true)` retains nested AND. An **outer functional call** never
performs that operator append: `and(and(true,false),true)` stays nested. Empty
left AND also follows the rule: `and()&&true` becomes one-child AND, not `true`.
Preserve the outer-construction provenance until normalization, not a blanket
rule that functional children can never flatten.

Defer infix comparison-chain rewriting rather than left-fold it incorrectly.
Check the **normalized left operand** of each infix comparison and reject as
UnsupportedFeature if it is either:

1. Any comparison node, including one constructed by a functional call; or
2. AND with at least two direct children, all of which are comparison nodes.

The second case is important: `and(lt(1,2),gt(3,2))<4` becomes an AND containing
three relations upstream, not a Boolean-to-number comparison. A single-child
AND, an AND with a Boolean child, an OR, or a right-nested relation does not meet
that chain rule. Unary plus and double numeric negation can disappear before this
check (`+(1<2)<3` and `-(-(1<2))<3` still form chains). Never reject every Boolean
left operand; ordinary Boolean/numeric context conversion remains valid.

This predicate follows the pinned
[libSBML `combineRelationalElements` implementation](https://github.com/sbmlteam/libsbml/blob/v5.21.1/src/sbml/math/L3Parser.ypp),
and targeted pinned execution confirmed its relation/AND cases. Keeping chain
expansion deferred avoids duplicated middle operands and a new expansion budget.
XOR/implies, textual logical aliases, rules/events/user functions/delay and
nonfinite constants remain explicit exclusions.

### Values, units, and demand-driven evaluation

Use typed internal numeric/Boolean values with an explicit unknown state and
finite context conversions. Public model parameter values remain numeric:
Boolean roots become1/0 there without changing Boolean MathML identity.
Known finite numeric zero is false; other finite numbers are true. Unknown is
never zero or false. Numeric arithmetic/builtins/comparisons accept Boolean
values through the verified1/0 conversion.

References, inferred symbols, cycles, namespaces, branch dimensions and Boolean
context units are inspected across **all** branches. Known branch/operand unit
mismatches still reject even with another unknown-unit argument; unspecified is
not dimensionless. Boolean results are dimensionless. Existing lexical/finite
literal restrictions, including zero-denominator rational rejection, remain
static restrictions even in potentially inactive branches.

Initializer numeric evaluation is a separate bounded demand traversal:

- Piecewise evaluates conditions in order and only the selected value. The first
  unknown condition returns symbolic unknown, without evaluating possibly inactive
  alternatives or folding identical alternatives into a readiness claim.
- Known no-match without otherwise rejects under the finite-initialization
  boundary. Unknown selection remains unknown. Rate laws retain omitted otherwise
  structurally, with no finite-runtime guarantee.
- AND/OR use sequential finite short-circuit evaluation. Proposed conservative
  unknown policy: stop at the first unknown operand, rather than visiting later
  possibly inactive domain errors or deriving a value from later operands. Root
  should confirm this policy explicitly before implementation.
- N-ary comparisons can evaluate their arguments with ordinary eager numeric
  demand and return unknown if any required value is unknown, retaining known
  domain errors in other demanded arguments. No Boolean algebra shortcut is
  required. Comparisons check successive argument pairs, not arithmetic left-fold
  Boolean intermediates.

Sequential finite logical checking is intentionally narrower than RoadRunner's
observed nonfinite truth behavior; do not promise equivalence for demanded NaN/Inf.

### Proposed bounded API and implementation order

Public API proposal, not an implemented signature:

```text
ComparisonOp = Equal | NotEqual | Less | LessEqual | Greater | GreaterEqual
LogicalOp = And | Or
PiecewiseBranch { value: ExpressionId, condition: ExpressionId }
ExpressionKind::Boolean(bool)
ExpressionKind::Not { child: ExpressionId }
ExpressionKind::Compare { op: ComparisonOp, args: Vec<ExpressionId> }
ExpressionKind::Logical { op: LogicalOp, args: Vec<ExpressionId> }
ExpressionKind::Piecewise {
    branches: Vec<PiecewiseBranch>, otherwise: Option<ExpressionId>
}
```

1. Extend the explicit parser continuation stack and typed syntax arena. Keep
   generic call parsing/rational recognition shared; retain an internal outer
   operator-versus-function marker for logical/comparison construction.
2. Extend normalization with the exact append rules and chain rejection above.
   Preserve arithmetic/rational behavior and check source plus normalized
   depth/node/arity limits. Owned ID vectors must not duplicate subtrees.
3. Extend every static walker: reference inference, dependencies/cycles, modifiers,
   unit analysis, lowering/public IR. Introduce shared typed context conversions;
   do not bolt Boolean strings onto the numeric Builtin registry.
4. Replace eager-only initial evaluation with an explicit demand task stack for
   condition/branch and logical continuations. Keep arithmetic, numeric builtin
   domain checks and rational values on the same evaluator, not a second engine.
5. Emit exact ordered comparison/logical MathML and proper piecewise/piece/otherwise
   structure, including missing otherwise. Update source-token formatter spacing
   without changing operators into textual aliases or losing call/group identity.
6. Independently test nested mixed-type cases, unknown/domain interactions,
   all-branch unit/cycle checks, symbol provenance, source/normalized limits,
   formatter idempotence, exact ASTs and historical arithmetic/rational replay.

### Additional read-only probe findings and remaining evidence work

After the rational repair,76 targeted isolated pinned-loader executions were run
without modifying native source or artifact files. Their detailed tool outputs are
exploratory evidence, **not a frozen compatibility corpus**; the evidence owner
must capture selected cases before feature implementation claims. No native
conditional tests or simulation-equivalence runs were performed in this pass.

Observed textual alias traps include `not -1`, `not +1`, `not !false`,
`(true) and false`, `true and (false)`, `sin(0) and true`, and `a_ and b` rejecting,
while `not(-1)`, `not _b`, `true and not false`, and `a and _b` accept.
Uppercase infix AND/OR can load, whereas mixed-case And/Or and uppercase NOT do
not behave as equivalent aliases. Comments do not generally restore the missing
formula-space boundary. These results justify the simple initial exclusion.

The pinned [formula reconstruction](https://github.com/sys-bio/antimony/blob/f3f5c6b7f3f22ab48f8e382a9318ef50bce2b4b2/src/formula.cpp)
inserts spaces according to adjacent component characters and variable handling;
[`sbmlx.cpp`](https://github.com/sys-bio/antimony/blob/f3f5c6b7f3f22ab48f8e382a9318ef50bce2b4b2/src/sbmlx.cpp)
then replaces specifically space-delimited textual operators. The native first
increment need not reproduce that broader alias machinery.

Required evidence additions before final scope freeze: durable exact-AST cases
for left functional logical append and all chain-predicate branches; n-ary
comparison coercion/runtime controls; unknown AND/OR policy examples; deep nested
piecewise/logical source and normalized-size stress; missing-otherwise rates;
Boolean-valued piecewise inside numeric functions and dimensional branches.
The earlier184 conditional reference executions and103 runtime processes remain
useful evidence, but do not alone certify this revised scope.

## Suggested ownership and execution order

After Phase 3A signoff and scope approval:

1. **Native owner:** `src/` and a new native conditional test file. Implement syntax
   provenance/types first; then normalization and conservative dimensions;
   demand-driven initialization; public IR; MathML; all existing walkers;
   formatter/limits and deliberate historical expectation migrations last.
2. **Evidence owner:** separate new conditional sources/manifest/reference snapshots,
   explicit current-profile overlay, narrowly expanded MathML gate vocabulary,
   API-compatible checker tests, and a new profile identity. Never rewrite Phase
   3A frozen artifacts or permit nonfinite/null projections as blanket passes.
3. **Independent reviewer:** independently authored compositions, precedence and
   branch/domain/type cases, source-limit and normalized-size stress, all-feature
   and core-only matrices, immutable binaries, replay of earlier arithmetic and
   numeric builtin corpora. Reviewer does not implement native behavior.
4. **Agy:** its separately owned design note, then one explicitly assigned new
   authoring-test file if requested. No simultaneous edits to native or evidence.

Before implementation, root must decide exclusions, missing-otherwise handling,
AND/OR demand semantics, and the exact treatment of function-call lexical quirks.
No part of this note authorizes bypassing the Phase 3A gate.

## Key Learnings

1. AND/OR precedence and source-call versus operator normalization differ from
   common programming-language assumptions and must be reference-tested.
2. L3V2 numeric/Boolean conversion is specified behavior, not a permissive loader bug.
3. Static dependency/unit inspection and lazy initial-value evaluation require
   different traversals; inactive cycles still reject, inactive domain errors need not.
