# Rational-token compatibility repair discovery

**Historical discovery and implementer checkpoints (2026-10-14).** The later
Phase 3A.1 correction is now [independently verified](RATIONAL_CALLS_EVIDENCE.md);
the discovery observations and earlier counts below are preserved unchanged.
A supplemental Phase 3B investigation found two compatibility gaps after Phase
3A's prior gates passed. Those receipts remain immutable historical results;
these new cases reopen compatibility review, not retroactively change test counts.

## Reproducible evidence

All inputs, scripts, XML, errors, ASTs and logs are retained under
[`artifacts/conditionals-phase3b/discovery`](../artifacts/conditionals-phase3b/discovery):

| Receipt | Actual result |
|---|---|
| `rational-calls.json` | Initial 17 sequential-process probes; 8 accept, 9 reject. |
| `rational-grid.json` / `rational_grid.py` | 175 isolated pinned reference processes, 138 accepts, 37 rejects, no process failures. Immutable native check accepts149. |
| `rational-native-exports.json` / `rational_native_exports.py` | 149 native exports succeed:106 reference-accepted exact expression ASTs;17 reference-accepted AST mismatches;25 upstream-rejected sources;1 scalar initializer has no expression AST (`1//2` ends in a comment). |
| `rational-edges.json` / `rational_edges.py` | 38 isolated reference probes,26 accepts,12 rejects. |
| `rational-limits.json` / `rational_limits.py` | 18 isolated reference probes,13 accepts,5 rejects. |

The 231 isolated rational discovery inputs are executions, not a count of unique
sources or passing compatibility tests. They use existing Antimony3.2.0 and
independent libSBML5.21.2. Antimony XML identifies bundled libSBML5.21.1.
The 149 native exports use only the existing immutable binary
`artifacts/builtins-phase3/bin/antimony`, SHA256
`8e48d4ed7e41c4c57cac0275d47e173cb91c84c7a02f500185b633977f376bae`.
No native rebuild, source change or new dependency was needed for discovery.

Reproduce from repository root:

```sh
python3 artifacts/conditionals-phase3b/discovery/rational_grid.py
.venv-oracle/bin/python artifacts/conditionals-phase3b/discovery/rational_native_exports.py
python3 artifacts/conditionals-phase3b/discovery/rational_edges.py
python3 artifacts/conditionals-phase3b/discovery/rational_limits.py
```

## Two distinct failures

1. `a=sin(1/2);` succeeds natively but fails upstream with an unexpected rational
   notation token. All tested single-argument numeric builtin spellings share the
   issue, including single-argument min/max. Spaces, comments and decimal spelling
   `1.0` do not avoid it. This is not invalid numeric division in general.
2. `a=(1/2);` and `a=sin((1/2));` load upstream with a rational atom (AST259,
   `<cn type="rational">1<sep/>2</cn>`). Native export incorrectly uses divide.
   Numeric agreement at0.5 cannot substitute for exact expression compatibility.

Positive neighbors distinguish the grammar:

- `sin((1)/2)`, `sin(1/(2))`, `sin(+1/2)`, `sin(1/+2)` export divide.
- `sin(0.5/2)` and `sin(1/2+0)` export ordinary arithmetic.
- `pow(1/2,3)`, `log(2,1/2)`, `min(1/2,3)` export divide arguments.
- `sin((1/2)+0)` contains a rational atom inside addition.
- `sin(1e14/2)` rejects; `sin(1e15/2)` loads with a scientific-number operand.

## Primary-source mechanism

