# Current-source receipt and native manifest-guard review

**Status: PASS for the recorded local, cached build-and-validation observation.**
No Rust language, grammar, profile, Cargo dependency or example protocol changed.
The [guide](../SOURCE_EVIDENCE.md) specifies the boundary; the
[compact receipt](../../tools/verification/evidence.json) retains all 283 project
input hashes, command results/log hashes, tool identities, executable hashes and
checker summaries. Full evidence lives in ignored
`artifacts/current-source-review/receipt.json` and its `logs/` directory.

## 1. What We Built (Plain English)

An opt-in development runner connects a snapshot of current source and locked
dependencies to an observed offline Cargo quality matrix, then validates the exact
executables reported by Cargo. This replaces the weaker practice of checking a
prebuilt binary without recording how it relates to the current working tree.

The native checker also now detects a fixture manifest changed or deleted during
execution. Previously a controlled mocked mutation still produced PASS. It now
records the original manifest hash and rejects aggregate success on manifest or
executable changes, including deletion. Four focused regressions cover this fix.

## 2. Why It Matters (Impact & Implications)

A collaborator can inspect one receipt rather than reconcile unrelated test logs
and executable hashes. Source additions/deletions, changed dependency trees,
configuration changes and changed artifacts invalidate the receipt even when
individual commands passed. The final binary paths come from Cargo's JSON artifact
messages, not guessed standard-target paths. This matters because a CLI-only build
can overwrite the same executable path with SBML export disabled; the final build
explicitly restores all features before compatibility checks.

This is **observed current-source evidence**, not a cryptographically signed,
hermetic or bit-reproducible build attestation. Cargo reported all three final
executables as `fresh=true`: cached artifacts were reused. No freshly compiled
claim is made. The runner records cache reuse honestly and does not delete targets.

## 3. Test Results (Verified, Not Assumed)

Executed after independent reviewer approval and all source/fixture owners froze:

```sh
.venv-oracle/bin/python tools/verification/run.py --execute \
  --target-dir target/independent-cli-review \
  --output-dir artifacts/current-source-review
```

The complete run returned **PASS, exit 0: 39 commands**, all return codes zero,
with **no commands NOT_RUN**. The command groups were four toolchain/metadata
observations, 26 native quality/build commands, four Python suites and five
compatibility/workload checkers. Observed command durations sum to about 178.7
seconds; this is not a performance benchmark or total wall-clock measurement.

### Native and Python gates

| Native feature selection | Binary unit | Integration | Doctests |
|---|---:|---:|---:|
| All features | 13 | 244 | 2 |
| Default | 0 | 212 | 2 |
| No-default core | 0 | 205 | 2 |
| No-default + CLI | 13 | 237 | 2 |

All four selections passed offline/locked check, build, all-target tests, strict
Clippy, separate doctests and strict rustdoc. Formatting also passed. There were
zero native failures or ignored tests.

| Python suite | Passed | Failed/skipped |
|---|---:|---:|
| Structural/CLI oracle | 147 | 0 |
| Selected numerical | 69 | 0 |
| Structural workloads | 51 | 0 |
| Current-source receipt | 31 | 0 |

The structural/CLI suite increased from 143 to 147 through the four native
manifest/executable mutation controls. Independently rerun receipt tests passed;
seven malformed-receipt attacks and four toolchain-link controls were also checked
outside the unit-method count. These control executions are not extra positive
model cases or unique automated test methods.

### Exact built-artifact validation

The runner handed off these isolated-target artifacts:

| Artifact | SHA-256 |
|---|---|
| `antimony` | `000ba7f63adb95d77a38698d0b2c388b5842344d5787e7b594ad59a90eff01c1` |
| `export_sbml` | `fe400be6ab84dff7a60a5293fd187bf3eeaf231d6fc153f51e59c72b9a3d689e` |
| `format_antimony` | `b6ffc6e4449b8a35e606c8a4747a9ebdd9df8158d98f358c718a8fb6122b013e` |

Every checker ran from this receipt using those exact paths, with no automatic
build or executable substitution inside a checker:

