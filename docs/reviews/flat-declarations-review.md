# Independent bounded flat-declaration review

Reviewed 2026-10-10 UTC. **Gate: PASS for `flat-declarations-v0.1-slice`.**
This increment extends the previously reviewed bounded subset, not full Antimony
or the complete kinetic-core-v0.1 target. Historical reviews remain historical.

## 1. What We Built (Plain English)

Common flat-model source can now omit empty parentheses after a model name and
combine ordinary parameter declarations in a comma-separated `formula` statement.
An isolated bare identifier now receives a syntax diagnostic instead of being
misidentified as a valid unsupported feature. Parsing, checking, canonical
formatting and SBML export agree on these narrow additions.

Independent evidence adds 17 original live-reference fixtures and ten reference
contract tests. Eight fixtures require successful translation; nine require
specific rejection categories. No earlier fixture source or golden was regenerated,
no comparison tolerance was introduced, and no unsupported rule was flattened
into an initial value.

### Reviewed boundary

- Named headers allow `model name` or `model name()` followed by a newline or
  semicolon. Nonempty model interfaces remain unsupported; same-line bodies with
  no header separator remain outside this bounded slice.
- Unqualified `formula a=1,b=a+2;`, declarations followed by later initialization,
  and mixed initialized/uninitialized declarations are accepted when every final
  parameter value is valid. EOF termination is accepted. Comma followed by newline
  or a trailing comma is malformed, not a continuation syntax.
- Each initializer keeps its original scalar-versus-InitialAssignment semantics:
  bare negative literals remain scalar; parentheses/unary plus remain expressions.
  Forward references and first-occurrence assignment ordering are retained.
- Dependencies on ordinary species propagate `constant=false` through parameter
  chains; boundary-species dependencies retain `constant=true`. No new semantic
  evaluator or parallel parameter authority was introduced.
- `formula` is the verified keyword. **`parameter` is not an Antimony 3.2.0
  declaration keyword**; this increment does not invent that alias.
- `const formula`, `var formula`, dollar-qualified formula entries, formula rules,
  functions and broader declaration syntax remain unsupported. Formula symbols
  promoted to species or compartments remain outside this parameter-only slice.
  Missing values, duplicate initialization and type promotion retain strict native
  rejection even when full upstream Antimony accepts them.
- Bare identifiers at statement boundaries now fail with Syntax. This is not a
  comprehensive malformed-versus-unsupported audit. The existing lexer can still
  classify `import "secret.ant"` as Syntax at the dot before the parser recognizes
  the unsupported import; `import "secret"` reaches UnsupportedFeature. Neither
  follows the import or accesses that file. This pre-existing limitation is deferred.
- The formatter preserves source tokens, statement order and explicit header
  parentheses rather than rewriting all equivalent headers to one spelling.
  Existing numeric, unit, resource and no-partial-output restrictions are unchanged.

