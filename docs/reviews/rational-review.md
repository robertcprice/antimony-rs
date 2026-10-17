# Independent rational-expression patch review

**Status: independent native matrix, original-model checks and all ten maintained evidence gates PASS.**

## Why this patch is necessary

After Phase 3A's bounded corpus and native matrix passed, fresh conditional syntax
investigation exposed a neighboring arithmetic conformance defect. Unary calls
such as `sin(1/2)` were accepted natively but rejected by the pinned reference;
accepted grouped `(1/2)` expressions exported division rather than rational
MathML. The original passing receipts remain unchanged historical evidence.
Neither a loader-only check nor numeric equality detects the latter structural
mismatch.

The independent original 76-case scan found 22 overacceptances and 19 exact AST
mismatches, alongside 33 exact matches, one common arity rejection and one retained
numeric-envelope rejection. Another 110 boundary and 27 signed probes establish
canonical-number behavior, component ranges and negation rules. These discovery
receipts remain under `artifacts/conditionals-phase3b/independent/`.

## Agreed bounded contract

The patch targets `math-core-v0.2.1-slice`. Rational expressions retain unreduced,
signed numerator and denominator components and exact MathML identity, not a
rounded scalar or disguised division. Public fields are `i64`; retained final
components fit inclusive signed 32-bit bounds. Internal sign-folding intermediates
may exceed that range before final normalization: `-(-2147483648/2)` rejects,
but `-(-(-2147483648/2))` is valid and must survive.

Recognition follows reference canonical numeric spellings, not merely `f64`
integrality or the expression tree. Scientific notation at magnitude `1e15`
prevents rational token recognition. Unary plus and extra grouping around an
individual component also preserve ordinary division. Unary function-call opening
parentheses are not available for a rational argument token: an extra whole group
is required.

Reference-invalid rational component/normalized overflow and consumed call-opening
forms produce `Syntax`. Reference-valid zero-denominator rational constants become
nonfinite MathML upstream and remain `UnsupportedFeature` in **all** native
contexts. Ordinary unparenthesized runtime division is unchanged. Existing source
numeric precision restrictions also remain unchanged.

## Independent tests and original-model verification

`tests/rational_independent.rs` provides 14 tests (13 without SBML) covering:

- Unreduced signs, canonical spellings and final-only sign folding.
- Inclusive component endpoints and invalid overflow.
- Ordinary-division near-neighbors and all 14 supported unary spellings.
- Nested rational arguments and exact rational MathML.
- Zero denominators, unknown initialization, cycles and finite domains.
- Conservative dimensions, UTF-8 provenance and formatting roundtrips.
- Reachable expression IDs/node counts and source/token/hard depth limits.

`artifacts/rational-phase3a1/independent/models.py` checks 379 inputs: 213 original
discovery cases, 121 fresh syntax neighbors and 45 lexical-trivia neighbors.
The final immutable CLI gives **264 exact exports and 115 expected rejections**.
Positive cases also pass independent libSBML Error/Fatal checks, closed projection
vocabulary, formatter byte-idempotence and identical re-export. Comments including
embedded newlines do not behave like actual expression newlines; both cases are
explicitly tested. These are targeted inputs, not a language-coverage percentage.

### Transparent correction of reviewer expectations

The first 379-case run produced 264 exact matches, 112 expected rejections and
three unexpected rejections. The reviewer had mistakenly marked ordinary initial
`(1/+0)` (two independently sourced cases) and `(0/+0)` as supported. They retain
division ASTs upstream, but violate the **preexisting** finite-real initialization
policy. Their expected outcome was explicitly corrected to `DeclarationConflict`,
without changing native behavior, sources or reference XML. The original manifest
and failed report remain `reference-models-initial379.json` and `first-models.json`;
`expectation-correction.json` records exactly these three changes. The 334-case
pre-trivia manifest is also retained. Final results must not conceal this history.

## Source-frozen matrix and immutable handoff

**17 commands passed**: formatting plus build/test/doctest/strict Clippy across
four configurations, all offline and locked using the existing local toolchain
and dependency cache. Clippy uses `--all-targets -- -D warnings`; builds and ordinary
tests also use all targets. This is local verification, not cross-platform CI or
a hermetic/reproducible-build claim.

| Configuration | Unit/integration tests | Doctests | Build/Clippy |
|---|---:|---:|---|
| All features | 400 passed | 2 passed | PASS |
| Default | 355 passed | 2 passed | PASS |
| Core only | 342 passed | 2 passed | PASS |
| CLI without SBML | 387 passed | 2 passed | PASS |

No test failed or was ignored. All **58 Cargo/src/tests/examples hashes** remained
unchanged across the matrix. The reviewer's 14 tests (13 core), Agy's 12 authoring
tests and native owner's nine tests are included in these totals, not additive.
Agy's helper was strengthened with parent authorization to match the actual
`Rational` variant rather than merely asserting that a node was not division;
all 12 tests and their coverage were retained. A separate pinned-reference check
of its sources gives **11 exact model exports and 14 reference/native syntax
rejections**, using the final copied CLI.

