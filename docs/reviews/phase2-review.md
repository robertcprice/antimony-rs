# Independent Phase 2 narrow-slice review

Reviewed 2026-10-09 America/New_York, after the Phase 1 gate. **PASS for the
implemented `numeric-flat-v0.1-slice` only.** This is not completion of every
Phase 2 grammar goal or the `kinetic-core-v0.1` release profile. Stop here; Phase 3
is not part of this verified increment.

## 1. What We Built (Plain English)

The library can now turn a small, explicitly initialized reaction model into
structured SBML using a native lexer, parser, semantic checker, and XML writer.
An independent review compared its output to pinned upstream output through
libSBML, tested rejected inputs and resource exhaustion, and verified that public
callers cannot construct or mutate invalid validated models.

Review found real gaps beyond the first seven happy-path fixtures. Workers
resolved them before this gate:

| Finding | Resolution and tracked regression |
|---|---|
| Reserved names such as `pi` became ordinary native parameters despite upstream rejection | Source-backed builtin inventory, rejected declarations/references; `reserved_constants_and_functions_are_not_ordinary_parameters` |
| `default_compartment=3` became a parameter instead of upstream's compartment | Explicit default-compartment identifier rejected in this slice; `reviewed_ambiguous_forms_fail_closed` |
| A newline incorrectly substituted for the semicolon between participants and rate | Lexer preserves the distinction; `reaction_requires_semicolon_and_unspaced_source_arrow_works` |
| `J:=>S;2` was lexed as an assignment rule instead of a source reaction | Arrow/colon disambiguation; same regression above |
| `--S` was accepted despite upstream rejection | Consecutive unary forms explicitly unsupported; frozen expression matrix |
| Signed/integral/scientific numeric leaves differed structurally | Narrow reference-backed numeric serialization, original lexemes retained; `reference_numeric_serialization_mapping_is_local_and_explicit` |
| `S*(P*S)` differed from upstream's reassociated tree | Reject right-nested same-operator addition/multiplication; no broad reassociation or algebraic normalization |
| Independent inspector rejected MathML scientific notation separator | Audited `sep` recognition preserves independently parsed AST type/value; `test_scientific_notation` |