Reference discovery used the pinned official [module/formulainit grammar](https://github.com/sys-bio/antimony/blob/a4cb7549a03beef569ce21a0a7342e315ca0ed43/src/antimony.ypp)
(module productions around 172–183 and declaration productions around 576–579)
and [tutorial model examples](https://github.com/sys-bio/antimony/blob/a4cb7549a03beef569ce21a0a7342e315ca0ed43/doc/AntimonyTutorial.md#introduction-and-basics).
Goldens came from actual pinned Antimony 3.2.0 execution, not source inference.
The release-tag-to-wheel build mapping remains unattested.

## 2. Why It Matters (Impact & Implications)

This removes avoidable rejection of common flat authoring syntax without adding
simulation, widening numerical assumptions or diluting existing exact semantic
checks. Users can write concise parameter lists and familiar model headers while
receiving honest errors for incomplete or excluded constructs.

The evidence distinguishes 48 supported models from 42 rejection tests. These are
not 90 successfully translated models, and overlapping native, reference and
formatter suites are not a compatibility percentage.

## 3. Test Results (Verified, Not Assumed)

Local Rust/Cargo 1.96.0, CPython 3.12.12, macOS 15.5 arm64; reference Antimony
3.2.0 and independently installed libSBML 5.21.2. Two independent reviewers
executed complementary gates: the native reviewer used an isolated Cargo target;
the reference reviewer checked the stable standard-target executables.

### Native reviewer execution

All commands below used `CARGO_TARGET_DIR=target/independent-flat-review` to avoid
changing binaries during the oracle checks.

| Command | Observed result |
|---|---|
| `cargo fmt --all --check` | Pass |
| `cargo check --locked --all-targets --all-features` | Pass |
| `cargo build --locked --all-targets --all-features` | Pass |
| `cargo test --locked --all-targets --all-features` | **212 integration passed**, 0 failed/ignored; inherited 185 + 14 implementation + 13 independent review tests |
| `cargo test --locked --doc --all-features` | **2 passed** |
| `cargo test --locked --no-default-features` | **205 integration + 2 doctests passed**, 0 failed/ignored |
| `cargo clippy --locked --all-targets --all-features -- -D warnings` | Pass |
| `cargo clippy --locked --all-targets --no-default-features -- -D warnings` | Pass |
| `RUSTDOCFLAGS='-D warnings' cargo doc --locked --no-deps --all-features` | Pass |
| `RUSTDOCFLAGS='-D warnings' cargo doc --locked --no-deps --no-default-features` | Pass |
| `cargo build --locked --offline --no-default-features --example format_antimony` | Pass |
| `cargo build --locked --offline --all-targets --all-features` | Pass using cached dependencies |
| `cargo package --list --allow-dirty` | Pass; 258-entry native-review snapshot; final coordinated inventory **259 entries**, no caches/venv/target/artifacts; expected unset-metadata warning |

Six additional CLI probes passed: supported header/list formatting, syntax
rejection preserving existing output, missing-initializer rejection without output,
bare-identifier export diagnostic, in-place formatting on a Unicode path, and
output-I/O failure cleanup. They are not additional Rust test counts.

### Reference reviewer execution

Each Python command below used `.venv-oracle/bin/python`.

| Arguments | Observed result |
|---|---|
| `-m unittest discover -s tools/oracle -p 'test_*.py' -v` | **120 passed**, 0 failed/skipped |
| `tools/oracle/corpus.py check` | **90 reviewed-reference replays passed** |
| `tools/oracle/native_check.py --native target/debug/examples/export_sbml --trusted-native` | **48 exact exports + 42 required rejections**, 0 failed/blocked |
| `tools/oracle/roundtrip_check.py --formatter target/debug/examples/format_antimony --native target/debug/examples/export_sbml --trusted-native` | **48 byte-idempotent exact native/reference roundtrips + 42 required formatter rejections**, 0 failed/blocked |
| Same roundtrip command with `--python /no/such/python` | Expected exit 2: **90 ORACLE_BLOCKED**, 0 supported/rejection success credit |
| `-m py_compile tools/oracle/*.py` | Pass |

The 48 original native exports, 48 formatted native exports and 48 formatted
reference exports each retain **307 libSBML warnings**. These are preserved, not
suppressed or presented as complete dimensional validation. Two inherited
invalid-reference fixtures can earn rejection credit only. All **90 numerical
stages remain NOT_RUN**; all six remote CI jobs remain **NOT_RUN**.

Stable executable and corpus identities:

- Exporter SHA-256: `fb390d10ffe0fe9667a4739bd3d68ffc0d3cc4b1eb71d2156410b8771d910c42`
- Formatter SHA-256: `2b2b9e5d3cc0f7afeec47e0f700274e5c05f00a74ca72f9d3f99ac1578e9199c`
- Manifest SHA-256: `d02e761c5fb109b3eff5a575b2dee3fe9b2651c9398db6c3ffe1ecf633277717`

Final examples were rebuilt after the last Rust documentation edit; both binary
hashes changed. The full native and roundtrip gates were rerun successfully on
these final artifacts. Root independently reconciled all 90 per-case summaries
against each complete report and checked current executable/checker/manifest
hashes before refreshing this evidence. Both binaries and the manifest remained
unchanged during the final roundtrip gate.
`tools/oracle/native-evidence.json` and `tools/oracle/formatter-evidence.json`
record current per-case results; complete process reports remain under ignored
`artifacts/flat-*-review.*`. This is binary-specific evidence, not a clean-checkout
reproducibility claim.

An initial shell wrapper attempted to assign zsh's read-only `status` variable
after completed checks; that orchestration error was corrected and subsequent
commands were executed explicitly. It was not a test failure or a passing gate.

## 4. What's Next (Concrete Next Steps)

Keep this increment bounded. Separately scope malformed/unsupported diagnostics,
including the quoted-import lexer issue; selected numerical comparisons with
analytic anchors; structured differential generation; and cross-platform CI
execution. Representative user models should guide further grammar additions.
Do not infer unnamed reactions, rules, events, functions or broad formula-type
promotion support from this change. `graphify update .` can refresh any future
architecture graph; no graph refresh was run here.

## 5. Novel Opportunities (Innovation & Monetization)

An auditable native preprocessing library can support editor checks, reproducible
model generation and lightweight scientific tooling. The differentiator is a small,
explicitly verified compatibility boundary—not an unsupported claim of full
Antimony compatibility. No publication, remote creation, licensing decision or
commercial action occurred.

## Key Learnings:

1. SBML parameter concepts do not imply a `parameter` source-language keyword.
2. Upstream `formula` names can later become species; this native increment deliberately remains parameter-only.
3. Concise syntax must preserve first-occurrence ordering and scalar-versus-expression initialization modes.
4. Required rejection success must remain separate from supported-model success.
