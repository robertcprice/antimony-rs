# Phase 3A numeric-expression architecture review

Date: 2026-10-14. Profile: `math-core-v0.2-slice`.

**Disposition: no remaining implementation blocker found in this review.** One
misleading public API comment was found and corrected with the coordinating
agent's explicit authorization before final freeze. This is a supplemental
**implementer-authored** architecture audit, not an independent compatibility
review or a substitute for the separate final feature matrix, reference gates,
and immutable-binary handoff.

## 1. What was built and inspected

The compiler now represents numeric function calls, mathematical constants, and
time explicitly. Arguments are owned arena IDs, not source strings or executable
host callbacks. Source syntax remains separate from the normalized semantic tree;
initial values, units, dependencies, modifiers, and SBML export consume that same
normalized representation. Time-dependent initial values remain symbolic rather
than silently becoming zero.

The audit inspected the native expression-variant references, their consumers,
crate feature/dependency declarations, public API comments, and existing test
logs. No graphify report/wiki was present. Structural search was supplemented by
exact searches for expression variants and execution/I/O/zero-fallback patterns.
Only this review document and the explicitly authorized facade rustdoc correction
were changed during the review; native behavior and tests were left untouched.

At the inspection checkpoint, the 31-file Cargo/native-source fingerprint was
`26b7e2aa52e30a9f6931af446c74b30348b2ffbf086d57749e1cc3904191e758`.
The digest covers `Cargo.toml`, `Cargo.lock`, followed by lexicographically sorted
`src/**/*.rs`; for each file, hash its UTF-8 relative path, NUL, and binary SHA-256
content digest into the aggregate SHA-256. This is an inspection identifier, not
a final source receipt or a test-to-binary binding.

## 2. Expression-consumer audit

| Consumer | Inspection and result |
|---|---|
| Source parser | [`syntax/expression.rs`](../../src/syntax/expression.rs) uses explicit Parse/Continue/Unary/Binary/Call tasks. Builtin lookup and arity validation precede node creation. Unary `log` resolves to `Ln`; binary `Log` retains `(base, value)`. Time and constants are distinct leaves. |
| Arithmetic normalization | [`expression_normalization.rs`](../../src/semantic/expression_normalization.rs) introduces `Shape::Call` with aliased child IDs, visits all arguments in both normalization and lowering passes, then remaps arguments to postordered output IDs. Calls do not become associative arithmetic groups. Time/constants remain leaves. |
| Declaration inference | [`symbols.rs::infer_expression`](../../src/semantic/symbols.rs) pushes every call argument in reverse for source-order traversal. Time/constants are intentionally not inferred parameters. |
| Dependency graph | [`initialization.rs::resolve`](../../src/semantic/initialization.rs) traverses every argument, collects only symbol dependencies, and treats time/constants as dependency-free leaves. Kahn traversal and cycle detection remain iterative. |
| Initial evaluation | [`initialization.rs::evaluate`](../../src/semantic/initialization.rs) traverses all numeric-call arguments, preserves their order, consumes exactly the call's operand count, and invokes the enum-based evaluator. Constants have explicit numeric approximations; time yields `None`. |
| Public IR lowering | [`semantic.rs`](../../src/semantic.rs) explicitly maps Call/Time/Constant syntax into matching immutable model nodes. Call IDs are model-scoped, as documented in [`model.rs`](../../src/model.rs). |
| Reaction modifiers | The reaction-rate walk in `semantic.rs` descends into every call argument. Only species symbols absent from participant lists become modifiers. Time/constants cannot masquerade as species. |
| Dimensions | [`units.rs::check_expressions`](../../src/semantic/units.rs) handles all new variants in postorder. `builtin_dimensions` consumes shared enum unit behavior. Time uses the actual resolved model default, including dimensionless defaults; absence remains unspecified. |
| Arena-wide scans | Reaction-reference rejection, source-order provenance, and species-unit ordering scan the entire normalized arena in `semantic.rs` and [`unit_mapping.rs`](../../src/semantic/unit_mapping.rs). These are not recursive root walkers: symbol nodes inside calls are already included. Ignoring non-symbol leaves is deliberate. |
| SBML | [`sbml.rs::math`](../../src/sbml.rs) explicitly handles Call/Time/Constant. It emits the time csymbol, constant elements, root degree 2, base-ten log base 10, and ordered explicit log bases. Its traversal uses tasks, not recursive expression calls. |
| Formatting | [`format/antimony.rs`](../../src/format/antimony.rs) is deliberately token-based, not an IR walker. It validates first, retains call spelling/commas/grouping, and reparses/reanalyzes generated text under original limits. No reconstruction from evaluated values occurs. |

