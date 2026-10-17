# Workload and current-source evidence plan

Date: 2026-10-10. Status: delivered bounded increment; independently checked.

## Completed evidence (separate from the original plan below)

- 51 workload unit tests and 86 live structural cases passed: 6 original motifs,
  64 supported generated cases and 16 explicit required rejections. Numerical
  stages remain NOT_RUN. These are not published models or coverage-guided fuzzing.
- The observed current-source receipt passed 39 commands with no unexecuted stage
  and no changes among 283 project inputs or recorded external/artifact identities.
  Four native configurations and 147 oracle/69 numerical/51 workload/31 receipt
  Python tests passed, followed by exact-artifact 90/8/86 evidence gates.
- Failure retention is implemented; automatic semantic-preserving minimization is
  explicitly deferred/NOT_RUN, not presented as delivered. Existing tiny recipes
  provide bounded reproducible failures. The initial eight harness-only Note
  handling failures were retained and fixed with regression coverage.
- Before/after source observations and cached Cargo artifacts are not hermetic,
  bit-reproducible or hostile-environment attestations. Remote jobs remain NOT_RUN.

See [workload usage](WORKLOADS.md), [source-evidence boundary](SOURCE_EVIDENCE.md)
and [final review](reviews/workloads-review.md) for commands, identities and limits.

## Purpose and boundaries

After the eight selected analytic models, broaden structural coverage and tie local
verification to the source used in a recorded build. Keep the native language
profile, original 90-case corpus and eight numerical anchors unchanged. This is
not broad biological validation, fuzz coverage, full Antimony compatibility,
hermetic compilation, or bit-reproducible-build certification. No publishing,
remote execution, licensing decision or external changes are included.

## Phase 0 — discovery completed

Three agents inspected existing APIs and performed bounded read-only probes:

- Native export/reference/format helpers in `tools/oracle/{native_check,oracle,
  roundtrip_check,cli_check}.py`; typed projection contracts in `contracts.py`.
- Native manifest mutation was reproduced with a controlled mock: the native
  checker could report PASS after its manifest changed. Copy the before/after
  byte/hash guard from `roundtrip_check.py`, including deletion handling.
- Eight generated positive prototypes matched native/reference semantics and
  formatting. Numeric underflow is not a generic accepted-reference negative:
  `1e-308` reaches an unsupported projection representation. Each rejection family
  must freeze its actual reference status and native diagnostic independently.
- Six original multi-reaction motifs passed structural/reference and idempotent
  formatting probes. These are representative motifs, not published real models.
- Actual cargo/rustc binaries are Homebrew Rust 1.96; the rustup channel name alone
  does not identify the invoked compiler. Cargo artifact JSON, offline/locked
  execution and actual executable identities are required.

## Phase 1 — workload contracts

1. Implement a deterministic versioned grid of 64 supported generated cases
   (four families, 16 cases each) and 16 explicit negative cases. Cover arithmetic
   initialization DAGs, compartments, units and numeric restrictions without
   inventing language semantics. Record exact sources, parameters and hashes.
2. Add six original multi-reaction motifs: enzyme binding/catalysis, branched
   pathway, unequal-volume transport/catalysis, rational product inhibition,
   reversible compartment shuttle and a unit-annotated sequential pathway.
3. Author static expected projections independently from native outputs. Positive
   credit requires matching reference contract, native projection, formatting
   idempotence and reference interpretation of formatted output. Negatives require
   the expected reference status AND native phase/code; arbitrary rejection is
   not success. Keep warnings visible.
4. Preserve failures with their exact source/parameters. Any bounded minimizer
   must retain the same classified discrepancy and valid prerequisites; missing
   tools or invalid reduced sources must never become minimized compiler bugs.
5. Keep generated80, motif6, structural90 and numerical8 denominators distinct;
   workload numerical stages remain NOT_RUN. Derived conservation laws are not
   claimed as trajectory tests.

Verification: deterministic regeneration, independent-contract mutation tests,
missing-tool/malformed-report controls, finite/numeric boundary checks, input drift,
positive and rejection live gates. No random smoke tests labeled coverage-guided
fuzzing, no generated source accepted merely because both implementations agree.

## Phase 2 — source-bound local verification receipt

1. Fix the native manifest drift/deletion false-PASS and add regression tests.
2. Implement an explicit development runner with no install/download behavior.
   Fingerprint source, tests, Cargo files, checker code, profiles, fixtures,
   dependency sources and relevant configuration before/after; rediscover added
   and deleted inputs. Explicitly exclude caches and generated evidence outputs
   to avoid self-referential hashes.
3. Record actual cargo/rustc executable paths, hashes, verbose versions/sysroot,
   lockfile dependency identities and a narrow nonsecret build-environment list.
   Record limitations for system libraries, build caches and external state.
4. Use bounded shell-free commands, `--offline --locked`, an isolated target and
   retained return codes/log hashes/timeouts. Run native quality/tests and Python
   suites. Finish with an all-feature CLI/example build to avoid a CLI-without-SBML
   artifact replacing the executable to be verified.
5. Obtain executables from Cargo artifact messages; pass those exact paths to
   native/formatter/CLI/numerical/workload gates. Missing outputs, malformed
   reports, nonzero exits, failed statuses or input changes fail the receipt;
   unexecuted stages remain NOT_RUN.

Verification: unit tests with mutation/addition/deletion/tool failures, actual
source-frozen local run, independent report/artifact matching. This records an
observed source-bound build using local caches, not an independently reproduced
binary or a proof of full dependency/toolchain trust.

## Ownership and final verification

- Workload owner: generation, shared checking, reduction, tests, guide and review.
- Motif/reviewer owner: original fixtures/manual expectations/derivations, followed
  by independent workload and receipt review.
- Provenance owner: runner/tests/guide and native-manifest guard.
- Root: phase coordination, evidence reconciliation, status documentation plan.

No full source-bound run until source/fixture owners freeze inputs. Refresh any
prior evidence invalidated by the native-checker change; do not merely patch its
hash. Independently review tests and negative controls, run actual build/test
commands, verify package/cache exclusions and documentation links. Record exact
counts and unresolved limitations before updating current-status documentation.

## Follow-on work

Published-model adoption needs explicit provenance, licensing and unsupported
feature triage. Coverage-guided fuzzing, remote OS/toolchain jobs, broader language
features and collaborative release approval remain separate milestones.
