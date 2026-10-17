# Independent unified CLI review

**Status: PASS for the optional unified CLI over the unchanged bounded language subset.**
This review is separate from the completed flat-declaration language increment.

## 1. What We Built (Plain English)

Built one optional `antimony` command that checks models, formats accepted source
and exports SBML through the existing library. It adds one-based source positions,
a closed JSON report, separated artifact/diagnostic streams and staged file output.
The language subset and 90 original fixture contracts did not change. User-facing
usage and limitations are in [the CLI guide](../CLI.md).

## 2. Why It Matters (Impact & Implications)

Users and editor/automation integrations should be able to invoke one predictable
interface without copying language logic or parsing human error messages. The CLI
must not expand or weaken the language profile merely to improve usability.

## 3. Test Results (Verified, Not Assumed)

Completed harness-development checks:

- `.venv-oracle/bin/python -m unittest discover -s tools/oracle -p test_cli_check.py -v`: **23 passed**, 0 failed/skipped.
- `.venv-oracle/bin/python -m unittest discover -s tools/oracle -p 'test_*.py' -v`: **143 passed**, 0 failed/skipped (120 inherited + 23 new checker tests).
- Python byte compilation passed for the two new checker files.

The stdlib checker implements only the closed JSON-Schema vocabulary used by
`cli-report.schema.json`, not general JSON-Schema validation. Unit regressions
exercise wrong field/type/status, duplicate JSON keys, original UTF-8 positions,
missing tools, artifacts on rejection, example-byte mismatches, semantic drift,
warning drift, non-idempotence and changed-input detection.

Independent source review found the adapter delegates to existing compiler APIs,
checks unavailable export before input I/O, recovers JSON mode without interpreting
literal path operands as options, escapes human terminal-control text and uses
fallible output writes. These are inspected properties, not substitutes for tests.

The new CLI live corpus gate passed **48 supported cases + 42 required
rejections** with zero failures/blocked cases. Each supported case passed check,
format and export, exact artifact-byte agreement with existing examples,
formatter idempotence, independent SBML inspection and live reviewed-reference
semantic comparison. Each rejection passed the expected stage/code contract for
all three commands with no artifact. These are 90 cases, not 270 distinct models.

- `cli_check.py --cli target/debug/antimony --native target/debug/examples/export_sbml --formatter target/debug/examples/format_antimony --trusted-native`: **PASS**, unchanged binary/manifest/profile/schema/checker inputs.
- The same command with unavailable CLI: **90 blocked**, no success credit.
- The same command with `--python /no/such/python`: **90 ORACLE_BLOCKED**, no success credit (aggregate expected failure).
- Retained **307 independent libSBML warnings** on CLI export and another 307 on
  formatted-reference output. Separately, **48 core-library warnings per command**
  match the existing example warning sequence exactly; these are different warning systems.

### Independent native engineering gate

The native reviewer used `CARGO_TARGET_DIR=target/independent-cli-review`; no
standard-target binary was rebuilt during the oracle checks.

| Feature configuration | Binary unit tests | Integration tests | Doctests |
|---|---:|---:|---:|
| `--all-features` | 13 | 244 | 2 |
| default features | 0 | 212 | 2 |
| `--no-default-features` | 0 | 205 | 2 |
| `--no-default-features --features cli` | 13 | 237 | 2 |

Every cell passed, with **zero failures/ignored tests**. The CLI-bearing modes
include 15 implementation and 17 independent CLI/I/O integration tests. Repeated
feature configurations are not additional unique test cases.

For each configuration the reviewer executed `cargo check --locked --all-targets`,
`cargo build --locked --all-targets`, `cargo test --locked --all-targets`,
`cargo test --locked --doc`, `cargo clippy --locked --all-targets -- -D warnings`,
`RUSTDOCFLAGS='-D warnings' cargo doc --locked --no-deps`, and
`cargo build --locked --offline --all-targets`, with the corresponding feature
flags before `--`. All passed. `cargo fmt --all --check` passed. The no-default
normal dependency tree is empty; optional JSON support did not leak into core.