No omitted new-expression consumer was found. The wildcard arms in symbol-only
walks intentionally ignore numeric leaves, while explicitly descending into calls.

## 3. Authority, safety, and resource findings

### No unknown-to-zero fallback

- Missing symbol initialization retains `None`; compartment handling retains its
  existing separately reported implicit-default contract.
- `Time` evaluates to `None`, including inside other initial expressions. An
  initial assignment remains exported even when its value is not known locally.
- Builtins check known invalid logarithm/root arguments before unknown propagation.
  A sibling unknown does not hide a known invalid intermediate expression.
- Finite intermediate/final checks occur in the shared initializer evaluator.
  Runtime kinetic laws are not evaluated or certified free of singularities.
- The two `unwrap_or(0)` occurrences found in the new expression path compute a
  maximum **structural depth**; neither substitutes a numeric model value.

### No source execution or new native runtime

[`builtins.rs`](../../src/builtins.rs) dispatches through a closed numeric enum and
Rust numeric operations. There is no arbitrary-function registry, evaluator of
source strings, foreign-language bridge, or dynamic callable resolution.
`Cargo.toml` retains only optional `quick-xml` for SBML and optional `serde_json`
for CLI reports. With both features disabled, it declares no normal dependencies.
No C/C++ or Python runtime dependency was added. The crate and Cargo lint policy
forbid unsafe code.

Execution/I/O pattern inspection found CLI-side filesystem handling in
[`bin/antimony_cli/io.rs`](../../src/bin/antimony_cli/io.rs), not in the language
core. CLI command enums and `std::process::id/ExitCode` are not execution of model
source. Source labels remain display/provenance data, not paths opened by parsing
or analysis. This is a scoped code inspection, not a complete OS security audit.

### Bounded, nonrecursive expression processing

- Parsing now uses heap-owned continuation tasks, with unchanged configured and
  hard-256 logical depth checks. Expression nodes store child IDs rather than
  recursively owned expression values, avoiding recursive drop.
- Normalization, initialization evaluation/dependency resolution, symbol inference,
  modifier collection, dimensions, and MathML traversal are iterative.
- Variadic argument storage is bounded by admitted source/tokens. It does not
  recurse once per argument. The formatter also enforces generated source/token
  budgets, so successful input parsing alone does not waive output limits.
- The initial recursive implementation did overflow the debug test-thread stack
  when callers raised configured depth and supplied deeply nested calls. This
  failure is retained as a finding, not counted as a pass. The explicit-task
  implementation was then tested at inclusive depths 2, 32, 128, and 256, with
  over-limit rejection, 2,000 nested calls, and a 10,000-argument maximum.

### Conservative dimensional knowledge

Known incompatible inputs reject even when another argument is unknown.
Unspecified or indeterminate inputs are not silently assigned dimensionless
units. Square roots only produce known dimensions when integer exponents divide
by two; otherwise the result remains indeterminate. Power analysis retains its
existing conservative boundary rather than introducing unsupported fractional
unit algebra. Numeric annotations never rescale values.

## 4. Public API documentation finding and correction