The final copied CLI also replays the unchanged earlier independent exercises:
**203 builtin inputs = 164 exact exports + 39 expected rejections**, and
**191 arithmetic inputs = 183 exact exports + eight expected rejections**.
Those checks retain layout/reference validation and do not claim numerical
trajectory validation. Existing Phase 3A output paths were never overwritten.

New immutable artifacts under `artifacts/rational-phase3a1/bin/` were copied once,
only after the successful stable matrix, with exclusive directory creation:

- CLI: `07cb70eb6e37c41a3a56fb50a426f3770c31cd55ad1769a9222e6fed1337f971`
- Exporter: `c8b216b4ac3e72c69adad203526aede542e5de186763f14e7ff08409d23e64e7`
- Formatter: `ef1f08f4f2d74db68b2f49cf931133615fdb46adf18499555a7d6bf12b09a28a`

Matrix commands/log identities are in `independent/matrix.json`; the four final
original-model receipts record the same copied CLI identity. All ten maintained gates ran separately against this handoff and passed; their
identities and case outcomes were independently inspected as detailed below.

## Independent architecture inspection

Read-only inspection traced the new leaf through bounded token lookahead,
iterative parsing, normalization, dependency collection, numeric initialization,
units, public IR and MathML. The recognizer checks canonical plain-integer spelling
before recognizing the precise token sequence; it does not reinterpret arbitrary
divide subtrees. Source grouping depth remains checked even when a rational token
collapses into one atom. Negation intermediates remain below `1e15` magnitude, so
i64 sign changes cannot overflow. Only final retained normalized leaves receive
i32/zero checks; rescued outer signs do not fail on dead intermediate nodes.

Evaluation and export consume the same typed pair. Rational dependencies/modifiers
are empty, dimensions remain unspecified like other bare numeric literals, and
unknown-valued expressions remain symbolic. No raw-string evaluator, fixture-ID
branch, external runtime dependency or core file/network execution was introduced.
Normalization preserves the existing node-count bound; the reviewer checks 159
reachable owned nodes for 80 rational leaves and the source/token/hard-depth limits.
The native owner's audit in `docs/RATIONAL_CALLS.md` is supplemental, not a
substitute for this independent review.

## Maintained gate audit and final signoff

All ten reports pass, with no failed/blocked cases or changed inputs:

| Gate | Verified result |
|---|---|
| New corrective rational corpus | 73 exact exports; 51 expected rejections |
| Original 90 cases | 56 exact exports; 34 expected rejections |
| Same 90 through formatter | 56 exact/idempotent roundtrips; 34 expected rejections |
| Same 90 through CLI | 56 supported; 34 expected rejections |
| Arithmetic 54-case current overlay | 48 exact exports; six expected rejections |
| Symbols 66-case corpus | 54 exact exports; 12 expected rejections |
| Builtins 109-case corpus | 66 exact exports; 43 expected rejections |
| Workloads | Six motifs + 68 generated positives; 12 expected rejections |
| Historical analytic trajectories | Eight passed |
| Existing exp/sin/time trajectories | Two passed |

These inventories overlap and must not be summed as distinct validated models.
All rational and other structural checks retain numerical **NOT_RUN**; no new
rational-specific trajectory certification is claimed. Existing ten numerical
anchors are regression evidence only. Six nonfinite rational reference cases
retain a separate nonfinite audit and native rejection, not finite projection or
equivalence credit. The ordinary divide-zero rate neighbor remains structural
support without a runtime safety promise.

Separately **392 Python tests passed**, zero failures/skips: 230 oracle/CLI,
80 numerical, 51 workload and 31 source-receipt tests. The full historical source
receipt was not regenerated and is not current-source proof.

`artifacts/rational-phase3a1/independent/gate-inspection.json` records inspection of
all ten final report identities and case statuses, matching new binary identities,
58 unchanged native source hashes and all three **old** Phase 3A binary hashes
still matching their original manifest. No old immutable artifact was replaced.
The root separately recompiled and executed the finalized core-only guide snippet;
its three-command PASS receipt is `artifacts/rational-phase3a1/api-guide/receipt.json`.

No remaining independent-review blocker was found for the bounded
`math-core-v0.2.1-slice` corrective increment. This does not certify all Antimony,
claim official endorsement, or make structurally valid models simulation-ready.
Phase 3B is a separate implementation/review decision; this patch does not add
conditional expressions, rules or user-defined functions.

## Key Learnings

1. Numeric equality cannot replace exact rational-versus-division expression identity.
2. Source grouping, sign spelling and canonical numeric serialization can determine
   lexical structure; normalized arithmetic alone is insufficient to reconstruct it.
3. A passed bounded corpus remains historical evidence when fresh counterexamples
   appear; preserve it, add explicit regressions and verify a new immutable build.
