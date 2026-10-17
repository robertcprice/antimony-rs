# Independent bounded units review

Reviewed 2026-10-10 UTC. **Gate: PASS for `units-flat-v0.1-slice`.**
This extends the verified initialization slice; it does not complete Phase 3,
all units, the target `kinetic-core-v0.1` profile, or a release gate.

## 1. What We Built (Plain English)

Supported models can preserve explicit parameter/compartment unit annotations
and small named unit definitions in SBML. Read-only queries distinguish known
dimensions, unspecified units and indeterminate expressions. Provable conflicts
fail before export; missing information is not silently treated as dimensionless.
Eleven independent review tests supplement nineteen implementation unit tests and
the existing native hardening suite.

## 2. Why It Matters (Impact & Implications)

Users no longer have to discard supported unit metadata to use native conversion.
Scale and multiplier remain metadata: a parameter with value 2 stays 2 even when
its unit denotes hours rather than seconds. This preserves source meaning without
claiming an automatic quantity converter or complete dimensional certification.

### Exact acceptance boundary

- Primitive kinds: `mole`, `second`, `litre`, `metre`, `dimensionless`. Verified
  aliases: `moles`, `seconds`, `litres`, `liter`, `liters`, `metres`, `meters`.
  Singular `meter` is intentionally excluded from this slice.
- `unit NAME = ...` supports flat products/quotients, named aliases, and nonzero
  integer source powers with absolute value at most **16**. Expanded powers are
  bounded to absolute **64**, with at most **5** distinct primitive components.
  Repeated kinds, even `second/second`, reject rather than silently cancel.
- A positive leading coefficient is accepted only on a direct primitive first
  factor with exponent +1. At most **15 significant digits** and normalized
  decimal scale **[-12,12]** are accepted. `1/NAME` is also supported. Parentheses,
  internal coefficients and fractional/zero/oversized powers are deferred.
- Precise distinction: `unit small=0.001 second; unit square=small^2;` is accepted;
  the already-scaled alias retains its component scale and power. A **new leading
  coefficient on an alias**, such as `unit another=2 small`, is deferred. Likewise
  `2 second^2` and `2*second^2` are rejected: their upstream mappings differ.
- `PARAMETER has NAME` and `COMPARTMENT has NAME` preserve canonical unit refs.
  Three-dimensional compartments require volume dimensions or explicit
  dimensionless units. Litre and cubic metre have the same dimensions but values
  are **not converted** between them.
- Species `has` is deferred because it denotes concentration and requires a
  reference-backed concentration-to-substance transformation. Reaction annotations,
  inline quantities/unit expressions, custom identifiers ending in `s`, and model
  defaults `substance`, `volume`, `area`, `length`, `extent`, `time_unit` reject.
  Ordinary names such as `volume_unit` are not those special default names.
- Unknown/cyclic unit references, redefinitions, repeated annotations and
  unit/model-symbol collisions fail closed. Some are accepted by upstream;
  these are explicitly stricter native-profile restrictions, not upstream syntax claims.

### Reviewed dimensional and safety boundaries

- Bare arithmetic numbers and unannotated symbols stay unspecified. Known
  addition/subtraction conflicts are detected even below an otherwise unknown
  expression. Known multiply/divide dimensions combine with checked integer
  arithmetic; a known non-dimensionless exponent rejects. Other powers remain
  conservatively indeterminate rather than inventing dimensional knowledge.
- An initialization target is checked against its expression only when both are
  known. Units are not inferred through unannotated intermediate entities. Literal
  initialization does not imply dimensionless units. Passing an indeterminate
  expression is not a consistency proof; the partial-check warning remains visible.
- Definition dependencies use iterative ordering, not recursive expansion.
  Unit definitions share the model-entity budget; expansion and exponents have
  fixed bounds. The owner tests a 10,000-alias chain and cycle; independent review
  adds a 65-definition chain/budget check and 4 threads × 12 isolated models.
- Components retain upstream order and definitions retain first-reference order;
  there is no algebraic simplification merely to make projections agree.
- Public unit/model fields remain private with immutable views. Native runtime
  remains free of Python/C++, filesystem/network/process calls and unsafe code;
  explicit example I/O and the isolated development oracle remain separate.
- Earlier initialization, grouping, numeric-expression precision/range and
  unsupported-language restrictions remain. No solver, formatter, amount
  initialization, compartment expressions, rules/events/functions or imports added.

Reference provenance remains qualified: Antimony 3.2.0 wheel hashes are pinned,
independent libSBML is 5.21.2, but release-tag/research SHAs do **not** establish the
wheel build's source SHA. No numerical or biological validity claim is made.

## 3. Test Results (Verified, Not Assumed)

Independent stable run: Rust/Cargo 1.96.0, CPython 3.12.12, macOS 15.5 arm64.
Executable SHA-256:
`599ac6f9a51074174eb0ec57ec9c0196e9b61c58fce11fdf9529b22676b18656`.