**Found:** `analyze()` in `src/lib.rs` still said explicit finite initial values
were required. That contradicted declaration-only symbols and symbolic `None`
values, although the crate overview and value getters already described them
correctly.

**Corrected with authorization:** the facade now documents retained absent and
symbolic initial values, finite **known** initial-expression values, and the fact
that structural validity does not establish simulation readiness. The parse and
analyze summaries now name the numeric-math flat-model slice. `cargo fmt --check`
passed after this documentation-only change. No runtime behavior changed.

The public getters still explain concentration versus amount, model-scoped IDs,
optional initialization values, source provenance, and the non-certification
meaning of `ValidatedModel`. The new call/constant/time API docs describe their
actual variants. Older generic “units slice” descriptions elsewhere are imprecise
historical naming, not assertions that functions or symbolic values are rejected.
No remaining contradictory native API contract was identified. Separate evolving
README/evidence guides were not treated as finalized by this audit.

## 5. Test evidence actually observed

These are **implementation-stage runs**, not the final independent matrix:

| Run observed by this agent | Result and limitation |
|---|---|
| `cargo test --all-features --no-fail-fast` | **365 Rust tests + 2 doctests passed**, 0 failed/ignored. Included native 17 builtin tests, independent 21 builtin tests, and Agy 14 authoring tests. Log: `/tmp/builtins-prefreeze-all.log`. |
| `cargo test --no-default-features` before Agy authoring file arrived | **291 Rust tests + 2 doctests passed**, 0 failed/ignored. Earlier coverage; not a result for the final test set. Log: `/tmp/builtins-core-tests.log`. |
| Later core-only run with Agy file | **Compilation blocked** by three borrowed-return closure lifetime errors in `tests/builtin_ergonomics.rs`. Not a pass; coordinating agent was notified and owns repair coordination. Log: `/tmp/builtins-prefreeze-core.log`. |
| Dedicated native builtin test run | **17 passed**, covering domains, arities, unknown propagation, modifiers, units, format, MathML, wide calls, and inclusive resource bounds. |
| `cargo build --all-features --all-targets` | Passed at the earlier implementation checkpoint; not a replacement for final frozen-source builds. |
| `cargo fmt --check` after API-comment correction | Passed during this review. |

The all-feature log SHA-256 is
`2ed9ceb1b498879a254d35b0558c33b92d45893647c1c7b9c68e9d38636d637d`.
The earlier core log SHA-256 is
`c70e38d429097ffb1cbf3051fccb632f722d120d877177076e282da80079c453`.
Temporary logs are inspection context, not promised repository artifacts. The
separate final review/evidence receipts must establish reproducible results after
all test-only repairs and the documentation correction.

## 6. Next steps and extension opportunities

1. Complete the independently owned final matrix, reference comparisons, and
   immutable artifact handoff. Do not replace the blocked later core run with the
   earlier passing core count.
2. Keep this audit supplemental: the author also implemented native Phase 3A.
3. When adding conditionals, introduce explicit value/condition types and lazy
   branch evaluation. The current eager numeric-call traversal must **not** be
   reused unchanged for piecewise branches containing inactive invalid expressions.
4. Consider a more declarative metadata record or exhaustive metadata matches for
   future builtin additions. Current arity/unit default arms are correct for the
   closed present enum, but a future variant must not accidentally inherit unary
   arity or dimensionless-input behavior. Reserved spellings must remain aligned.
5. Reuse the typed arena and independent generated-model comparisons for future
   editor/linter integrations and transparent interoperability demonstrations;
   none of this establishes biological validity or a new external standard.

## Key Learnings

1. Reviewing facade method comments separately from crate-level documentation
   catches stale API contracts even when the implementation is correct.
2. Explicit expression continuations preserve resource limits without relying on
   thread stack size; argument-ID arenas also avoid recursive drop hazards.
3. Unknown initial values, unknown dimensions, and runtime singularities are
   distinct concerns and must remain distinct in future conditional/rule work.
