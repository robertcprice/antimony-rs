# Independent conditional-expression review

**Current status: independently signed off — native matrix, independent exact-model
grids, focused architecture review and all twelve maintained gates PASS.** Profile: `conditional-core-v0.3-slice`.
The earlier rational defect recorded at the end of this document is **resolved** by
Phase 3A.1, not an outstanding conditional blocker. Historical artifacts are intact.

This reviewer owns adversarial tests and independent evidence, not native conditional
implementation or maintained Python gates. This is a bounded implementation, not
full Antimony compatibility or an official standard. Structural validity does not
imply complete initial state or simulation readiness; no solver is added to the crate.

## Frozen native verification

`artifacts/conditionals-phase3b/independent/matrix.json` records **17 successful
commands**: formatting, and build/all-target tests/doctests/strict all-target Clippy
for each of four feature configurations. Commands use offline, locked dependencies
and a separate review target directory.

| Features | Rust tests | Doctests | Failed / ignored |
|---|---:|---:|---:|
| Default | 401 | 2 | 0 / 0 |
| All | 446 | 2 | 0 / 0 |
| Core only | 386 | 2 | 0 / 0 |
| CLI without SBML | 431 | 2 | 0 / 0 |

All **64 Cargo/source/test/example hashes** are unchanged before/after the matrix.
The restored all-feature generation was copied once into a newly created exclusive
`artifacts/conditionals-phase3b/bin/` directory. Its manifest binds all downstream
native executions:

- CLI: `4fccc60b1a5f9e490c90a78a1232ba393c08a46170e6f6db51e48e352c095a30`
- Exporter: `6952011f02efa183473eabb7545e6b95283ca70e575ec1f71218743e783a6964`
- Formatter: `46e1dbbe51721c54de68d5e9ce915a299f34b529a6dd92d44acb991187ee1f42`

## Independent contracts and actual executions

| Corpus on frozen CLI | Exact positive exports | Expected rejections | Failures |
|---|---:|---:|---:|
| Current conditional 343 | 236 | 107 | 0 |
| Historical rational 379 | 264 | 115 | 0 |
| Historical builtin 203, explicit current overlay | 165 | 38 | 0 |
| Historical arithmetic 191 | 183 | 8 | 0 |

Positive checks require complete canonical model/ordered-AST projection equality,
closed MathML vocabulary, no libSBML Error/Fatal, formatter idempotence and identical
re-export. This is not byte equality of reference XML or a compatibility percentage.
Negative cases earn rejection-only credit, not supported-language equivalence.
The current contract was prepared before native outputs: 187 post-rational cases,
58 lexical neighbors, 24 Boolean-call probes, 64 newly composed models, and 10 full
Agy authoring models. Source strings, reference XML, expectations and hashes are
retained in `independent/model-contract.json`; results are `final-models.json`.

The historical builtin case `piecewise_deferred` is now intentionally supported:
`x=piecewise(1,time>0,0);`. `builtin-replay-overlay.json` binds the old script hash,
new script hash and this sole outcome change. Historical 164/39 remains unchanged;
current 165/38 is explicitly not an unchanged-expectation replay. Rational and
arithmetic expectations are unchanged. All outputs are in the new phase directory.

`runtime-known.json` separately records **32 known finite controls** matching pinned
RoadRunner values within 1e-14. These are selected initializer observations, not
unknown semantics, arbitrary-trajectory proof or a substitute for the maintained
analytic trajectory gate.

The exact named `conditional_input` sample in `docs/CONDITIONALS.md` passes reference
projection, libSBML checks, formatting and re-export (`guide-sample.json`). Its source
SHA is `bf686466028ae955c92a0ea1a96219c3f2891178a9fa56b12bfae54e6a87c328`.
This receipt claims structural equivalence only; the separately frozen implicit-model
analytic anchor is the sole associated numerical trajectory claim.

## Adversarial tests and evidence-backed test repairs

Reviewer tests: **21 all-feature / 20 core**, including typed Boolean preservation,
finite coercion, exact operator/function nesting, chain-wrapper rejection, selected
branch evaluation, eager comparisons, first-unknown stopping, inactive cycles,
all-branch units/modifiers, rational boundaries and reserved-name behavior.
A 300-branch piecewise has **602 unique reachable nodes**; 3000-deep NOT, function,
piecewise, logical and comparison shapes reject safely in subprocesses with caller
limit 10000. A 96-level supported piecewise control succeeds. No stack size increase
or reduced implementation ceiling was used. All belong to the full matrix counts.

Agy's 12 ordinary-authoring tests pass all/core. Its original source is preserved as
`independent/agy-original.rs`, and first runs honestly retain 7/12 then 11/12 passes.
Repairs were expectation/source-contract corrections, not native changes:

- An unknown/lazy test used a zero-denominator rational literal; its intended dynamic
  division became `((1)/0)`, while original rational rejection remains a negative.
