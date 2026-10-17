# Flexible flat-model milestone (and Phases 6–8 modular/event extension)

Date: 2026-10-15. **Phases 1–5 (`functions-core-v0.5-slice`), the 36-model [independent collaboration gate](COLLABORATION_VERIFICATION.md), and Phases 6–8 (`modular-core-v0.8-slice`: `substanceOnly` species, dynamic compartments, reaction kinetic-law overrides, discrete `at` events, and hierarchical modules/submodels/ports/`M.x`/`is` flattening) are implemented and independently verified within their bounded scope; see [MODULAR_AND_EVENTS.md](MODULAR_AND_EVENTS.md), [FUNCTIONS.md](FUNCTIONS.md), [RULES.md](RULES.md), [CONDITIONALS.md](CONDITIONALS.md), and [RATIONAL_CALLS_EVIDENCE.md](RATIONAL_CALLS_EVIDENCE.md).**
This responds to the request that Herbert be able to write and change his own
models rather than follow a curated demo. It supplements, not retroactively
changes, [the implementation plan](IMPLEMENTATION_PLAN.md) and frozen profiles.

## Finding and recommendation

The native library and CLI already accept arbitrary source strings, files and
stdin. They do not check fixture identities. Frozen inventories belong to the
verification harness. The obstacle is the deliberately narrow language slice.
Keep resource limits, diagnostics, validation-before-export and filesystem safety;
expand actual language semantics instead of bypassing checks.

Build one coherent **general flat-model subset** before calling this flexible
independent testing. Priorities are (1) expressions/numbers, (2) source-aware
symbols and ordinary declarations/reactions, (3) builtins/time, (4) assignment and
rate rules, then (5) reusable user functions. Unset initialization belongs in the
symbol foundation. Conditions/piecewise share the expression foundation and are
valuable for time-varying experiments. This is not full Antimony.

## Phase 0 — discovery and observed evidence

Read [SOURCES.md](SOURCES.md), [DESIGN.md](DESIGN.md),
[PROFILE_V0_1.md](PROFILE_V0_1.md) and [VALIDATION.md](VALIDATION.md).
The source-discovery commit and the installed reference runtime are separate
baselines; do not assert they are the same artifact. These probes used installed
Antimony 3.2.0 and libSBML 5.21.2.

Three read-only reviewers contributed: declaration/rule and expression agents,
plus Agy Argon (`gemini-4-argon-medium`, planning mode). Root reconciled their
advice against live probes. Review outputs are local scratch, not release fixtures:

- `artifacts/flexibility-audit/declarations/results.json`: 37 cases.
- `artifacts/flexibility-audit/declarations-extra/results.json`: 12 cases.
- `artifacts/flexibility-audit/expressions/probes.json`: 26 cases.
- `artifacts/flexibility-audit/argon-review.txt`: advisory, not authority.

**75 deliberately gap-targeted original probes:** 67 accepted by the reference,
8 reference rejections; native accepted 3 and rejected 72. This is NOT a random
corpus or an estimate of compatibility percentage. Declaration probes: 46/49
reference acceptance, 2 native controls accepted. Expression probes: 21/26
reference acceptance, 1 native accepted. Nine accepted expression cases exceed
the current projection vocabulary; they are NOT upstream language rejections.

One native-accepted expression, `S+(-(-P))`, retains nested unary operators while
reference output simplifies to `S+P`. This is an observed exact-structure mismatch,
not demonstrated numerical disagreement. Normalization needs an explicit policy.

Fresh native build and tests during this audit: **257 unit/integration tests and
2 doctests passed; zero failures/ignored tests**. Logs are
`artifacts/flexibility-audit/{build,tests,doctests}.log`. Existing registry cache
was used offline with a new audit target directory; not a clean-machine claim.
No Rust source, Cargo manifest, frozen profile or original fixture was changed.

### Existing APIs and patterns to reuse

- `src/lib.rs`: `parse(&SourceFile, &Limits) -> Result<ParsedDocument, Diagnostic>`;
  `analyze(&ParsedDocument) -> Result<ValidatedModel, Diagnostic>`.
- `src/format/antimony.rs::format_antimony`: validated, re-parsed, idempotent layout.
- `src/sbml.rs::write_sbml`: validated-model export gate; `math` and `number` emitters.
- `src/syntax/parser.rs::expression`: bounded Pratt parser and expression arena.
- `src/semantic/initialization.rs::{resolve,evaluate}`: iterative dependency work.
- `src/model.rs`: private verified fields and public read-only queries.
- Tests: `tests/{initialization,flat_syntax,flat_syntax_review,formatting,format_review,hardening,vertical_slice}.rs`.
- `tools/oracle/oracle.py::{run,ast}`: subprocess reference conversion and projection.