The reserved inventory is grounded in the release-tag
[Registry::SetupFunctions/SetupConstants](https://github.com/sys-bio/antimony/blob/a4cb7549a03beef569ce21a0a7342e315ca0ed43/src/registry.cpp),
with runtime probes confirming representative names. The source tag is still not
proof of the wheel's build source.

## 2. Why It Matters (Impact & Implications)

This is a real native conversion path, not a Python-backed facade or a fixed XML
template. It preserves arrow direction, fixed stoichiometry, compartment sizes,
species concentrations, boundary/constant distinctions, modifiers and arithmetic
trees on the checked cases. Parsing and model semantics remain separate; export
requires privately constructed validated IR. Missing values never become zero.

The native library contains no filesystem/network/process invocation. File I/O
is explicit in the example adapter; reference Python remains development-only.
Source spans are UTF-8 checked. Expression arenas avoid recursive destruction,
export traversal is iterative, and both configured and hard depth guards apply.
All error paths are fail-fast, with one mandatory diagnostic, including when the
report budget is zero; warning counts are checked during analysis.

The parser/semantic/export modules are cohesive rather than a monolith. Read-only
model access prevents bypassing validation; constructing an independent public
`ExpressionKind` does not allow inserting it into a `ValidatedModel`. This is a
code review conclusion, not a formal proof of safety or a long-running fuzz audit.

### Actual acceptance boundary

One anonymous model or one `model name() ... end`; named reactions; explicit
numeric initial values; explicit or reaction-inferred species; initialized scalar
parameters; fixed positive compartments; source/sink and positive numeric
stoichiometry; `$`/`const species` boundary forms; bounded reaction arithmetic.
Species initialization is concentration only, not amount. Default volume-one
compartment behavior is reported, not silently invented.

**Explicitly deferred/rejected:** units, expression initializers, rules, events,
functions, imports/composition, arbitrary metadata, unnamed reactions, repeated
initialization or repeated species on one reaction side, builtins as symbols,
explicit `default_compartment`, empty-both-sided reactions, consecutive unary
operators, and right-nested same-operator `+`/`*` groups. Rate literals have at most
15 significant digits and are zero or magnitude `[1e-6, 1e15)`; numbers outside
that verified envelope are unsupported rather than silently rounded into parity.
No canonical Antimony formatter, numerical simulator, complete dimensional
analysis, full-profile support, or cross-platform certification exists. Unsupported
family coverage is incomplete: some metadata spellings currently fail through a
generic syntax diagnostic rather than a dedicated unsupported-feature diagnostic.
The event/rule/function and corpus rejection claims are specifically tested; do
not extrapolate them into complete unsupported-family classification.

## 3. Test Results (Verified, Not Assumed)

Independent final runs used macOS 15.5 arm64, Rust/Cargo 1.96.0, CPython 3.12.12,
Antimony 3.2.0 and independent python-libsbml 5.21.2. Native executable SHA-256:
`c43d03c2581cb7ebef5a8a2ebf3e30e39fa69ce0a26275a5bd6ea7fe3d40b138`.

| Exact command | Observed result |
|---|---|
| `cargo fmt --all --check` | Pass |
| `cargo check --all-targets --all-features` | Pass |
| `cargo build --all-targets --all-features` | Pass |
| `cargo test --all-targets --all-features` | **55 integration passed**: 22 foundation + 33 slice; 0 unit/example tests; no failures/ignored |
| `cargo test --doc --all-features` | **1 passed** |
| `cargo test --no-default-features` | **52 integration + 1 doctest passed**; 3 export-only tests correctly not compiled |
| `cargo clippy --all-targets --all-features -- -D warnings` | Pass |
| `RUSTDOCFLAGS='-D warnings' cargo doc --no-deps --all-features` | Pass |
| `cargo build --offline --all-targets --all-features` | Pass using already-fetched dependencies |
| `.venv-oracle/bin/python -m unittest discover -s tools/oracle -p 'test_*.py' -v` | **43 passed**: 34 harness/contract/checker + 9 live reference; no failures/skips |
| `.venv-oracle/bin/python tools/oracle/corpus.py check` | **13 reference replays passed** |
| `.venv-oracle/bin/python tools/oracle/native_check.py --native target/debug/examples/export_sbml --trusted-native` | **8 exact structural export matches + 5 expected rejections**, 0 failed/blocked; **13 numerical NOT_RUN** |
| Same native-check command with `--python /no/such/python` | Expected exit 2; 13 blocked, 0 exported or expected-rejection passes |
| `cargo tree --all-features` / `cargo tree --no-default-features` | Optional quick-xml 0.42.0 → memchr 2.8.3; no dependencies without default features |
| `cargo package --list --allow-dirty` | Pass; 69 entries before this review, no caches/virtualenv/artifacts; final inventory after review: 70 entries |

No-default tests repeat native coverage, not extra unique cases. The frozen
additional differential suite produced **16 accepted exact matches + 3 explicit
rejections across 19 expressions**, and **14 accepted exact matches + 6 explicit
rejections across 20 models**. There were **zero accepted mismatches after fixes**.
Initial defects and differences above were observed before fixes, not hidden.

These 39 probes are reproducible from tracked test matrices
`frozen_nineteen_expression_probe_acceptance_regression` and
`frozen_twenty_model_probe_acceptance_regression` in `tests/vertical_slice.rs`.
They preserve every exact expression/model and expected native acceptance. For
independent comparison, the following script extracts those frozen literals,
runs the prebuilt native executable, and checks accepted projections without
rewriting expressions. It deliberately leaves supported-profile rejection distinct
from upstream validity:

```sh
.venv-oracle/bin/python - <<'PY'
import ast, re, sys
from pathlib import Path
sys.path.insert(0, 'tools/oracle')
import native_check, oracle
source = Path('tests/vertical_slice.rs').read_text()
exe = Path('target/debug/examples/export_sbml').resolve()
for name, expression in [('nineteen_expression', True), ('twenty_model', False)]:
    body = source.split('fn frozen_' + name + '_probe_acceptance_regression()', 1)[1]
    body = body.split('];', 1)[0]
    pairs = re.findall(r'\("((?:\\.|[^"\\])*)", (true|false)\)', body)
    matched = rejected = 0
    for quoted, expected in pairs:
        text = ast.literal_eval('"' + quoted + '"')
        if expression:
            text = f'S=2;P=1;J:S=>P;{text};'
        native = native_check.native_export(exe, text)
        if expected == 'false':
            assert native['status'] == 'REJECTED', (name, text, native)
            rejected += 1
            continue
        reference = oracle.run('convert', text)
        assert native['status'] == 'EXPORTED', (name, text, native)
        checked = oracle.run('inspect', native['xml'])
        assert reference['status'] == checked['status'] == 'PASS', (text, reference, checked)
        assert not native_check.differences(reference['projection'], checked['projection']), text
        matched += 1
    print(name, len(pairs), 'cases:', matched, 'exact matches;', rejected, 'rejections')
PY
```

The unit resource tests exercise source/token/entity/diagnostic/depth limits,
5,000 nested parentheses, 5,000-node left chains, raised-config hard depth, and
**1,728 three-character adversarial inputs**. Additional independent CLI probes
verified a 4 MiB + 1 byte input and 5,000 nested parentheses return
`resource_limit`, and an import returns `unsupported_feature`, each without an
output artifact. Import safety also follows from absence of native I/O paths;
this is not an OS syscall-tracing claim.

The sample README command was executed and independently inspected: L3V2,
0 independent errors, **12 retained warnings**. Across eight differential exports,
**93 retained warnings** were recorded, not suppressed. Unit gaps are expected
and disclosed. Packaging still warns about pending project license/repository
metadata; nothing was published. No numerical, alternative-toolchain/OS, or
long-running fuzz gate was performed.

## 4. What's Next (Concrete Next Steps)

Stop at this verified slice. Next authorized work should select one deferred
feature family, add reference fixtures first, preserve private validation and
limits, then rerun independent projections and negative cases. In particular,
initial-expression dependencies, units, formatter, broader names/grouping/numeric
forms, and full-profile grammar remain substantial work. Obtain maintainer
agreement on the oracle target, license and API before any release claim.

## 5. Novel Opportunities (Innovation & Monetization)

The useful differentiator is a compact native compiler with auditable, stage-by-
stage compatibility evidence and explicit rejection rather than silent semantic
loss. That could support future upstream review and reliable Rust integration.
No official endorsement, speed superiority, scientific validation, publication or
commercialization approval follows from this gate.

## Key Learnings:

1. Seven successful models did not expose reserved-name and separator mismatches;
   bounded independent probes found and closed concrete acceptance gaps.
2. Numeric representation can be mapped locally with reference evidence; changing
   arbitrary expression association to make tests pass is not acceptable.
3. A passing rejection test is not evidence of native support for that feature.
