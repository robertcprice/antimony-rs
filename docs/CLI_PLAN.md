# Unified CLI execution plan and decision log

Date: 2026-10-10. Status: **implemented and independently verified locally**.
This is an adapter increment, not a new language profile or simulation engine.
User instruction: continue proactively and document decisions and evidence as work
proceeds. All work stays in this standalone repository.

## Phase 0 — documentation discovery completed

Three delegated reviews inspected the current compiler API, filesystem adapters,
diagnostic locations and reference harness before implementation.

Allowed APIs and copy-ready patterns:

- `SourceFile::new`, `SourceFile::location` and `SourceLocation` in `src/source.rs`:
  checked UTF-8 spans; one-based lines, Unicode scalar columns and byte columns.
- `parse`, `analyze`, `format_antimony`, and optional `write_sbml` in `src/lib.rs`:
  existing validated pipeline; no duplicate syntax or scientific semantics.
- Diagnostic severity/code/labels/notes in `src/diagnostic.rs`; read-only model
  identity, entity counts and warnings in `src/model.rs`.
- `examples/format_antimony.rs` demonstrates same-directory `create_new`,
  `write_all`, `flush`, cleanup and rename. The export example's direct write must
  **not** be copied into the new CLI.
- `tools/oracle/native_check.py` and `roundtrip_check.py` provide trusted-executable
  subprocess isolation, original corpus expectations and fail-closed comparison.
- Installed `serde_json` 1.0.150 source documents `Value`, `json!` and serialization;
  use the established encoder rather than inventing JSON escaping.
- [Rust rename documentation](https://doc.rust-lang.org/std/fs/fn.rename.html)
  describes replacement and platform differences. Documentation is not evidence
  of local Windows/Linux execution. Test the actual Rust 1.96 build locally.

## Decisions before coding

1. **Optional adapter:** `cli` feature gates binary `antimony` and exact optional
   `serde_json = 1.0.150`; default features remain `sbml`. No-default library remains
   dependency-free. CLI-only builds still support check/format; export reports an
   explicit unavailable-capability error before opening input.
2. **Small command surface:** `check INPUT`, `format INPUT [-o OUTPUT]`, and
   `export INPUT [-o OUTPUT]`. Explicit `-` selects stdin or raw artifact stdout;
   absent output means raw stdout. Support help/version and `--` for literal paths.
   Use `args_os`, not lossy path conversion for filesystem operations.
3. **Separate reports from artifacts:** human diagnostics/status go to stderr.
   `--json` puts exactly one versioned report on stdout, suitable for pipelines.
   JSON format/export require an actual output path, not `-`; reject collisions
   before reading or writing. No artifact text embedded in reports.
4. **Stable result meaning:** exit 0 success, 1 usage/I/O/unavailable capability,
   2 language/resource rejection. Reports retain warnings, stages, source labels,
   notes and model counts. They do not certify biological or numerical correctness.
   Native rejection codes are forwarded, not guessed from message prose.
5. **Safe explicit output:** validate/generate completely before staging output.
   Reject final output symlinks, nonregular destinations and read-only files;
   preserve regular-file permission bits. Allow explicit in-place replacement.
   New Unix staging files are private (0600). Rename detaches an output hardlink
   rather than modifying its other names. Keep filesystem handling out of core.
6. **Honest failure boundaries:** staging protects existing output before commit,
   not against hostile parent-directory races or power failure. Ownership, ACLs,
   extended attributes and inode identity are not preserved. If report output
   fails after artifact commit, return failure without pretending rollback occurred.
   Use fallible output writes, not panic-on-broken-pipe printing.
7. **Bound input bytes first:** read at most source-cap + 1 bytes and detect size
   excess before UTF-8 decoding. This avoids misclassifying an oversized multibyte
   source cut by the read bound as merely an encoding error. Reject nonregular
   file inputs to avoid intentionally opening devices/FIFOs; stdin remains explicit.

## Phase 1 — implementation ownership and gates

- CLI owner: manifest/lock, argument adapter, report rendering and focused CLI tests.
- Filesystem/review owner: isolated binary I/O module and independent safety tests.
- Evidence/documentation owner: user guide, report schema, compatibility harness
  and review. Existing corpus, core semantics and example protocols stay unchanged.
- Root: decisions, integration review, scope control and final verification.

Verify argument errors/flags/delimiter/path encoding, exact JSON shape, escaped
human terminal controls, warning visibility, byte/character positions, size bounds,
no-default CLI capability errors, stdin/stdout/report separation and exit codes.
Verify sentinel preservation on rejection/write failure, temporary cleanup,
Unicode/in-place paths, symlink/read-only/directory/hardlink behavior and broken
pipes. Platform-gated tests must be identified, never counted as portable proof.

Anti-pattern guards: no new parser/evaluator, runtime oracle/FFI/process calls in
core, invented JSON model standard, suppressed warnings, direct destination
truncation, changed oracle protocol, or claims of full Antimony support.

## Phase 2 — independent verification and documentation

Run format/check/build/tests/strict Clippy/rustdoc with all features and the relevant
no-default combinations, including CLI without SBML. Recheck zero-dependency core.
Compare the CLI over all 90 original reference cases: 48 supported exports and 42
required rejections. Check and format must agree with the same native pipeline;
successful artifact bytes must match the existing adapters before independently
checking reference semantics. Missing prerequisites block rather than pass.

Freeze all source, including rustdoc, before final binary hashes and oracle evidence.
Record actual commands, counts, warnings, unavailable stages and discovered defects
in `reviews/cli-review.md`; user-facing usage in `CLI.md`; report contract in
`cli-report.schema.json`. Update current status documents only after the gate.
Do not replace historical evidence with unexecuted claims.

## Deferred work

No solver installation, numerical certification, new language features, remote CI
execution, publication, license selection, commit or remote creation in this phase.
After this gate: independent analytic numerical anchors, representative workloads,
structured differential generation and stronger reproducible-source evidence.

## Final execution checkpoint

The selected adapter contract passed: 13 CLI binary unit tests + 244 integration
+ 2 doctests with all features; default 212 + 2; core no-default 205 + 2; CLI-only
13 binary unit + 237 integration + 2 doctests. All four quality/build/lint/doc/offline
configurations passed with no failures/ignored tests; no-default core has zero
normal third-party dependencies.

The full Python suite passed 143 tests. All 90 unchanged reference cases replayed;
the CLI gate passed 48 supported cases and 42 required rejections across all three
commands, with exact artifact/report and independent semantic checks. Missing
binaries/oracle each blocked 90 cases with no success credit. Changed example
binary identities were independently revalidated through the existing native and
formatter gates (48 exact/idempotent + 42 rejections). Final fingerprints, warning
counts, schema/safety limitations and manual process evidence are recorded in
[the CLI review](reviews/cli-review.md), not inferred from this plan. Numerical
simulation and remote Linux/macOS/Windows CI remain NOT_RUN.
