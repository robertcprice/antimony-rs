# Independent structural workload review

**Status: independent 86-case structural gate, 51 workload unit tests and the
39-command observed current-source receipt PASS.** No numerical execution is
claimed for these workloads.

## 1. What We Built (Plain English)

A shared gate tests six original model motifs and 80 deterministic structured
cases against independent structural contracts, the pinned upstream converter,
native export and canonical formatting. It does not broaden the grammar or
change the existing 90 language cases or eight numerical anchors.

## 2. Why It Matters (Impact & Implications)

The new cases combine supported features while refusing to treat arbitrary
upstream output as truth. Separate expected structures catch common-mode bugs;
explicit negative contracts prevent any rejection from being called correct.

## 3. Test Results (Verified, Not Assumed)

The owner and independent reviewer each actually ran:

```sh
python3 -m unittest discover -s tools/workloads -p 'test_*.py' -v
.venv-oracle/bin/python tools/workloads/check.py \
  --native target/debug/examples/export_sbml \
  --formatter target/debug/examples/format_antimony --trusted-native
```

Both runs passed **51 tests** (12 generator, 21 checker, 18 motif), zero failures
or skips, and **86 live cases**: six original supported motifs, 64 supported
generated cases and 16 required rejections. These are three separate categories,
not 86 supported models. All 86 numerical stages remain NOT_RUN. Each supported
case passed all ten structural/formatter stages against independent expectations.
Both binary/source fingerprint guards reported no changes.

The development gate retained **765 original-reference libSBML warnings**,
including 48 from the 12 upstream-accepted/native-rejected cases. Each supported
native, formatted-reference and formatted-native path retained **717 warnings**.
The native/formatter/reparsed-native paths each retained 78 core warning/note
records. Warnings are not silently removed or treated as dimensional certification.

Actual missing-binary and missing-reference controls each gave **86 blocked,
zero passing cases**. Missing binary is aggregate ORACLE_BLOCKED; missing reference
is aggregate FAIL with individual ORACLE_BLOCKED records. The independent reviewer
also passed eight direct protocol controls: preserve Warning/Note, reject Error,
wrong stage, unknown severity, absent message, arbitrary/blank lines and appended
errors. Unit controls cover independent-contract mismatches, wrong rejection
codes, formatting drift, idempotence, source hashes, input mutation/deletion and
failure retention. None earns additional model-success credit.

The first development live run had eight harness failures: legitimate
`Note/default_compartment` records were rejected by an overly narrow stderr parser.
The failure bundles remain under ignored `artifacts/workloads-failures/`; the
checker was corrected to preserve Warning and Note explicitly, with a regression.
No native compiler defect or native-source change was involved. Independent review
also closed a source-identity gap by taking the fingerprint before cached-source
preparation and rejecting any change immediately afterward. A controlled mutation
test demonstrates that no case executes on that discrepancy.

### Final source-bound revalidation

The provenance owner actually ran the following after all source/fixture owners
froze their inputs, using the already installed pinned environments:

```sh
.venv-oracle/bin/python tools/verification/run.py --execute \
  --target-dir target/independent-cli-review \
  --output-dir artifacts/current-source-review
```

**39/39 commands passed**, none unexecuted; all **283 project source inputs** and
recorded dependency/config/toolchain/interpreter/artifact identities remained
unchanged. Python suites passed **147 oracle/CLI, 69 numerical, 51 workload and
31 receipt tests**, zero failures/skips. Native check/build/all-target tests,
doctests, strict Clippy/rustdoc and offline builds passed in four configurations:

| Configuration | Binary unit | Integration | Doctests |
|---|---:|---:|---:|
| All features | 13 | 244 | 2 |
| Default without CLI | 0 | 212 | 2 |
| No-default core | 0 | 205 | 2 |
| CLI without SBML | 13 | 237 | 2 |

The final exact-artifact gates passed original **90 native**, **90 formatter** and
**90 CLI** cases (each 48 supported/42 required rejections), separate **8 analytic
models**, and separate **86 workloads**. Original native/formatted/CLI inspection
paths retain 307 libSBML warnings each; analytic paths retain 104 each. The new
workload warning totals above were reproduced exactly. Original 90 and workload86
numerical stages remain NOT_RUN; the eight analytic models are not pooled with them.

The final all-feature Cargo invocation reported all three selected executables
`fresh: true`: these were reused cached artifacts in an isolated target, **not a
clean rebuild**. Exact paths came from Cargo artifact messages, not guessed paths.

| Artifact | SHA-256 |
|---|---|
| CLI | `000ba7f63adb95d77a38698d0b2c388b5842344d5787e7b594ad59a90eff01c1` |
| Exporter | `fe400be6ab84dff7a60a5293fd187bf3eeaf231d6fc153f51e59c72b9a3d689e` |
| Formatter | `b6ffc6e4449b8a35e606c8a4747a9ebdd9df8158d98f358c718a8fb6122b013e` |

The retained full receipt is `artifacts/current-source-review/receipt.json`, SHA-256
`8ff02e2efb48b6db309e531b351f77399d36c11d6521452c386f618b64cba21b`.
Full checker JSON is in that directory's `logs/{native_corpus,roundtrip,cli,
numerical,workloads}.stdout.log`; the receipt binds each log hash. Durable summaries
were regenerated from those final reports: [native](../../tools/oracle/native-evidence.json),
[formatter](../../tools/oracle/formatter-evidence.json),
[CLI](../../tools/oracle/cli-evidence.json),
[numerical](../../tools/numerical/evidence.json),
[workloads](../../tools/workloads/evidence.json) and
[source receipt](../../tools/verification/evidence.json).
Historical reviews retain their historical binary identities. The source receipt
establishes an observed local build/gate connection, **not** bit reproducibility,
hermetic compilation, upstream source-to-wheel attestation or remote execution.
See the [precise source-evidence boundary](../SOURCE_EVIDENCE.md).

Two documented enzyme-motif CLI showcase commands also passed (check/export),
with one visible warning each and an actual SBML file written. These usability
probes do not increase the model denominator.
Initial discovery exercised eight positive prototype roundtrips and explicit
negative boundaries; these are discovery probes, not the final denominator.

Review identified draft expectation corrections for signed wrapped literals and
upstream precision loss outside the accepted envelope. Corrections were separately
confirmed against the existing pinned semantics; no general normalization or
native acceptance was introduced. The strict scalar rejection remains mandatory.

Final independent reconciliation checked all five compact case projections against
their raw 90/90/90/8/86 reports and receipt/report hashes: PASS. Documentation QA
checked 31 Markdown documents, 178 local links and eight anchors: zero failures
after correcting one stale planning link. `cargo package --list --allow-dirty`
listed 322 entries with the required new workload/receipt files and no artifacts,
virtual environments, target files or Python caches; only the expected unset
package-metadata warning remained. `git diff --check` passed.

## 4. What's Next (Concrete Next Steps)

Expand independently reviewed parameter dimensions or nominate provenance-eligible
external models. Retain future discrepancies without changing expectations to bless
them. Automatic sound reduction remains planned,
not implemented; all 86 new numerical stages remain NOT_RUN.

## 5. Novel Opportunities (Innovation & Monetization)

Small inspectable workload contracts can support exporter/editor regression
pipelines. Honest finite coverage and reproducible failures are more useful than
inflated fuzzing or full-language claims. No publication is implied.

## Key Learnings:

1. A finite parameter grid is not coverage-guided fuzzing.
2. Independent contracts must precede accepting native/reference agreement.
3. Reducing a failure must preserve its meaning and classified discrepancy.
