# Independent Phase 1 review

Review date: 2026-10-09 America/New_York. Scope: Phase 1 foundations and isolated
reference tooling in `<repo-root>`, before Phase 2.
**Gate: PASS for starting the narrow Phase 2 vertical slice.** This is not native
Antimony compatibility, a release review, or approval to publish.

## 1. What We Built (Plain English)

An independent review verified the building blocks that will let a native parser
report precise errors without confusing byte offsets, Unicode characters, or
separate input sources. It also verified that a separate reference process can
turn reviewed fixtures into independently inspected SBML, and distinguish a bad
model from a missing or failed reference tool. The reviewer changed only this
review document; implementation fixes were made by the owning workers.

Four concrete review findings were resolved before this gate:

- The initial oracle could project unrepresented package data as success. Its
  closed XML/MathML vocabulary now refuses unsupported structures and namespaces.
  Independent valid FBC input returned `PROJECTION_UNSUPPORTED`.
- Corpus check initially enforced intended acceptance only during golden recording.
  Both modes now reject mismatches; expectation paths are constrained to the owned
  manifests directory. Independent negative probes confirmed both guards.
- The package inventory included Python bytecode. Explicit Cargo exclusions now
  remove caches, virtualenvs, downloaded artifacts, and build output, even before
  the repository's initial commit.
- Compatibility records lacked the planned schema and a mapping to frozen profile
  identifiers. Records now use F01–F13/X01–X13, validate the manifest and records,
  and expose separate syntax/resolve/export/numerical stages, all `NOT_RUN` here.

## 2. Why It Matters (Impact & Implications)

The next native slice can rely on checked source spans and actual reference
semantics instead of guessed arrow direction, precedence, or default values.
Reference success is not mistaken for native implementation success. Missing
oracles, crashes, timeouts, unsupported projection forms, and malformed SBML do
not pass compatibility gates.

Source IDs remain caller-owned and must be unique within a compilation.
Diagnostic budgets bound the count, not caller-supplied message/label bytes;
callers must propagate a rejected `Diagnostics::push` rather than relying only
on `has_errors()`. Future token/depth/entity consumers must invoke their guards.
No parser, semantic analyzer, exporter, native Python subprocess, or hidden
scientific defaults exists in the reviewed native foundations.

Reference processes are **not a sandbox**. CLI conversion requires trusted-input
acknowledgment; the corpus additionally confines and hashes its inputs and rejects
textual imports. Never expose this reference harness to hostile input. Its
projection is narrow and not a general SBML-equivalence checker.

## 3. Test Results (Verified, Not Assumed)

Independently executed on macOS 15.5 arm64 with Rust/Cargo 1.96.0 and oracle
CPython 3.12.12. Commands below were run from the repository root:

| Command | Observed result |
|---|---|
| `cargo fmt --all --check` | Pass |
| `cargo check --all-targets --all-features` | Pass |
| `cargo build --all-targets --all-features` | Pass |
| `cargo test --all-targets --all-features` | 22 integration passed; 0 unit; 0 failed/ignored |
| `cargo test --doc --all-features` | 1 passed; 0 failed/ignored |
| `cargo test --no-default-features` | Same 22 integration + 1 doctest passed; not additional unique cases |
| `cargo clippy --all-targets --all-features -- -D warnings` | Pass |
| `RUSTDOCFLAGS='-D warnings' cargo doc --no-deps --all-features` | Pass |
| `cargo package --list --allow-dirty` | Pass; 54 entries before this review file, no cache/venv artifacts |
| `.venv-oracle/bin/python -m unittest discover -s tools/oracle -p 'test_*.py' -v` | 30 passed: 14 harness + 8 contract + 8 live reference; 0 failures/skips |
| `.venv-oracle/bin/python tools/oracle/corpus.py check` | 12 replay passes: 11 upstream-accepted, 1 intentional rejection |
| `.venv-oracle/bin/python tools/oracle/oracle.py probe` | Pass; Antimony 3.2.0, independent libSBML 5.21.2 |
| `.venv-oracle/bin/python -m py_compile tools/oracle/*.py` | Pass |
| `.venv-oracle/bin/python tools/oracle/corpus.py check --python /no/such/python` | Expected exit 2; 12 `ORACLE_BLOCKED`, no passes; all native stages `NOT_RUN` |

The 30 Python tests were also preceded by separate 14-test harness and 8-test
integration runs, not counted twice. Six additional inline Python assertions
checked both cached wheel SHA-256 digests against `wheels.json`, mismatched intended
acceptance, escaped expectation paths, valid FBC projection refusal, and DTD
refusal. All six passed. The first reviewer FBC probe lacked required FBC
attributes and correctly returned `SBML_INVALID`; correcting that probe yielded
`PROJECTION_UNSUPPORTED`, as intended. No product test failure was hidden.

The first reference model has zero independent errors and 12 retained unit-related
warnings. They are categorized and explained in `docs/ORACLE.md`; this is not a
claim of dimensionally certified biology. Package inventory warns about missing
license/repository metadata, appropriately pending approval while `publish=false`.

Wheel hashes were verified locally. The release tag SHA, research SHA, and unknown
wheel-build source SHA remain distinct and honestly documented. The review did
not independently establish binary-to-source reproducibility. No numerical
simulation, other operating system, alternate Rust toolchain, full-profile corpus,
or fuzz campaign was run. Their absence is not success.

## 4. What's Next (Concrete Next Steps)

1. Implement only the explicit first-reaction native parse/resolve/export slice,
   using these guards and privately constructed validated output. Reject events
   and imports without I/O; do not infer unsupported syntax or biological values.
2. Independently inspect native SBML, compare its structured projection against
   the pinned reference, and assert warnings/required attributes explicitly.
3. Add parser-specific token/depth/source exhaustion and unsupported-feature
   tests; the foundation budget API alone does not prove parser safety.
4. Expand and audit projection forms and the corpus before broadening claims;
   obtain maintainer agreement on the oracle target and license before release.
5. If a codebase graph is adopted, run `graphify update .` after code changes.

## 5. Novel Opportunities (Innovation & Monetization)

A versioned, provenance-bearing compatibility ledger could make a future native
Antimony implementation easier for external reviewers and downstream consumers
to trust. Publishable value would be reproducible translation evidence and precise
unsupported-feature reporting, not an unearned claim of full compatibility or
scientific validation. Any sharing or commercialization remains separately
subject to licensing and maintainer authorization.

## Key Learnings:

1. A successful upstream conversion and a native compatibility pass are separate facts.
2. Independent projection must reject unrepresented package behavior, not erase it.
3. Explicit Cargo exclusions matter before the initial commit; ignore files alone
   did not keep Python bytecode out of this package inventory.
