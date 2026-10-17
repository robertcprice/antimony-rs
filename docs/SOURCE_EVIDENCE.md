# Observed current-source build receipts

**Historical checkpoint: local receipt PASS — 39 commands, no unexecuted stages.**
The retained receipt predates the arithmetic expansion. Its hashes do **not**
describe the current edited source; a new full receipt has not yet been recorded.
Its runner also hardcodes older native-outcome inventories and cannot certify
the expanded `math-core-v0.2.1-slice` profile merely by refreshing hashes. Use the
[current maintained gates](BUILTINS.md#reproduce-after-an-immutable-reviewer-handoff)
with their explicit current-profile overlays. Those gates and the independent
native matrix are separate evidence, not a replacement full-source receipt.
See the [compact receipt](../tools/verification/evidence.json) and
[independent review](reviews/source-evidence-review.md) for exact counts and limits.
All three final Cargo executable messages reported `fresh=true`: this run reused
validated cached artifacts, not a newly compiled or clean-room binary build.
The tool creates local evidence connecting observed source,
locked dependencies, actual tool binaries, Cargo commands and validated artifacts.
It does **not** establish a hermetic build, reproducible binaries, an upstream
source-to-wheel attestation, remote CI execution or release approval.

## Why another evidence layer?

A prebuilt executable hash identifies bytes, but alone does not show that the
current working source was built. Existing semantic/numerical checkers deliberately
accept prebuilt trusted programs and do not invoke Cargo. This runner adds the
missing build observation while preserving that separation.

A focused review also reproduced a narrower defect: the native checker could
report PASS if its fixture manifest changed during the run. It now records
`source_manifest_sha256`, checks `manifest_changed_during_check`, and treats
manifest/executable deletion as a structured failure. Case-level successes remain
visible, but aggregate evidence fails. Four new regressions cover unchanged input,
manifest mutation, manifest deletion and executable deletion. This changes checker
identity, so prior receipts referencing its old hash must be refreshed, not silently
presented as current.

## Opt-in execution

Freeze source, fixtures, checker code and runtime changes with all contributors
before starting. The runner never installs packages, downloads dependencies,
changes the standard Cargo target, rewrites evidence summaries, or cleans caches.

```sh
.venv-oracle/bin/python tools/verification/run.py --execute \
  --target-dir target/independent-cli-review \
  --output-dir artifacts/current-source-reviewed
```

The output directory must be a **new** child of ignored `artifacts/`; existing
runs are not overwritten. The target must be an isolated child of `target/`, not
`target` or `target/debug`. Reusing the existing independent target saves disk;
this is explicitly a cached build observation, not a clean-room rebuild. At
initial discovery, free disk was approximately 4.9 GiB and the reusable target
occupied 934 MiB. No automatic target deletion is authorized.

`--oracle-python` and `--numerical-python` select already installed trusted
interpreters. Missing tools fail closed. `--timeout` sets a 1–3600-second deadline
per command, default 1200. Every command is shell-free. Each stdout/stderr stream
is drained with a 16 MiB retained-byte limit; timeout, overflow, log-write failure,
crash and nonzero exit invalidate that command. Owned POSIX process groups are
terminated, including children, rather than leaving Cargo compilers behind.
This process-management implementation requires POSIX; Windows execution is not
claimed. Cargo's offline flag does not sandbox build scripts or prevent arbitrary
trusted subprocess code from using the network.

## Exact build and validation sequence

1. Record source inventory, resolved tool executable paths/hashes, filtered build
   environment and applicable Cargo configuration paths, including absent configs.
2. Record verbose Cargo/rustc versions and actual compiler sysroot. Fingerprint
   sysroot libraries. Toolchain file symlinks record both link text/resolved path
   and target bytes; directory/dangling links are rejected. Do not infer the compiler from `rustup show`: on the reviewed
   host Cargo/rustc are Homebrew 1.96.0 binaries even though rustup separately
   reports a stable toolchain.
3. Run `cargo metadata --offline --locked --all-features --format-version 1`.
   Record resolved registry dependency names, versions, sources, Cargo.lock
   checksums and actual extracted source-file/tree hashes. External path/git
   dependencies are outside this bounded runner's scope and are rejected.
4. Run formatting checks. For **all features, default, no-default core, and
   no-default plus CLI**, execute check/build/test/strict Clippy over all targets,
   separate doctests and strict rustdoc. Every applicable Cargo command uses
   explicit `--offline --locked --target-dir`; `cargo fmt` has no such flags and
   performs only its check operation.
5. **Last native build:** all-feature CLI and examples with Cargo JSON output.
   This must follow the CLI-without-SBML gates, which may otherwise leave a
   feature-reduced CLI at the shared executable path. Extract the three required
   executable paths from successful `compiler-artifact` messages and require the
   `cli`/`sbml` features and final `build-finished` success. Do not guess paths or
   use an old standard-target binary. Record each artifact's hash and Cargo
   `fresh` flag; cached artifacts are not relabeled freshly compiled.
6. Run Python unit suites for structural/CLI, numerical, workload and receipt
   tools in their appropriate pinned interpreters. Zero tests, skips and expected
   failures do not earn a successful complete-suite receipt.
7. Pass those **exact built paths** into the existing native-export, formatter
   roundtrip, CLI, eight-model numerical and 86-case workload checkers. Check exit
   codes **and** expected JSON schema, aggregate PASS, required case count, exact
   frozen case identities/source hashes, schema-specific required substages/outcomes,
   expected-rejection diagnostic contracts, mutation flags and exact count maps. Retain
   full checker stdout/stderr and their hashes. A zero exit with a FAIL report
   does not pass. The three 90-case gates overlap; their numerical NOT_RUN stages
   remain unchanged. Eight selected analytic models are separate. Workloads add
   structural checks, not new numerical claims.
8. Rediscover source/config/dependency/sysroot inputs, compare tool identities and
   executable hashes, and fail the aggregate receipt on changes. Files added or
   deleted during the run count as changes, not just modified existing files.

Primary Cargo references: [build flags and JSON output](https://doc.rust-lang.org/cargo/commands/cargo-build.html),
[configuration discovery](https://doc.rust-lang.org/cargo/reference/config.html),
and [environment variables](https://doc.rust-lang.org/cargo/reference/environment-variables.html).

## Fingerprint scope and exclusions

Included project directories: `src`, `tests`, `examples`, `tools`, `fixtures`,
`profiles`, `.github`. Included root inputs: `Cargo.toml`, `Cargo.lock`, and optional
`build.rs`, `rust-toolchain`, `rust-toolchain.toml`. All discovered files count,
including untracked files. Project/dependency input symlinks, dangling known-input links and nonregular
files are rejected rather than followed outside the declared trees.

Explicitly excluded generated summaries:

- `tools/oracle/native-evidence.json`
- `tools/oracle/formatter-evidence.json`
- `tools/oracle/cli-evidence.json`
- `tools/numerical/evidence.json`
- `tools/workloads/evidence.json`
- `tools/verification/evidence.json`

Also excluded: `target`, `artifacts`, virtual environments, caches, `.git`, Python
bytecode/cache directories, and prose-only README/docs outside the included source
roots. Rustdoc comments inside Rust source **are included**. Numerical environment,
wheels and requirements records remain included; they are inputs, not generated
receipt summaries. These exclusions avoid self-invalidating receipts when summaries
are regenerated, but mean a receipt does not bind all repository prose or all files
inside a Python environment. Individual numerical checker fingerprints cover its
selected installed runtime files; wheel/source provenance limits still apply.

The environment passes only PATH, HOME, CARGO_HOME, temporary-directory settings,
SDKROOT, MACOSX_DEPLOYMENT_TARGET, DEVELOPER_DIR and SYSTEMROOT when present, plus
explicit compiler/rustdoc paths, target directory, offline/color controls,
`RUSTDOCFLAGS=-D warnings`, locale and Python bytecode policy. Other inherited
variables—including credentials, compiler wrappers and arbitrary Rust flags—are
removed. Applicable Cargo configs are fingerprinted without copying their content
into reports. The bounded policy accepts only `[build].jobs`, `[net].offline` and
`[term].color/quiet/verbose`; wrappers, linker/rustflag overrides, aliases, includes,
registries and other overrides are rejected pending a separately reviewed expansion.

OS frameworks, SDK contents, system linker internals and the complete dynamic-library
closure are not fingerprinted. Installed dependency trees and locked archive checksums
are recorded as distinct observations; that is not an independent proof that every
extracted byte originated in a publisher's source release. Before/after checks also
cannot detect a hostile transient mutation restored between snapshots. Trusted local
execution and cooperative source freeze are assumptions, not a security sandbox.

## Receipt and failure semantics

The ignored output contains `receipt.json` (`antimony-current-source-receipt-v1`)
and bounded per-command log files. It records command argv/cwd/deadline/elapsed time,
return codes and log hashes, source/dependency/config/toolchain snapshots, selected
Python interpreter hashes, built artifacts, checked report summaries and explicit
`not_run_commands`. Aggregate status is PASS only after all required stages and
final immutability checks succeed. BLOCKED prerequisites and FAIL outcomes return
exit 2; only PASS returns exit 0. Individual completed stage successes do not erase
a later failure. A receipt is itself ordinary local evidence, not a signed attestation.

Unit tests use synthetic temporary inputs and mocked Cargo results plus real bounded
Python subprocess controls. They never compile the project. Run them with:

```sh
.venv-oracle/bin/python -m unittest discover -s tools/verification -p 'test_*.py' -v
.venv-oracle/bin/python -m unittest discover -s tools/oracle -p test_native_check.py -v
```