| Exact command | Observed result |
|---|---|
| `cargo fmt --all --check` | Pass |
| `cargo check --locked --all-targets --all-features` | Pass |
| `cargo build --locked --all-targets --all-features` | Pass |
| `cargo test --locked --all-targets --all-features` | **111 integration passed**: 22 foundation + 33 slice + 18 initialization + 8 hardening + 19 units + 11 independent units review; no failures/ignored |
| `cargo test --locked --doc --all-features` | **1 passed** |
| `cargo test --locked --no-default-features` | **105 integration + 1 doctest passed**; 6 export-only tests not compiled |
| `cargo clippy --locked --all-targets --all-features -- -D warnings` | Pass |
| `RUSTDOCFLAGS='-D warnings' cargo doc --locked --no-deps --all-features` | Pass |
| `cargo build --locked --offline --all-targets --all-features` | Pass with already-fetched dependencies |
| `.venv-oracle/bin/python -m unittest discover -s tools/oracle -p 'test_*.py' -v` | **67 passed**, no failures/skips |
| `.venv-oracle/bin/python tools/oracle/corpus.py check` | **46 reference replays passed** |
| `.venv-oracle/bin/python tools/oracle/native_check.py --native target/debug/examples/export_sbml --trusted-native` | **23 exact structural matches + 23 required rejections**; 0 failed/blocked; **46 numerical NOT_RUN** |
| Same native-check command with `--python /no/such/python` | Expected exit 2: **46 blocked**, no exported/rejection passes |
| `.venv-oracle/bin/python -m py_compile tools/oracle/*.py` | Pass |
| `cargo package --list --allow-dirty` | 149 entries; no caches, virtualenv, target or artifact paths |

The independent inspector now uses `getExponentAsDouble()`; its real fractional
power regression checks 0.5 instead of truncating to an integer. Existing 28
reference snapshots were retained; 18 unit fixtures add seven accepted exports
and eleven explicit rejections. Skips or rejection passes are not unit support.
The 23 exported documents retain **201 independent SBML warnings**, not a
dimensional validation certificate. The six configured remote CI jobs remain **NOT_RUN**. Repeated configurations,
loops and subcases are assertions, not additional unique test counts.

### Frozen independent probe matrix

`tests/units_review.rs::frozen_units_review_probe_acceptance` stores all 18 original
sources. On the stable binary, **14 accepted sources exactly matched independent
reference projections; four rejected explicitly**, with zero accepted mismatch.

Accepted probe names: `scale_low`, `scale_high`, `scale_decimal`,
`scale_leadingzeros`, `scaled_alias_power`, `composite_inverse`, `resolved_limit`,
`dimensionless_component`, `order_annotation`, `multi_quotient`,
`quotient_negative`, `volume_dimensionless`, `unit_only`, `unknown_exponent`.
Rejected: `resolved_excess` (resource_limit), `annotated_builtin_symbol`
(declaration_conflict), `known_nested_mismatch` and `dimensioned_exponent`
(units_inconsistency). These complement, rather than replace, the checked corpus.

Reproduce the independent projection comparison using each tracked accepted
source and the explicit Python APIs `native_check.native_export(executable, text)`,
`oracle.run('convert', text)`, `oracle.run('inspect', native['xml'])`, then
`native_check.differences(reference['projection'], inspection['projection'])`.
Require PASS inspection/reference and an empty differences array; do not compare
raw XML or normalize operator trees. Imports are not in these trusted inputs.

Final status consistency checked **33 local Markdown links/anchors**, all **26
frozen feature IDs** (13 inclusions/13 exclusions), evidence paths, the unchanged
release target (`null`), and the 149-entry package inventory. The README scaled-unit
example exported offline and passed independent trusted SBML inspection.

An early reviewer helper failed compilation because SourceFile construction
returns LimitExceeded, not Diagnostic. The helper was corrected; all stable final
commands above were rerun. No core defect remains open from this bounded review.

## 4. What's Next (Concrete Next Steps)

Stop at this verified units increment. A future separately scoped change should
first establish species concentration/substance and model-default mappings with
executable reference cases. Full-profile grammar/rejections, formatting, selected
numerical comparison and maintainer/release gates remain incomplete.

## 5. Novel Opportunities (Innovation & Monetization)

A small auditable native converter with explicit knowledge/uncertainty states can
serve Rust model editors and validation pipelines without embedding a reference
runtime. Broader interoperability depends on transparent coverage—not branding a
partial checker as SI certification. No publication or monetization action taken.

## Key Learnings:

1. Unit annotations preserve metadata; dimensional equality never authorizes numeric rescaling.
2. A power of an already-scaled alias differs from adding a new coefficient to an alias.
3. Exact independent projections need floating-point unit exponents even when native support is integer-only.