The pinned [Antimony grammar](https://github.com/sys-bio/antimony/blob/f3f5c6b7f3f22ab48f8e382a9318ef50bce2b4b2/src/antimony.ypp)
passes number tokens through `Formula::AddNum` and retains mathematical punctuation.
[`Formula::AddNum`](https://github.com/sys-bio/antimony/blob/f3f5c6b7f3f22ab48f8e382a9318ef50bce2b4b2/src/formula.cpp)
uses a C++ stream with precision15 before constructing the formula. Thus source
`1.0` and `1e0` become `1`; large integral values can become scientific notation.
These pinned research sources explain observations but do not prove wheel identity.

The [libSBML5.21.1 grammar/lexer](https://github.com/sbmlteam/libsbml/blob/v5.21.1/src/sbml/math/L3Parser.ypp)
recognizes an entire parenthesized signed-integer quotient as one rational token,
including its parentheses. A function requires a separate opening-parenthesis
token, so the same sequence immediately after its name causes a syntax error.
Expression-position rational tokens produce rational AST atoms. An optional minus
is recognized on each component; unary plus and nested operand grouping prevent
this lexical match. The [5.21.2 source](https://github.com/sbmlteam/libsbml/blob/v5.21.2/src/sbml/math/L3Parser.ypp)
contains the same mechanism. This is a lexical distinction, not algebraic folding
of arbitrary divide trees.

## Bounded repair contract proposed to root

- Add `Rational { numerator: i64, denominator: i64 }` to syntax and public
  expression IR. Store the exact unreduced signed pair; never reduce2/4, move a
  denominator sign, or turn the atom into a generic binary divide node.
- Recognize the lexical shape at a source opening parenthesis, before parentheses
  are discarded. Match exactly optional one minus, numeric token, slash, optional
  one minus, numeric token, close parenthesis. Apply existing scalar precision and
  magnitude restrictions first. A component qualifies only when its verified
  precision15 default-format spelling is a plain integer; existing scientific
  notation threshold logic is shared, not reinvented using only `fract()==0`.
- At function-call opening, the same match is a **Syntax** error. Do not ban
  division generally and do not mislabel it as an unsupported builtin.
- Valid final components are inclusive signed32-bit integers. `(2147483647/2)`,
  `(-2147483648/2)` and denominator equivalents load; adjacent outside values fail
  upstream MathML parsing. Wider internal storage permits checked unary negation.
- Existing unary normalization folds minus into the numerator:
  `-(1/2)` becomes(-1/2), `-(1/-2)` becomes(-1/-2).
  `-(-2147483648/2)` fails upstream after its positive numerator exceeds the bound.
  Validate the final normalized leaf with Syntax errors for out-of-range
  components; do not overflow or wrap. Existing adjacent
  `--` rejection remains separate; spaced `- -(1/2)` accepts.
- A zero denominator is **UnsupportedFeature in every context**: upstream emits
  nonfinite INF/NaN rather than a finite rational leaf, including rate expressions.
  This is a deliberate finite-literal profile restriction. Ordinary rate `1/0`
  remains structurally expressible, with no runtime-domain guarantee.
- Evaluation uses the same typed pair, converts safely to finite f64 quotient,
  and applies the existing initialization checks. No second string evaluator.
  Bare rational dimensions remain unspecified, like bare numeric literals;
  no invented dimensionless unit certificate. Atom references/modifiers are empty.
- Export uses MathML rational cn/sep. Public getters, normalization, symbol
  inference, dependency traversal, units and lowering all consume this same node.
  Source formatter retains numeric lexemes and grouping; as before, it removes
  comments. The parsed source still retains original comments and spans.
- Preserve ordinary divide behavior, all existing arithmetic grouping rules,
  iterative traversal, hard logical depth and configured resource bounds.
  Add focused endpoint, lexical-context, formatter and stress regressions, then
  independently rerun previous corpora plus new supplemental evidence.

## Remaining risks and decision status

Root prefers typed rational support and accepts the finite zero-denominator
boundary. Root authorized implementation after endpoint review, using profile
`math-core-v0.2.1-slice`. The native repair is in progress. Exact unary-collapse/endpoint ordering and
resource accounting require tests; lexical recognition must not discard source
limits. Existing Phase3A gates are necessary regression checks, not sufficient
coverage of this newly discovered atom. Phase3B implementation remains paused.

## Implementer verification and architecture audit

**Implementation complete; independent full matrix and supplemental gates remain
pending.** This is an implementer-authored audit, not independent signoff.

Commands used `CARGO_TARGET_DIR=target/rational-repair`:

- `cargo test --all-features`:400 Rust tests +2 doctests pass;0failed/ignored.
- `cargo test --no-default-features`:342 Rust tests +2 doctests pass;0failed/ignored.
- `cargo clippy --all-features --all-targets -- -D warnings`:pass.
- `cargo clippy --no-default-features --all-targets -- -D warnings`:pass.
- `cargo fmt --check`:pass.
- `rational_repair_precheck.py`:231 retained discovery input executions,
  153 native accepts,0 native-accept/reference-reject,0 accepted-expression AST
  mismatches. Receipt `rational-repair-precheck.json` records the separate native
  binary hash. This is an expression precheck, not a full SBML-equivalence gate.
- All3 prior immutable binary hashes still match their unchanged manifest.

The first native-test attempt had two incorrect test expectations (the existing
cycle category and the formatter's established comment removal), corrected without
changing behavior. A prior symbol test expected domain errors for `(1/0)`;
its two original sources now explicitly expect unsupported nonfinite rational
literals, alongside two new `((1)/0)` companions retaining divide-domain coverage.
Profile assertions were migrated to the corrective identity, not deleted.

Concrete inspection covered every expression consumer:

| Consumer | Inspected behavior |
|---|---|
| `syntax/rational.rs`, `syntax/expression.rs` | Bounded token lookahead; canonical numeric spelling distinction; no division-tree heuristic; source span covers the atom; logical grouping depth is still checked. |
| `semantic/expression_normalization.rs` | Rational unary folding uses safe i64 values below1e15; only reachable retained atoms undergo final i32/zero checks. Dead transient out-of-range numerators do not falsely fail rescued signs. |
| `semantic/initialization.rs` | Rational has no dependencies; numeric evaluation derives from the same pair; finite checks and symbolic `None` remain unchanged. |
| `semantic/symbols.rs`, `semantic.rs` | Symbol inference/modifier traversal treats rational as a leaf; lowering preserves both components exactly into public IR. |
| `semantic/units.rs` | Bare rational is unspecified, not assumed dimensionless. Other dimensional checks remain intact. |
| `sbml.rs` | Exact unreduced rational cn/sep emission, no divide replacement or special string evaluator. |
| `format/antimony.rs` | Existing source-token formatting is unchanged; regression tests cover lexemes, grouping, comment removal, idempotence and same exported expression. |

No conditional variants, operator spellings, dependencies, runtime, unsafe code,
file/network execution paths or global registries were introduced. No native
integer arithmetic can overflow in the rational path: the recognized decimal
component bound is below1e15 and unary folding only changes its sign; final output
is signed32-bit and denominator is nonzero. All expression traversal remains
bounded and iterative. Reviewer tests exercise159 reachable nodes for80 rational
atoms and raised-depth/source/token limits. No additional architecture blocker was
found in this supplemental inspection.