New IR types below are design requirements, **not existing API names**.

## Phase 1 — general expressions and faithful numbers

**Delivered increment:** `arithmetic-v0.1-slice`; see the
[arithmetic review](reviews/arithmetic-review.md). The dedicated exact gate covers
54 original cases (47 positive, 7 negative), including deeper group context and
cancellation/underflow initialization regressions found independently. The
original 75-gap audit below/above remains historical, not a current rejection score.
The retained full source receipt predates this increment. New structural checks
do not imply new numerical trajectories or delivery of later phases.

The implementation preserves source layout and builds reference-normalized
semantic groups before lowering to binary IR shared by analysis and export.
Scientific literals use exact decimal mantissa/exponent spelling within the
15-digit, zero-or-[1e-307,1e308] contract. Excess precision remains unsupported.
The oracle now preserves user-function call identity, without adding function
XML vocabulary or native function support.

### Original implementation requirements

**Implement:** retain source lexemes/grouping for formatting; represent numeric
forms and operators sufficiently to export small/large scientific notation and
ordinary nested arithmetic without current convenience restrictions. Specify
semantic normalization separately from source layout. Verify valid unary forms
rather than accepting every sign sequence: reference rejects `--S` in our probe.

**References:** parser `expression`, `src/syntax/numeric.rs`, model expression
arena, SBML `math`/`number`; copy existing boundary/precedence/initializer test
patterns from `tests/initialization.rs` and `tests/hardening.rs`.

**Verify:** nested sums/products, mixed signs, precedence, exponent boundaries,
underflow/overflow, roundtrip idempotence, preserved literal meanings and
independent MathML validation. Add a regression for the observed unary mismatch.
Distinguish exact reference-normalized structure from numerical checks; do not
silently loosen all existing exact comparisons.

**Guards:** no deleting numeric guards without a representation policy; no
arbitrary reassociation just to pass tests; keep node/depth/byte ceilings.

## Phase 2 — source-aware symbols, declarations and unnamed reactions

**Delivered increment:** `flat-model-v0.2-slice`; [scope and verification guide](SYMBOLS.md).
The implementation intentionally keeps computed compartment sizes and
reaction-valued expressions deferred. The requirements below are the original
plan, not a claim that every possible symbol interaction is supported.

**Implement:** cohesive symbol-resolution module tracking occurrence order,
explicit/inferred role, qualifiers, provenance and initialization state. Support
compatible redeclarations, formula-to-species promotion, const/var declarations,
compartment lists/deferred sizes and optional reaction labels. Separate unset,
scalar and expression initialization. Preserve symbolic initial assignments when
they cannot yet be evaluated; export absent values as absent, never zero.

**References:** `src/semantic.rs::analyze_document` and `required_initial`,
`src/syntax/mod.rs::Statement`, parser `statement`/`participants`, initialization
resolver, `src/model.rs` concrete-f64 getters; existing flat-syntax tests.
The formatter currently recognizes a reaction's internal semicolon by a colon
(`src/format/antimony.rs`); replace that assumption using parsed statement shape.

**Verify:** named/unnamed mixtures; empty source/sink sides; source-order ID
collisions; later reaction reassignment; compatible redeclarations; qualifier
order; uninitialized symbols and compartments; symbolic initializer dependencies;
format/check/export of unseen sources. Preserve distinctions between explicit
unset compartment size and implicit default compartment size 1.

Observed upstream IDs are `_J0`, `_J1`, etc. An earlier `_J0=4` reserves a name;
a later `_J0=4` updates the generated reaction law. Pre-reserving all future
identifiers is incorrect. Species const/var order is significant.

**Guards:** don't preserve incompatible f64 getter guarantees by returning fake
values. Design any API change explicitly. Separate structural validity from
simulation-ready initialization; this crate does not become a solver.

## Phase 3 — shared function-call, time and condition expressions

**Delivered Phase 3A:** `math-core-v0.2-slice`; see the [verified builtin guide](BUILTINS.md)
and [independent review](reviews/builtins-review.md). Typed numeric functions, constants
and time are implemented. The later `math-core-v0.2.1-slice` corrects rational-token
recognition and exact rational-atom structure; see [evidence](RATIONAL_CALLS_EVIDENCE.md).
Conditions, booleans and piecewise below remain Phase 3B
requirements, not support claims. The parser uses a bounded iterative task stack.

**Implement:** typed builtin/time/constant/call nodes, one verified builtin table
with spelling, arity, MathML mapping and dimensional behavior. Start with exp,
logs, roots, trig, abs, powers and time; extend explicit comparisons/boolean
operators and piecewise through the same expression infrastructure. Probe each
additional builtin before declaring support; the list is a plan, not coverage.

