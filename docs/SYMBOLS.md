# Flexible flat-model symbol foundation

**Verified bounded increment: `flat-model-v0.2-slice`.**
Machine-readable scope: [implementation boundary](../profiles/flat-model-v0.2.json).
API migration: [optional initial-value getters](SYMBOLS_API.md).
This is Phase 2 of [the flexible-modeling plan](FLEXIBLE_MODELING_PLAN.md), not
Phase 2 of the original implementation roadmap. Earlier review documents and
source-bound receipts remain historical evidence for their recorded sources.

The purpose is ordinary model authoring: declaration-only symbols, source-ordered
compatible declarations, const/var qualifiers, inferred species, unnamed reactions,
and symbolic initial values whose dependencies are not yet assigned. A missing
value is not zero. Unset compartment size is not the implicit default-compartment
size of one. A symbolic InitialAssignment is not a continuously evaluated rule.

## Reference contracts, not native-derived goldens

`fixtures/manifests/symbols-v2.json` pins original project-authored sources and
Antimony 3.2.0 / independent libSBML 5.21.2 reference snapshots. Native output is
never used to generate these expectations. The ordinary CLI remains open to
arbitrary source files and stdin: fixture hashes are a verification-tool input
boundary, not a native-language whitelist.

The gate checks original input through check, format and export; command warning
agreement; formatting twice; native export of formatted input; reference conversion
of formatted input; and independent inspection of both exports. Structural
comparison is exact, without algebraic normalization or floating tolerance. Missing
attributes remain null in the projection. All libSBML warnings are retained;
errors/fatal messages fail validation. No simulation or trajectory claim follows
from acceptance of an incomplete model.

Known constant domain errors still reject even if another expression branch has an
unset dependency. Cycles reject. Computed compartment sizes, reaction-valued
expressions, functions, time, rules, events and model hierarchy need separate
support; see the final implementation boundary after independent review.

## Explicit historical outcome overlay

The original 90-case manifest and snapshots, arithmetic 54-case manifest and
snapshots, and 86-case workload recipes/reference contracts are retained unchanged.
`fixtures/manifests/flat-model-v2-overlay.json` explicitly changes **native outcomes
only** for seven original cases (unset values, declaration updates/promotion and
qualifiers) and four unset workload cases. It pins the historical manifests and
workload contract source. The arithmetic expectations do not change; its report distinguishes the historical
corpus profile from the implementation profile verified in current CLI reports.

Historical gate defaults retain historical expectations. Use `--current-profile`
explicitly for the current original-corpus and workload expectations. Reports
record the active implementation identity and overlay hash. A changed reference
snapshot, changed projection, unavailable oracle, or unexpected native acceptance
is never automatically converted into a pass.

## Reproduce with trusted prebuilt binaries

These commands do not build, install or download anything. Substitute independently
built executable paths if appropriate; use an immutable copy for a final review.

```sh
python3 tools/oracle/native_check.py --trusted-native --current-profile \
  --native target/debug/examples/export_sbml
python3 tools/oracle/roundtrip_check.py --trusted-native --current-profile \
  --native target/debug/examples/export_sbml --formatter target/debug/examples/format_antimony
python3 tools/oracle/cli_check.py --trusted-native --current-profile \
  --cli target/debug/antimony --native target/debug/examples/export_sbml \
  --formatter target/debug/examples/format_antimony
python3 tools/oracle/expression_check.py --trusted-native --cli target/debug/antimony
python3 tools/oracle/symbols_check.py --trusted-native --cli target/debug/antimony
python3 tools/workloads/check.py --trusted-native --current-profile \
  --native target/debug/examples/export_sbml --formatter target/debug/examples/format_antimony
```

The shared CLI report validator requires the current implementation identity even
when the historical corpus expectation mode is selected. Do not use that combination
to claim reproduction of a historical CLI binary. Historical receipts retain the
checker hashes and binary hashes with which they were originally produced.

`tools/verification/run.py` remains the historical source-receipt recipe and
hardcodes the older outcome inventories. Its previously retained receipt is not
fresh Phase 2 evidence. The current overlay gates above are the review entrypoints.

## Verified results (2026-10-14)

Final gates used the reviewer's stable three-binary handoff, with unchanged input
fingerprints. [Compact evidence](../tools/oracle/symbols-evidence.json) links raw
reports and binary hashes; [independent review](reviews/symbols-review.md) records
the native feature matrix and independently authored source checks.

- Symbol corpus: **66 cases, 54 exact supported exports and 12 expected rejections**.
- Original corpus: **90 cases, 55 supported and 35 expected rejections**, each through
  native export, formatter and CLI gates. These are three routes through the same
  sources, not 270 distinct models.
- Arithmetic corpus: **54 cases, 47 supported and seven expected rejections**.
- Workloads: **86 cases, six original motifs plus 68 generated supported cases and
  12 expected rejections**. These are structural, not numerical results.
- Existing numerical regression: **eight analytic models passed**, with both native
  and reference trajectories. This does not add numerical coverage for incomplete
  symbol models or establish biological validity.
- Python: **334 tests passed, zero failures/skips** (183 oracle, 69 numerical,
  51 workloads, 31 historical receipt tests). Receipt unit tests passing does not
  make the historical receipt recipe compatible with this profile.
- The API guide's Rust example compiled against the independently built library
  and ran successfully.

Preliminary candidate checks that saw replaced binaries correctly failed their
aggregate input-stability gates and were not credited. An initial numerical run
using a relative interpreter path was blocked by the isolated worker directory;
the final run used the absolute pinned interpreter path. Neither result was hidden
or counted as a pass.