- A custom unit ending in `s` hit an existing exclusion; use `u_clock` for dimension
  coverage and retain the excluded `u_s` negative.
- Numeric XML spelling and modifier ordering assertions now respect the established
  semantic projection contract. Reference uses source modifier order and can omit
  `type="real"`; native canonical ID order and explicit real type are equivalent.
- A missing-otherwise assertion now counts the legitimate authored zero condition,
  rather than prohibiting any zero leaf. The absent otherwise is still asserted.
- A formatter substring assertion no longer mistakes identifier `keep_nested_and`
  for a textual `and` operator.

## Lexical and Boolean boundaries

Fresh 58-case operator probes verify that `= =`, `& &`, and `| |` may reconstruct
across spaces/tabs/block comments, including embedded block-comment newlines;
actual newline tokens block reconstruction. Separated `< =`, `> =`, `! =` reject.
The lexer counts original tokens before bounded selective coalescing.

Six bare Boolean aliases and four capitalized zero-argument calls are supported.
Lowercase calls reject; eight tested capitalized nonzero calls load upstream but
produce null MathML. They deliberately earn **no positive equivalence credit**.
The 24-case independent probe distinguishes these outcomes, and the Rust test
checks their diagnostic boundary. Mixed-case ordinary identifiers are not Boolean
constants. Coercion happens during evaluation, retaining Boolean MathML leaves.

## Focused anti-pattern and resource audit

Source inspection, supported by `independent/audit-grep.txt` and the frozen hashes:

- `syntax/expression.rs` uses an explicit Pratt continuation stack; no recursive
  expression calls. Parser and normalized-arena hard depth remain **256**. Original
  token and expression counts are checked; variadic arguments are admitted through
  parsed token-backed nodes, not an unbounded external input vector.
- `syntax/functions.rs` converts bounded arguments into typed variants. Piecewise
  pairs and optional otherwise are retained without a sentinel/fabricated zero.
  `ExprKind::children` / `map_children` in `syntax/mod.rs` include every branch and
  otherwise; the child iterator allocates nothing.
- `semantic/expression_normalization.rs` moves obsolete left logical vectors using
  `mem::take`, avoiding growing copies at every infix append. Lowering is iterative,
  checks normalized depth and forbids normalized node count above admitted source
  nodes. Functional/right nesting remains intact. The existing wide-function-plus-
  infix regression supplements the reviewer unique-ID/wide-piecewise test.
- `semantic/evaluation.rs` has typed Numeric/Boolean/Unknown and explicit Visit,
  Apply, Logical and Condition tasks. Unknown maps to `None`, never 0 or false.
  Logical and piecewise demand only the selected continuation; comparisons visit
  all children before applying their operation. Numeric nonfinite checks remain.
  `unwrap_or(0)` grep hits are empty-child **depth calculations**, not value fallbacks.
- `semantic/initialization.rs` retains iterative all-child dependency collection and
  Kahn resolution; inactive references/cycles remain static. `semantic/units.rs`
  traverses every branch and otherwise, including known conflicts behind unknowns.
  Bare rational dimensions remain Unspecified; Boolean dimensions are known unitless.
- `semantic.rs` preserves all branches during public-IR lowering and modifier walks.
  `sbml.rs` emits ordered piece/value/condition and optional otherwise through an
  explicit task stack, not recursive output or silently discarded inactive branches.
- `conditional.rs` is enum metadata/finite comparisons only. Cargo dependencies
  remain optional quick-xml and serde_json; no solver, host-language expression
  execution, external runtime, mutable global registry or unsafe code was added.

This is a focused source and bounded-stress review, not a proof of every possible
input or a claim of constant memory. Storage scales with admitted nodes/tokens and
explicit work stacks; limits are essential. Future lazy constructs must extend the
demand evaluator deliberately, not reuse eager numeric-call evaluation indiscriminately.

## Final maintained-gate inspection and signoff

`independent/gate-inspection.json` records the final independent receipt inspection:
all **12 maintained gates PASS**, all 64 frozen source hashes and **9 immutable
binary hashes** (current, rational repair, original builtin phase) rechecked.
**1311 existing input paths** were independently rehashed. Logical alias and
installed-package labels retain their own gate before/after checks; the inspection
receipt explicitly lists aliases not independently path-resolved, rather than
claiming a stronger audit. All 159 positive conditional cases have successful exact
comparisons and no libSBML Error/Fatal; 103 negatives earn rejection-only credit.

| Maintained gate | Verified result |
|---|---|
| Conditional 262 | 159 supported / 103 expected rejection |
| Rational 124 | 73 / 51 |
| Original, formatter, CLI (90 each) | 56 / 34 each |
| Arithmetic 54 | 48 / 6 |
| Symbols 66 | 54 / 12 |
| Builtins 109 | 66 / 43 |
| Workload 86 | 74 supported (68 generated + 6 motifs) / 12 rejection |
| Original numerical | 8 passed |
| Builtin numerical | 2 passed |
| Conditional numerical | 1 passed |