**References:** `src/syntax/reserved.rs`, expression arena, initialization evaluator,
unit-analysis walkers and SBML `math`; expression probes for exp, roots, logs,
pow, time and piecewise. In the installed reference `log` maps to ln, whereas
`log10` uses base-10 log. Do not infer these mappings from other languages.

**Verify:** arity, nested calls, undefined names, domain behavior, time in rates
and initial expressions, typed conditions, dimension knowledge and unknown-unit
propagation. Prove lazy branch behavior where required; do not eagerly evaluate
an inactive invalid branch. Reference accepts `a=time+1`; investigate its exact
initial-state semantics rather than banning time there by assumption.

**Guards:** widen oracle MathML tags only alongside preserved semantics and
negative controls. The prior projection omitted user-function call names
(`isFunction` true, `isName` false); this was fixed in Phase 1 before any function-conformance claim.
Different function names with identical arguments must not compare equal.

## Phase 4 — genuine assignment and rate rules

**Implement:** distinct syntax/IR collections for initial assignments, continuously
assigned values (`:=`) and rate equations (`'=`). Resolve target ownership,
mutability, dependencies and conflicts explicitly; emit SBML listOfRules.
Rate dimensions are target units divided by time. Preserve unset initial values
if structurally valid; report readiness separately rather than guessing them.

**References:** lexer unsupported tokens, `Statement`, `ValidatedModel`, semantic
initialization/dependency checks, `src/sbml.rs::emit`; original rule probes.

**Verify:** assignment chains/cycles, duplicate/conflicting targets, initial/rule
ordering, rule-controlled parameters/species/compartments and reaction overlap.
In observed reference output, reaction-participating rule targets become boundary
species; initialization after assignment rule rejects, whereas the reverse order
can replace initialization. Add analytic trajectory checks for initialized rule
models using development-only independent solvers, not a core dependency.

**Guards:** never lower a runtime rule into an InitialAssignment. No double state
ownership. Rules are excluded from the frozen kinetic-core-v0.1 target: introduce
an explicitly versioned broader target rather than rewriting history silently.

## Phase 5 — reusable user functions

**Implement:** top-level definitions, local argument scope, call resolution/arity,
reference-compatible definition ordering, recursion diagnostics and MathML
function definitions, using Phase 3 call nodes. Add bounded function/dependency
limits without executing user code.

**References:** expression function probes; source tutorial/grammar in SOURCES;
public model queries and SBML emitter patterns. Scope probes reject global/time
capture, recursion and the tested forward definition; don't assume C/Rust rules.

**Verify:** user-authored Hill/Michaelis-Menten helpers at multiple call sites,
parameter shadowing, unknown/recursive calls, source roundtrips, preserved call
identity and numerical equivalence to equivalent expanded formulas.

**Guards:** no raw-string expression escape hatch or callable host functions.
Function support also requires a broader versioned target contract.

## Final verification and collaboration gate

For each phase: update implementation identity and accurate limitations; add
regressions; rerun all-feature and core-only quality gates, existing oracle,
formatter and CLI gates. Retain old negative expectations as historical records;
new profiles must explicitly account for intentionally newly accepted cases.

For the milestone: another reviewer authors models without consulting fixture
IDs, edits parameters/species/rates, combines features and calls the ordinary
library/CLI. Require format idempotence, independent SBML validation, reference
semantic comparisons and selected dynamics where a complete initial state exists.
Keep invalid, unsupported, resource-limited, incomplete and oracle-blocked outcomes
distinct. Not every structurally valid model is ready to simulate.

Suggested review exercises: unnamed conversion chain; reusable saturating rate;
small-rate-constant model; time-dependent input; derived assignment variable;
rate-controlled state; piecewise stimulus; incomplete model later filled in.
These are examples of behaviors, not a whitelist. Include negative near-neighbors.

Stop before modules/imports, hierarchical composition, events/delays, full unit
algebra, SBML import, a simulator, GUI or a replacement external standard. Revisit
those with Herbert. Amount initialization/substance-only species are important
follow-on work but need a separate dimensional/initial-state contract.

## Effort and sequencing

Planning estimate, not measured implementation time: phases 1–2 roughly **20–35
engineering hours**; phases 3–4 another **25–45 hours**; functions plus broader
combined verification **15–30 hours**. Total **60–110 hours** including review,
with uncertainty dominated by reference normalization, symbol order and rules.
Parallel agents can explore independent expression/symbol branches, but model IR,
export and integration need one owner; agent speed is not validation evidence.

After phases 1–4 pass the open-ended gate, Herbert can usefully test a substantially
more general flat subset. Functions are the next composability upgrade, not a
reason to postpone all feedback. Invite review as an experimental implementation,
not the official Rust standard. No collaborator messages or publication are
performed by this plan.