- Native corpus: **90 PASS = 48 supported exports + 42 required rejections**.
- Formatter roundtrip: **90 PASS = 48 exact/idempotent roundtrips + 42 required rejections**.
- Unified CLI: **90 PASS = 48 supported cases + 42 required rejections**.
- Selected analytic numerical models: **8 PASS, 0 failed/blocked**.
- Separate structural workloads: **86 PASS = 6 original synthetic motifs + 64
  generated supported cases + 16 required rejections**.

The three 90-case gates overlap; they are not 270 independent models. Their
numerical stages remain NOT_RUN. All 86 structural workloads also remain
numerically NOT_RUN. Only the eight separately selected analytic anchors receive
numerical credit. Warning records remain in the full checker reports; no warnings
were erased to obtain aggregate success. See the [numerical review](numerical-review.md)
and [workload guide](../WORKLOADS.md) for those scopes.

### Immutability and independent inspection

The final receipt records:

- **283 project source/input files**, identical before/after and to the current
  declared inventory during independent inspection.
- **12 locked registry dependencies / 531 extracted source files**, unchanged.
- **4,706 compiler sysroot files**, unchanged. One Homebrew file symlink is bound
  to its link text, resolved LLVM target path and target bytes rather than skipped.
- Seven actual tool executable identities and Cargo configuration discovery;
  actual Cargo/rustc report Homebrew Rust 1.96.0, not an inferred rustup channel.
- **78 command-log hashes** and **three executable hashes**, independently
  recomputed successfully.
- All eight change categories empty: source, dependency, Cargo config, tool bytes,
  tool resolution, sysroot, Python interpreters and built artifacts.

Root independently checked the full receipt, all commands, current source inventory,
log/artifact hashes and checker counts. A second reviewer independently inspected
its boundary and controls. Generated compact evidence summaries are explicitly
excluded from the source inventory and were regenerated from these new full reports,
not updated by merely patching their hashes.

### Review findings fixed before this run

An early receipt validator checked only aggregate PASS, case count and unique IDs.
Independent adversarial review showed it could accept wrong numerical IDs and a
failed or unexecuted analytic substage. The final validator binds the exact frozen
90/8/86 inventories, source hashes, required per-checker substages, outcomes and
count maps. It distinguishes legitimate expected-rejection failures/NOT_RUN stages
from missing required positive work. Seven independently mutated numerical reports
were rejected. The original valid eight-case report remained accepted.

Actual-host preflight also found a Homebrew `rust-objcopy` symlink in the compiler
sysroot. The toolchain-specific inventory now records both link and target identity;
project/dependency source symlink rejection remains unchanged. Target mutation,
same-byte retargeting, dangling targets and directory links were tested. An initial
handwritten test required resolving macOS `/var` to `/private/var`; this was test
setup correction, not a production failure hidden from the final run.

## 4. What's Next (Concrete Next Steps)

Run this receipt again after relevant source/fixture/tool changes, rather than
continuing to advertise old executable evidence as current. Keep a cooperative
source freeze during execution. The guide lists precise exclusions and trust
assumptions: before/after snapshots cannot defeat hostile transient edits restored
between observations; SDK/OS/linker internals and all dynamic dependencies are not
a complete fingerprinted closure.

Future improvements can add a separately authorized clean target build, cross-host
receipts and stronger dependency/source attestation. Those are distinct gates, not
claims earned by this cached local run. Six remote native CI jobs remain NOT_RUN;
no publication, licensing, external contact or external work was performed.

## 5. Novel Opportunities (Innovation & Monetization)

Machine-readable receipts can make exporter/library integration reviews easier in
scientific pipelines: users can see exactly which source, tools and checks support
a result. The useful distinction is inspectable evidence with bounded claims—not
a new claim of universal correctness or biological validation. The pattern could
later support local CI/reporting integrations without adding runtime dependencies.

## Key Learnings:

1. Executable hashes alone do not establish an observed current-source build.
2. Cached Cargo artifacts must be labeled cached, not freshly compiled.
3. Complete stage contracts and exact inventories matter more than aggregate PASS.
4. Source immutability, toolchain identity and reproducible builds are different claims.
