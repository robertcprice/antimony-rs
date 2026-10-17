# Executable development oracle — Phase 1

This is an **upstream reference harness**, not a native parser or a simulation
engine. No Rust runtime dependency invokes it. This provisional local target
still needs maintainer review; it is not an official compatibility designation.

## Pin and provenance

Executed on CPython 3.12.12, macOS 15.5 arm64:

- Antimony **3.2.0**, library reports `v3.2.0`; its generated XML reports bundled
  libSBML **5.21.1**.
- Independent `python-libsbml` inspector **5.21.2**. Its registered packages and
  XML-library build versions are recorded in `tools/oracle/environment.json`.
- Exact downloaded wheel URLs and verified SHA-256 digests are recorded in
  `tools/oracle/wheels.json`; installation used hash-required requirements.
- Official release tag `v3.2.0` resolved to
  `a4cb7549a03beef569ce21a0a7342e315ca0ed43` via the
  [GitHub tag API](https://api.github.com/repos/sys-bio/antimony/git/ref/tags/v3.2.0).
  This is **not independent proof of the wheel build's source SHA**.
- The previous research SHA
  `f3f5c6b7f3f22ab48f8e382a9318ef50bce2b4b2` is distinct. Wheel/source mapping
  remains explicitly unverified, rather than silently equating these revisions.

Published artifact sources: [Antimony 3.2.0](https://pypi.org/project/antimony/3.2.0/)
and [python-libsbml 5.21.2](https://pypi.org/project/python-libsbml/5.21.2/).
No upstream fixture or parser code was copied. Fixture licensing remains subject
to the project's undecided license; no public redistribution approval is implied.

## Reproduce

For this exact macOS arm64 / Python 3.12 pin (other platforms need separately
reviewed wheel hashes, not silently substituted artifacts):

```sh
uv venv --python /opt/homebrew/bin/python3.12 .venv-oracle
uv pip install --python .venv-oracle/bin/python --only-binary :all: \
  --require-hashes -r tools/oracle/requirements-macos-arm64-py312.txt
python3.12 tools/oracle/oracle.py probe
python3.12 -m unittest discover -s tools/oracle -p test_oracle.py -v
python3.12 -m unittest discover -s tools/oracle -p test_contracts.py -v
python3.12 -m unittest discover -s tools/oracle -p test_integration.py -v
python3.12 tools/oracle/corpus.py check
```

Installation is explicit, isolated and optional; ordinary Cargo tests never
install/download/import the reference packages. The harness orchestration and
unit tests use only Python's standard library. Integration tests require the
pinned virtualenv; unavailable dependencies **fail**, never skip/pass.

```sh
python3.12 tools/oracle/oracle.py convert --trusted fixtures/accepted/first_conversion.ant
python3.12 tools/oracle/oracle.py inspect --trusted artifacts/native-first.xml
```

`convert` emits reference SBML, semantic projection, and full categorized libSBML
messages. `inspect` independently parses native-exported SBML through the same
projection; compare the `projection` objects, **not XML strings**. No numerical
simulation, RoadRunner installation, or biological-validation claim is made.

## Safety and failure contract

Every input runs in a fresh subprocess with `-I`, a temporary working directory,
a 30-second timeout, no shell, and a 4 MiB source bound. This is isolation, **not
an OS security sandbox**. Antimony can follow imports; never pass untrusted input
to the reference. Arbitrary `convert`/`inspect` paths require explicit `--trusted`.
Corpus runs accept only owned, source-hash-verified fixtures under `fixtures/`
and reject even a textual `import` occurrence as defense in depth. DTD/entity
inputs are refused before independent XML parsing. No network is performed by
the harness itself; native parsing must separately prohibit all import I/O.

Statuses are not conflated: `ORACLE_BLOCKED` (missing/mismatched reference),
`ORACLE_ERROR` (crash, timeout, bad worker protocol), `REFERENCE_REJECTED` (upstream
syntax/semantic rejection), `SBML_INVALID` (independent errors),
`PROJECTION_UNSUPPORTED` (data outside the projection vocabulary),
`INPUT_REJECTED`, and `PASS`. Non-PASS CLI results exit 2. A load result of zero
is success; actual observed conversions returned one.

Projection v1 is deliberately narrow: SBML L3V2 Core, the tested entity/initial
assignment/rule/event/unit constructs, arithmetic and the tested `sin`/`gt`/time
math nodes. Unknown core elements/attributes, non-Core namespaces, annotation,
notes, species-reference metadata, stoichiometry math, and unknown MathML/csymbol
forms fail closed rather than disappearing from a comparison. This is **not** a
general SBML-equivalence checker. Metaids, XML formatting, and provenance comments
are intentionally excluded; names and SBO terms on represented entities remain.
No expression simplification, reordering, or numerical equivalence is assumed.

libSBML installs an internal `l3v2extendedmath` plugin even for plain Core input.
That is not an XML package declaration. Package rejection uses the actual XML
namespaces, not merely `getNumPlugins()`.

## Corpus and observed semantics

`fixtures/manifests/original-v1.json` pins each original source SHA, feature IDs,
profile expectation, provenance, reference status, and expectation path.
`*.expected.json` were generated **only by successful real reference executions**.
`corpus.py record` is an explicit reviewed regeneration operation; it does not
run on ordinary tests. Both recording and checking enforce intended reference
status, and writes are restricted to `fixtures/manifests/`.

The initial twelve cases passed reference replay: **11 upstream-accepted models and
one malformed-input rejection**. Included are first_conversion, both arrows,
non-unit/default compartments, `$` and `const` boundary forms, missing values,
precedence, assignment rules, events, and a function call. The latter three are
valid full-language constructs but outside the proposed native profile. Missing
values are accepted upstream but intentionally rejected by the strict target.
These are not 12 native-supported models; native status is `NOT_RUN` in this gate.

Verified directly:

- `->` is reversible; `=>` is irreversible.
- `$S` and `const species S` mean boundary=true, constant=false.
- Missing species/parameter initial values remain unset; default compartment=1.
- In a volume-3 compartment, `S=2` remains concentration 2, not amount 2.
- Unary minus is weaker than power; power associates right (`-S^2`, `2^(3^2)`).
- XML may contain n-ary multiplication, but independent libSBML's AST projects it
  as left-associated binary multiplication. The projection retains that tree.
- first_conversion yields L3V2 with zero errors and **12 warnings**. IDs
  99505/99506/99507/99508/20513/20616/20702 concern unspecified or unverifiable
  units; 80701 recommends parameter units. These are preserved and categorized,
  not suppressed. The fixture is not dimensionally proven.

Phase 1 verification: 22 standard-library unit/contract tests, 8 real integration tests, and all
12 initial corpus replay cases passed with zero skips/failures. Python byte compilation
also succeeded. Native build/test evidence belongs to the native implementation
report, not this upstream gate. Next: use this inspector for the first native
export, expand corpus to all profile feature families, audit additional projection
forms before accepting them, and obtain maintainer agreement on the release pin.

## Versioned evidence contract

`compatibility.schema.json` defines `antimony-rs-compatibility-v1` result records.
Every case carries `profile_id`, fixture/source hash, the existing profile's
frozen `F01`–`F13`/`X01`–`X13` IDs, reference status, and separate native syntax,
resolve, export, and numerical stages. All native stages remain `NOT_RUN` in
this upstream-only runner. Semantic subcase names remain `semantic_tags`, not
competing feature IDs. `contracts.py` validates the corpus before any reference
execution and validates each emitted compatibility record. Unknown features,
malformed hashes, duplicate fixtures/tags, missing fields/stages and unknown
statuses fail closed. The profile's native implementation status is not changed
by an upstream reference success. The frozen ID set is checked against the
existing profile; changing it requires reviewing/versioning this contract.

## Native differential check (Phase 2)

Build the example explicitly, then invoke the trusted prebuilt program. The
checker never invokes Cargo or installs anything itself:

```sh
cargo build --example export_sbml
python3.12 tools/oracle/native_check.py \
  --native target/debug/examples/export_sbml --trusted-native
# Optional bounded first case:
python3.12 tools/oracle/native_check.py \
  --native target/debug/examples/export_sbml --trusted-native \
  --fixture first_conversion
```

The runner uses each owned fixture in a temporary directory, obtains a fresh
upstream result, verifies that result against the reviewed snapshot, invokes the
native CLI, independently parses/validates its XML, and compares projections.
Its SHA-256 identifies the exact native executable tested. Structural mismatches
include field paths. No algebraic simplification or reordered expressions are
allowed. Scalar numbers with equal values compare equal (`3` versus `3.0`),
but distinct initialization fields, AST types, flags and absent fields do not.

CLI contract: success exits 0 and writes XML to the second path argument;
diagnostic rejection exits 2 with `stage=syntax|resolve|export code=CODE
message=TEXT` on its first stderr line. Codes map to the native structured
DiagnosticCode families. Unknown diagnostics, crashes, timeouts, missing output,
or an output artifact left on rejection fail the check. Native syntax/resolve/
export stages are reported separately; the export stage includes independent
SBML validity **and** reference semantic agreement. Expected rejections are
counted separately from supported exports. Numerical simulation stays `NOT_RUN`.

### Executed Phase 2 evidence

The binary-specific result in `tools/oracle/native-evidence.json` records the
UTC execution time, tested executable SHA-256, manifest SHA-256, package pins,
per-case stages, retained warning IDs, and exact command. It is an execution
record, **not** a golden result generated by the native implementation.

Historical Phase 2 checkpoint: **8 native-supported exports matched reference projections,
5 expected explicit rejections, 0 failures/blocked, 13 numerical NOT_RUN**.
The eighth positive case is scientific notation. Audited MathML `sep` support
allows e-notation inspection while preserving AST_REAL versus AST_REAL_E types;
no broad scalar/AST or expression-association normalization was added. The
native writer was corrected to match observed upstream literal serialization.

At that Phase 2 checkpoint, the complete Python suite passed **43 tests** (34 unit/contract/checker tests,
9 real-reference integration tests), zero failures/skips. All **13** reference
fixtures replayed successfully, and Python byte compilation passed. The previous
Phase 1 counts above remain historical. The full native differential output and
test log are kept under ignored `artifacts/`. Rerun after any native or oracle
change; an old executable hash is not evidence for a newly built binary.

## Arithmetic-initialization reference increment

Fifteen additional original fixtures extend the corpus from 13 to 28 without
changing the prior sources/goldens. These establish a bounded initialization
contract before native differential results are claimed:

- `a=2+3` preserves an InitialAssignment expression; parameter `value` is unset.
  The same applies to species initialization (both concentration and amount
  attributes unset). Expressions are not folded into scalar output attributes.
- Bare `-2` is a scalar initial value, but `+2`, `(3)` and `(-2)` are initial
  assignments even when their MathML reduces to a single numeric node.
- Forward references and acyclic chains are accepted. The reference orders
  assignments by **species, then compartments, then parameters**; within each
  type it retains first symbol occurrence, including declarations and reaction
  participants. This is not dependency/topological order.
- A parameter initialized from an ordinary species is inferred `constant=false`;
  this propagates through parameter chains. A boundary-species dependency yields
  `constant=true` by default. Species' own `boundaryCondition` and `constant`
  flags remain independent. Fixed compartment references are also tested.
- Reaction cycles are valid. Self/mutual initialization dependency cycles are
  upstream semantic rejections, represented by `semantic_rejected`, not called
  malformed syntax. Native tests require a resolve-stage semantic diagnostic.
- Upstream preserves division-by-zero, non-finite-result arithmetic and invalid
  real-power-domain expressions as unevaluated initial assignments. The native
  strict profile must reject them during resolution, not claim upstream rejected
  the source. Unknown dependencies may similarly remain unset upstream.
- Compartment arithmetic initialization is valid upstream (`size` unset plus an
  InitialAssignment), but explicitly deferred by this bounded native increment.
  It is a rejection fixture, not silently replaced with a numeric size.

All 28 reference replays passed. The expanded Python suite has 54 passing tests
(35 unit/contract/checker and 19 real-reference integration), zero failures/skips.
The stable-binary initialization differential passed **16 exact native/reference
exports and 12 expected rejections**, with zero failures/blocked results and all
28 numerical stages `NOT_RUN`. `native-evidence.json` now records the tested
binary SHA-256, timestamp, manifest hash, and per-case stages. This remains
binary-specific development evidence; rerun after code or oracle changes.

## Bounded units reference contract

Unit discovery used the official [release tutorial](https://github.com/sys-bio/antimony/blob/a4cb7549a03beef569ce21a0a7342e315ca0ed43/doc/AntimonyTutorial.md#units),
[grammar](https://github.com/sys-bio/antimony/blob/a4cb7549a03beef569ce21a0a7342e315ca0ed43/src/antimony.ypp),
and [unit conversion code](https://github.com/sys-bio/antimony/blob/a4cb7549a03beef569ce21a0a7342e315ca0ed43/src/unitdef.cpp),
then executed the installed pinned release. Source-tag/wheel provenance remains
qualified as above. Eighteen additional original unit fixtures were executed
before recording expectations; the preceding 28 expectations were checked,
not blindly regenerated. The reference corpus now has 46 cases.

The bounded native target is named definitions built from a verified primitive
catalog, flat products/quotients, bounded integer powers, unscaled alias expansion,
and a positive coefficient on a direct primitive first factor with exponent one.
Only parameter/compartment `has NAME` annotations are included; unsupported forms
must reject explicitly. Native API/status documents govern the exact final limits.

Executed findings that constrain implementation:

- `0.002 mole` becomes multiplier 2, scale -3; `3600 second` becomes multiplier
  3.6, scale 3. Component order follows the expression; definition order follows
  first occurrence, including forward aliases. Values and compartment sizes are
  **not rescaled** by unit annotations.
- Species `has` describes concentration, not directly SBML `substanceUnits`:
  `S has mole` in a litre compartment generates the substance unit `mole_litre`.
  Species annotations are therefore explicitly deferred, not copied incorrectly.
- Definitions called `time_unit`, `substance`, or `volume` also set model-default
  attributes; `area`, `length`, and `extent` also affect defaults. These six
  names are deferred. By contrast, `substance_unit`, `volume_unit`, `area_unit`,
  `length_unit`, and `extent_unit` are ordinary custom IDs in the tested release.
- `2*metre^2` and `2 metre^2` differ: the former emits multiplier sqrt(2) on an
  exponent-2 component, the latter multiplier 2. Scalar/power combinations,
  internal coefficients, and parenthesized unit expressions are not in this slice.
- Repeated primitive kinds are preserved as separate upstream components, even
  `second/second`. The bounded native grammar rejects these rather than applying
  unverified cancellation. Fractional powers are valid upstream but deferred.
- The inspector uses `getExponentAsDouble()`: the old integer getter would truncate
  fractional powers. A real reference regression now asserts exponent 0.5.
- Unknown and cyclic unit references can become empty upstream UnitDefinitions.
  They are not dimensionless and the native strict profile rejects them. Upstream
  permits redefinition and certain dimensional inconsistencies; stricter native
  rejections are classified separately from upstream acceptance.
- Verified direct aliases include `moles`, `seconds`, `litres`, `liter`, `liters`,
  `metres`, and `meters`. Singular `meter` direct annotation is rejected by this
  pinned release's export validation, despite working inside a definition; it is
  deliberately excluded. Custom names ending in `s` may singularize upstream and
  are deferred rather than silently renamed.

The unit fixtures contain both known-consistent and provably conflicting arithmetic,
unknown-unit cases, scale/multiplier/component-order checks, forward aliases, and
explicitly deferred features. This is not general SI dimensional certification;
bare/unspecified dimensions must stay unknown rather than become dimensionless.
Numerical simulation remains `NOT_RUN`.

### Executed units increment evidence

After the native owner signalled a stable binary, the complete gate passed:
**67 Python tests** (35 unit/contract/checker and 32 real-reference integration),
**46 reference replays**, and **23 exact native/reference exports plus 23 expected
explicit rejections**. There were zero failures, skips, or blocked cases. All
46 numerical stages remain `NOT_RUN`; Python byte compilation also passed.

`tools/oracle/native-evidence.json` now records this units-increment execution,
including the UTC timestamp, stable executable and source-manifest hashes, exact
commands, per-case stages, diagnostics, and retained libSBML warning IDs. Earlier
phase counts above are historical. This is binary-specific evidence for the
bounded subset, not complete Antimony/SBML compatibility, complete dimensional
analysis, simulator validation, or an upstream endorsement.

## Species/default units: executed release contract (Phase A)

The original corpus now includes 25 additional species/default-unit fixtures.
The two previously deferred `units_species_annotation` and
`units_model_defaults` cases retain their exact source paths, hashes and goldens;
only their native expectation changes. `fixtures/manifests/species-units-migrations.json`
records that transition. Directory names are historical provenance, not support
claims. The manifest remains authoritative.

Pinned release source reviewed: [module.cpp](https://github.com/sys-bio/antimony/blob/a4cb7549a03beef569ce21a0a7342e315ca0ed43/src/module.cpp)
(`AddOrFindUnitDef`, finalization), [module-sbml.cpp](https://github.com/sys-bio/antimony/blob/a4cb7549a03beef569ce21a0a7342e315ca0ed43/src/module-sbml.cpp)
(unit redirects and species export, approximately lines 2310–2470), and
[unitdef.cpp](https://github.com/sys-bio/antimony/blob/a4cb7549a03beef569ce21a0a7342e315ca0ed43/src/unitdef.cpp)
(`Reduce`, `MultiplyUnitDef`, `ComponentsMatch`, `GetCanonical`). The release tag
is still not an independently attested wheel-build commit. All new goldens were
executed using the pinned wheels, not inferred from this source.

Observed behavior is release-specific, not an algebra-normalization policy:

- Species `has` is concentration metadata. Export multiplies its **raw** unit
  components by the compartment annotation; an implicit compartment uses literal
  litre even when the model volume default is scaled or dimensionless.
- Exact inverse components are removed only when the initial raw component count
  exceeds two. Reduction precedes alias expansion. Thus direct `mole/litre`
  times litre becomes mole, whereas an alias of that concentration can retain
  three expanded components. A two-component reciprocal/product pair remains.
- Scales/multipliers must match for cancellation. No concentration, parameter or
  volume numeric value is automatically rescaled. Generated identifiers combine
  source unit names, including upstream trailing-`s` singularization.
- Existing component-matching user definitions may be reused without changing
  their component order. Generated definitions follow original definitions in
  first-species occurrence order; generated IDs are not sorted or renumbered.
- The six special declarations are `substance`, `volume`, `area`, `length`,
  `extent`, `time_unit`. Canonical singleton builtins replace special aliases;
  otherwise a canonical component match to an independently declared ordinary
  unit redirects the default and removes the special definition. Ordinary aliases
  remain. Entity attributes use redirects, but generated names use source names.
- Model defaults alone do not materialize per-species `substanceUnits` or
  per-compartment `units` attributes. Missing attributes remain missing.
- Upstream permits conflicting defaults and repeated annotations and can silently
  overwrite a generated-name collision (including deleting a parameter). These
  behaviors are **not** permission for native silent data loss: strict rejection
  fixtures retain the actual accepted upstream projection.

### Invalid upstream export is not semantic equivalence

An explicit compartment without unit annotation can cause Antimony 3.2.0 to emit
an undefined species substance-unit reference (`mole_volume` or `conc_volume`).
Independent libSBML reports `SBML_INVALID`, error 10313. Two reviewed fixtures
preserve that status, projection and error classification as evidence. They can
only have manifest expectation `strict_profile_rejected` and can earn only an
**expected native rejection**, never supported-export or projection-match credit.
The checker does not compare native XML against invalid upstream XML. Native
acceptance fails, unexpected invalid reference fails, and oracle unavailability
remains `ORACLE_BLOCKED`; tests cover all these distinctions. Replay `PASS` means
an expected invalid-reference observation was reproduced, not that its SBML is
valid. Numerical stages remain `NOT_RUN` throughout.

Additional discovery probes (ignored local artifacts) found upstream process
errors for generated-name collisions with species/reaction names. They are not
converted into accepted reference expectations; native-only collision tests are
the appropriate safety gate for those cases.

The dimensionless model-volume override with an implicitly compartmented,
explicitly annotated species is a deliberate stricter native rejection: the
upstream literal-litre conversion creates a provable source-versus-export
concentration-dimension contradiction. Its real upstream `PASS` golden is
retained; no source rewrite or algebraic repair is used to claim equivalence.
Scaled litre overrides of the same dimension preserve scale metadata without
numeric rescaling. The Phase A corpus expectations are 40 supported exports and
31 explicit rejections (including the two invalid-reference observations),
pending the stable native gate.

A further live regression probes all six special names assigned a bare numeric
value without `unit` (for example `volume=2`). Upstream rejects each as an invalid
unit definition, not an ordinary parameter initialization. Native must not silently
reinterpret these reserved model-unit declarations as numeric symbols.

Phase A stable rerun: **87 Python tests, 71 reviewed-reference replays, 40 exact
native exports and 31 required native rejections passed**, with zero failures,
skips or blocked cases; all 71 numerical stages remain `NOT_RUN`. Binary SHA-256
`edffb516113c6761af1a0eaa2e58d6e40e12272a083004eb8f7d0bb96a258d8e`
was unchanged during comparison. `tools/oracle/native-evidence.json` includes
per-case flags, retained invalid-reference error IDs and timestamp/manifest hash.
This is a binary-specific development gate, not general dimensional certification.

## Native formatter roundtrip gate (Phase B)

Build the optional examples explicitly, then run:

```sh
python3.12 tools/oracle/roundtrip_check.py \
  --formatter target/debug/examples/format_antimony \
  --native target/debug/examples/export_sbml --trusted-native
```

The stdlib harness executes trusted prebuilt programs with explicit input/output
path arguments, a temporary working directory and timeout. No shell, Cargo,
installation, network or simulator is invoked. This is not a security sandbox.
Only hash-pinned owned fixtures are selected; generated input containing textual
`import` is rejected before upstream conversion. Formatter output is limited to
4 MiB, UTF-8, nonempty. Diagnostics use the same explicit exit-2 protocol as the
exporter; `export` on the formatter means text serialization, **not SBML**.

Each original input must first pass the existing reviewed-reference/native gate.
For each supported input the harness then requires **all** of:

1. Native formatting succeeds, and a second formatting is exactly byte-identical.
2. Pinned upstream conversion of the formatted text yields valid, supported SBML
   with the exact original reviewed semantic projection.
3. Native parsing/resolution/export of formatted text yields SBML that passes
   independent libSBML inspection and the same exact original projection.

No expression reassociation, unit simplification, ID renaming, order sorting or
additional projection exclusions are introduced for formatting. Existing XML
noise handling is unchanged. Golden fixtures are not rewritten by the harness.
For rejected input, the formatter must emit an appropriate classified rejection
and **no output artifact**, including the two known invalid-reference cases; such
cases receive no idempotence or semantic-roundtrip credit. Missing reference
execution blocks even negative cases. Original and formatted warnings are retained.
Both executable hashes, the manifest hash and per-source hashes are recorded;
a binary or manifest change during the check fails the aggregate gate. All
numerical stages remain `NOT_RUN`. Phase B evidence is separate from Phase A.

### Scalar boundary safety evidence discovered during formatter review

Independent review found an inherited scalar-export defect below the normal
range: `a=1e-308` produced invalid native SBML, while the pinned reference follows
an unrepresented MathML/unit path (`PROJECTION_UNSUPPORTED`). `1e-999` similarly
must not silently underflow to zero. Even the IEEE minimum-normal literal
`2.2250738585072014e-308` is rejected by the pinned reference during SBML creation.
No inspector vocabulary was expanded to conceal these differences. Four live
regressions preserve these classifications and verify that `1e-307` and `1e308`
work in parameter (including negative), species concentration, compartment size
and reaction stoichiometry scalar roles. Native range rejection remains an
explicit safety boundary, not a claim of full upstream numeric coverage.

Review also exposed a pre-existing scalar precision mismatch: the reference
serializes `1.234567890123456789` to `1.23456789012346`,
`1.0000000000000002` to `1`, and `1000000000000001` to `1000000000000000`.
The native full-f64 export formerly differed. Two original executed-reference
fixtures now require an explicit unsupported-precision native diagnostic, and a
live regression preserves the observed values. No comparator tolerance or
rounding was introduced. The earlier 71 goldens remain unchanged; the corpus is
now **73 cases (40 supported, 33 required rejection)**. The conservative native
scalar range is zero or absolute value `[1e-307, 1e308]`, with a bounded precision
policy rather than silent rounding or underflow.

Phase B final stable gate: **110 Python tests and 73 reference replays passed**;
all **40 native exports matched**, all **40 formatted inputs matched through
both upstream and native SBML paths**, all **40 were byte-idempotent**, and all
**33 required rejections** were classified and artifact-free. Zero failures,
skips or blocked cases; 73 numerical stages remain `NOT_RUN`. The two binaries
and source manifest were unchanged during the formatter gate:

- formatter SHA-256: `2d4382de44541b8ae4c73e1a62e549db774b42b26e1423e2553f5ae602818faf`
- exporter SHA-256: `3cc56ad55635a6a4407944055186e7462c13907723b8f62af97537336dd83dcb`

`tools/oracle/formatter-evidence.json` records timestamp, both binary hashes,
manifest/checker hashes, each original/formatted/second-formatted source hash,
per-path statuses and retained warning/error IDs. `native-evidence.json` records
the same final export gate. Full local process reports remain ignored artifacts;
tracked summaries assert only the executed bounded contract, not simulation or
full-language conformance.
