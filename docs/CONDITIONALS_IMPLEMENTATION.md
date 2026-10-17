# Conditional core implementation — Phase 3B

Status: independently verified bounded checkpoint (2026-10-14). The 17-command
native matrix and all 12 immutable-artifact gates passed; Python 441 tests passed.
See the [current evidence guide](CONDITIONALS.md) and
[independent final review](reviews/conditionals-review.md). The implemented identity is
`conditional-core-v0.3-slice`, not full Antimony compatibility. Reference authority
and discovery receipts are recorded in [CONDITIONALS_REFERENCE.md](CONDITIONALS_REFERENCE.md).
Historical math/rational evidence remains immutable.

## Contract and public representation

The owned expression arena adds `Boolean(bool)`, `Not { child }`,
`Compare { op, args }`, `Logical { op, args }` and
`Piecewise { branches, otherwise }`. `PiecewiseBranch` has public `value` and
`condition` expression IDs. Comparison operators are `Equal`, `NotEqual`, `Less`,
`LessEqual`, `Greater`, `GreaterEqual`; logical operators are `And`, `Or`.
Children are arena IDs, never source strings or recursively owned trees.

Accepted literal aliases are `true`, `True`, `TRUE`, `false`, `False`, `FALSE`.
Exactly `True()`, `TRUE()`, `False()`, `FALSE()` also produce Boolean leaves.
Lowercase Boolean calls and all nonzero-argument Boolean calls reject with Syntax.
The latter must not inherit upstream load-only acceptance with missing MathML.

Binary `< <= > >= == !=` and direct `eq/equals/lt/leq/gt/geq` functions (at least
two arguments), `neq` (exactly two), `!`, `not` (exactly one), symbolic `&& ||`
and functional `and/or` (zero or more) are supported. Empty AND is true and empty
OR false. AND/OR symbolic operators have equal precedence and associate left.
All textual infix/prefix aliases, XOR/implies, infix comparison-chain rewriting,
user functions, rules and events remain explicitly deferred.

`piecewise(value, condition, ..., otherwise)` retains ordered pairs and an optional
otherwise. One argument is an otherwise-only node; zero arguments reject. The
public tree is preserved even when the initializer getter has a known value.

## Demand and static validation are intentionally different

A shared iterative evaluator carries Numeric, Boolean or Unknown values. Finite
numeric zero is false; other finite numbers are true. Boolean numeric conversion
is exactly zero/one. Unknown values and time remain `None`, never guessed zero.

* Logical operands are demanded sequentially. A known decisive operand stops;
  the first unknown also stops immediately with Unknown. Thus `and(u,false)` and
  `or(u,true)` remain unknown, while `and(false,u)` and `or(true,u)` are known.
* Comparisons demand every argument, even after a false pair or unknown argument;
  known invalid arithmetic in a later sibling is not masked.
* Piecewise demands conditions in order, then only the first selected value.
  An unknown condition stops with Unknown. Known no-match without an otherwise
  rejects in finite initialization; reaction rates remain structurally valid.
* Derived invalid domains reject only when demanded. Literal envelope violations,
  rational zero denominators and unsupported syntax reject statically even in
  inactive branches. This is not a nonfinite runtime-emulation promise.

All branches still participate in symbol inference, dependency cycles, modifier
collection and dimensions. Known branch-value or comparison-operand dimensions
must agree even when another operand is unknown. Logical operands and conditions
cannot have known nonzero dimensions. Boolean results are dimensionless; bare
numeric/rational literals remain **Unspecified**, not known dimensionless. Unknown
dimensions are never promoted to known dimensionless. See the separately preserved
[units erratum](MATH_UNITS_ERRATUM.md).

## Exact normalization and lexical boundaries

An outer symbolic logical operator appends to a same-operator normalized left
node, including one originating as a functional call. Outer functional calls and
right groups remain nested. Before constructing an infix comparison, its normalized
left child is rejected if it is a comparison, or an AND with at least two direct
children all comparisons. Unary-plus and collapsed double-minus wrappers cannot
evade this guard; genuinely nested right relations and direct relational-function
nesting are not rewritten into chains.

The lexer coalesces consecutive token pairs `= =`, `& &`, `| |`, including spacing,
tabs and removed block comments. Actual newline tokens remain barriers. The
operators `<=`, `>=`, `!=` require contiguous characters. Original tokens are
charged before coalescing; formatting emits canonical operator text rather than
re-emitting a span containing removed comments. The rational call-open guard is
shared with numeric calls, so `not(1/2)` is Syntax while `not((1/2))` is valid.

## Resource and architecture audit

No dependency, unsafe block, reference runtime, evaluator of source strings,
filesystem access or global registry was introduced. Parsing, normalization,
evaluation, dependency walking, unit checking and MathML writing use explicit
iterative work stacks. Configured and hard expression-depth ceilings remain.

Every source expression node is admitted against the token ceiling; each function
argument is an already admitted child node. Source expressions form trees with no
subtree duplication, and branch pairs only reorganize the admitted argument IDs.
Logical source infix nodes hold two IDs each. Normalization **moves** an obsolete
same-operator left vector with `mem::take` before appending, instead of retaining
a growing copy at each infix node. Each source edge therefore occurs in only a
constant number of live arena representations; pending traversals are likewise
bounded by admitted nodes/edges. Temporary map/lowering copies are linear in these
bounds. Normalized output node count cannot exceed admitted source nodes; vector
length and normalized depth are not allowed to hide expansion.

The native stress regression constructs a 5,000-child functional AND followed by
64 infix appends, checks the single retained 5,064-child logical node, evaluates it,
and checks rejection under a reduced original-token ceiling. Independent tests
also cover deep input in subprocesses; their counts belong to the reviewer record.

## Walkers updated

* Source arena and call metadata: explicit typed variants and allocation-free
  `ExprKind::children()` / child remapping.
* Normalization: chain guard, left append, retained nesting, final depth/node caps.
* Initialization: static all-child dependency graph, separate demand continuations.
* Symbols, units and modifiers: every branch/condition, not just selected values.
* Public lowering: owned IDs, typed operators and branch records.
* Export: Boolean leaves, logical/relational apply nodes, and structural
  `piecewise/piece/otherwise` elements (not a generic function apply).
* Formatter: retained source tokens, canonical fused operators, idempotent reparse.

## Verification log

Native-owner `cargo test --all-features --test conditionals` with
`CARGO_TARGET_DIR=target/conditional-core`: **13 passed, 0 failed, 0 ignored**.
`cargo check --all-features` and `cargo build --all-features` passed.
Full tests: **446 all-feature + 2 doctests**, **386 core-only + 2 doctests**;
zero failed or ignored. Strict Clippy (`--all-targets`, separately all-feature
and no-default-feature), `cargo fmt --all -- --check`, and strict rustdoc
(`RUSTDOCFLAGS="-D warnings" cargo doc --all-features --no-deps`) passed.
`cargo package --list --allow-dirty` listed 1,606 entries and no artifacts, target,
reference virtualenv, cache or Python bytecode paths. Its existing metadata warning
(no license/license-file/documentation/homepage/repository) remains explicitly
unresolved; no publishing or package-manifest change was performed.
Initial all-suite attempts
identified stale prior-slice rejection assertions and independent authoring-test
assumptions; these are not recorded as passing runs. Independent final matrix and compatibility gates remain separate from these
native-owner checks; no full-language or simulation-readiness claim follows.