The single new analytic anchor uses 101 points on [0,10], including join t=5.
Native/reference differential maximum error is 0; each analytic maximum absolute
error is approximately **1.13e-10**. This continuously joined rate/state example
is not evidence for discontinuities, events or arbitrary trajectories. Structural
corpus numerical stages remain explicitly NOT_RUN.

Final Python logs independently inspected: **441 passed, 0 failed, 0 skipped**
(267 oracle, 92 numerical, 51 workloads, 31 verification). The evidence owner
preserved an earlier wrong-directory discovery attempt separately; it ran no tests
and is not concealed in these final counts. The root's final core-only API guide
receipt has exit0 and matches current guide/matrix hashes.

**Independent signoff: PASS for this bounded conditional slice.** No native blocker
remains from this review. No further source/test/review edits are planned. Future
assignment/rate rules require their own reference discovery and frozen contracts;
this signoff does not authorize or imply support for them.

---

# Historical discovery record (superseded status, preserved evidence)

The following discovery occurred before the separately verified rational correction.
The original inputs, failures and prior frozen binary are retained, not rewritten.

## Fresh independent discovery

`artifacts/conditionals-phase3b/independent/probe.py` runs original inputs in
separate pinned Antimony 3.2.0/libSBML 5.21.2 processes and records complete XML,
ordered ASTs, parser diagnostics and independent consistency messages.
`reference.json` contains **156 inputs: 120 loader accepts and 36 rejects**, zero
process failures and no libSBML Error/Fatal messages among accepted exports.
These are structural observations, not native support or numerical gate credit.

The inputs independently confirm equal-precedence left-associative AND/OR,
functional logical arities 0/1/2/3/5/17, relational functional arities 2/3/5/17
(except `neq`, exactly two among tested values), and piecewise arities 1/2/3/5/17.
They also inspect namespace collisions, inactive cycles, unknown branches and
unit-warning cases.

### Additional normalization constraints

Outer infix construction can flatten even a **functional** left operand:

- `lt(1,2)<3` becomes a three-child `lt` chain.
- `lt(1<2,3)` and `lt(lt(1,2),3)` retain the nested left relation.
- `and(true,false)&&true` becomes three-child AND.
- `and(and(true,false),true)` retains the nested call.
- Right-nested operator/call operands retain their nesting.

Thus a deferred-chain guard cannot look only for an infix-produced left child.
The semantic construction of the outer operation also matters. These observations
refine, rather than contradict, the source-provenance requirement in the discovery
note. `!2^0` exports `!(2^0)`: power remains tighter than unary NOT.

The proposed finite contextual conversions, all-branch static checks and separate
lazy selected-branch evaluation remain coherent. Sequential logical short-circuit
rejection of a demanded nonfinite first operand is an explicit native restriction,
not a reproduction of every RoadRunner nonfinite observation. At that historical checkpoint, conditions were not yet implemented or signed off.

## Historical rational defect — resolved by Phase 3A.1

The native owner discovered that the existing unary builtin `sin(1/2)` is accepted
by frozen native Phase 3A but rejected upstream as rational-notation syntax.
The reviewer independently expanded this into **76 inputs**, using the immutable
CLI with SHA256
`8e48d4ed7e41c4c57cac0275d47e173cb91c84c7a02f500185b633977f376bae`.

Exact comparison results (`rational-comparison.json`):

| Outcome | Count |
|---|---:|
| Native accepts; reference rejects | 22 |
| Both accept; exact AST mismatch | 19 |
| Exact projection match | 33 |
| Both reject wrong arity | 1 |
| Native retains existing numeric boundary rejection | 1 |

The 14 unary spellings tested all reject whole integer-ratio arguments upstream,
including one-argument `min`/`max`. Spaces/comments do not escape the behavior.
Integer-valued `1.0/2`, `1/2.0` and `1e0/2` also reject after upstream number
canonicalization; `0.5/2` is accepted and matches.

A second defect is distinct from this overacceptance: `sin((1/2))`, `min((1/2),3)`
and the preexisting arithmetic `(1/2)` load upstream but export a **rational
numeric leaf** (AST type 259), whereas native exports a divide application
(type 47). Equal mathematical values do not satisfy the exact structural contract.
By contrast, `sin((1)/2)` preserves division and matches exactly. Multiargument
`pow(1/2,3)`, `log(2,1/2)` and `min(1/2,3)` also match.

Scripts `rational_probe.py` and `rational_compare.py` retain all sources, outcomes,
reference XML and precise projection differences. No native source or old frozen
artifact was modified. This finding required a separately verified rational patch before resuming Phase 3B.
That patch is now independently signed off in [rational-review.md](rational-review.md);
typed unreduced rational atoms and call-open syntax boundaries resolved both defects.
The current 379-case rational replay below passes without changing its expectations.