The tests cover staged-write errors/cleanup, preserving existing output,
permissions, symlinks, hardlinks, in-place paths, size-before-UTF8 classification,
stdin/stdout and broken pipes. Closing the JSON report pipe after artifact commit
returns exit 1 while leaving the committed artifact in place; it does not pretend
to roll back. A broken diagnostic stderr pipe also returns 1 instead of panicking.
The local macOS filesystem rejected invalid-UTF8 filename creation (OS error 92);
this is an environmental boundary, not proof that such filenames worked. Linux
and Windows execution and all six remote CI jobs remain **NOT_RUN**.

Four additional bounded process probes passed: FIFO input and FIFO output were
rejected within a three-second timeout, a nonwritable parent directory prevented
output creation, and an unreadable input was rejected. These are not additional
Rust test methods.

### Current example revalidation and evidence identity

All-feature builds changed the example executable identities even though their
protocols and the core semantic source were unchanged. The current binaries were
therefore rerun, not relabeled using old results:

- `tools/oracle/native_check.py --native target/debug/examples/export_sbml --trusted-native`: **48 exact supported exports + 42 required rejections**, zero failures/blocked.
- `tools/oracle/roundtrip_check.py --formatter target/debug/examples/format_antimony --native target/debug/examples/export_sbml --trusted-native`: **48 byte-idempotent exact native/reference roundtrips + 42 formatter rejections**, zero failures/blocked.
- `tools/oracle/corpus.py check`: **90 reviewed reference replays passed**.

Each command used `.venv-oracle/bin/python`. Each original/formatted native path
retains 307 libSBML warnings. Corpus sources, manifest and goldens were unchanged.
All **90 numerical stages remain NOT_RUN**.

Final standard-target SHA-256 identities:

- CLI: `188be804f78a342e8d005ecfba17a3fc072733434e389bce412ee9c459781e3a`
- Export example: `a7b40bb9b320cdcea3f44ccea5cb8b0effb601e2901d459b65cc67ebdfce3dec`
- Format example: `ab7025ecc1887bdc0e2d950e9f8c232df368b96efb82ce29eef2b8eb02dad1c2`
- Fixture manifest: `d02e761c5fb109b3eff5a575b2dee3fe9b2651c9398db6c3ffe1ecf633277717`

The CLI gate also records unchanged profile/report-schema/checker hashes.
`tools/oracle/cli-evidence.json` retains the new compact 90-case report; existing
native/formatter evidence summaries now record their actual current-binary reruns.
Historical reviews keep their historical executable identities. Full process
reports are under ignored `artifacts/cli-*-review.*`.

Final documentation QA independently checked **61 local Markdown links/anchors**
with zero errors. `cargo package --list --allow-dirty` listed **273 entries**,
with no cache, virtual-environment, target or artifact paths; only the expected
unset package-metadata warning remained. Three README JSON check/format/export
smokes passed without rebuilding or changing the CLI executable.

The schema vocabulary was checked against the official
[JSON Schema 2020-12 Core](https://json-schema.org/draft/2020-12/json-schema-core)
and [Validation](https://json-schema.org/draft/2020-12/json-schema-validation)
documents. The local checker remains deliberately bounded and additionally
requires the CLI's emitted integer wire representations; it is not a general
Draft 2020-12 conformance implementation.

## 4. What's Next (Concrete Next Steps)

This adapter increment is complete. Next prioritize separately scoped analytic
numerical anchors and representative/differential models. Execute remote platform
jobs only with authorization; do not infer cross-platform success from a configured
matrix. Preserve the same report schema or explicitly version incompatible changes.
The inherited quoted-import diagnostic classification limitation remains deferred.
Run `graphify update .` if maintaining an architecture graph; no graph refresh was
performed by this review.

## 5. Novel Opportunities (Innovation & Monetization)

A stable machine-readable native interface enables lightweight editor and model
preprocessing integrations. Reliability and explicit limitations—not full-language
or simulation claims—are the useful differentiator. No publishing action is implied.

## Key Learnings:

1. Human diagnostics and raw artifacts must use separate streams.
2. JSON success must describe a completed operation, not merely successful parsing.
3. Optional CLI dependencies must not leak into no-default core builds.
