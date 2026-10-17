# Independent species and model-default units review (stage A)

Reviewed 2026-10-10 UTC. **Gate: PASS for the bounded stage-A subset.**
Subset: `species-default-units-flat-v0.1-slice`. This is not full Phase 3,
full unit algebra, dimensional certification or completion of the target profile.
Canonical formatting is the separately gated stage B, not part of this A verdict.

## 1. What We Built (Plain English)

Species can now retain a source concentration-unit annotation while exporting the
separately derived SBML substance-unit reference. Six special unit declarations
set model defaults without copying inherited metadata onto individual entities.
Read-only model queries keep source annotations, emitted substance references,
model defaults and dimensional knowledge distinct. Eleven independent reviewer
tests supplement twenty-two native implementation regressions.

## 2. Why It Matters (Impact & Implications)

A concentration annotation cannot simply be copied into SBML substanceUnits.
This increment implements the pinned reference's actual transformation—including
alias identity, component order and definition reuse—without multiplying species
values by compartment volume or converting scaled quantities behind the user's back.

### Reviewed mapping and rejection boundary

- Source `S has mole` with a litre compartment emits `mole_litre`, not `mole`.
  A direct `mole/litre` concentration definition can cancel the volume factor;
  a scaled concentration retains scaled substance metadata and unchanged numbers.
- Alias identity matters before expansion. `unit conc=base; unit base=mole/litre`
  does not necessarily produce the same component list as direct `conc=mole/litre`.
  Only the audited raw-component inverse-pair reduction is applied. No broad
  algebraic normalization or SI conversion is performed.
- Matching existing definitions may be reused, retaining their own component order.
  Generated definitions follow user definitions, in first species-occurrence order,
  including references before declaration. Same generated identity is reused.
  Name collisions with different definitions or model symbols reject rather than
  reproduce upstream overwrite/crash behavior.
- Special names are `substance`, `volume`, `area`, `length`, `extent`, `time_unit`.
  Primitive/default aliases redirect according to reference export; ordinary aliases
  retain identity. Missing defaults stay absent. Parameters do not inherit arbitrary
  substance/time defaults. Explicit entity units override inherited defaults.
- Species source annotations are queryable separately from emitted substance refs.
  Unannotated species gain known concentration dimensions only with sufficient
  substance and compartment/default-volume knowledge. Otherwise they remain unknown;
  no default of dimensionless is invented. Known contradictions reject.
- An explicit compartment without an explicit unit annotation can cause upstream
  export to reference a missing generated unit. Such cases reject; native does not
  invent a definition to make invalid reference output appear valid.
- Upstream implicit species conversion uses literal litre even with a model-volume
  override. `unit volume=dimensionless; species S; S=2; S has mole` is rejected when
  source and emitted concentration dimensions conflict. A valid upstream parse is
  not automatically acceptance by this stricter native profile.
- All six default names are reserved against numeric model-entity reinterpretation:
  bare `volume=2` and analogous constructs reject, matching live reference rejection.
- Generated units share the entity budget and have at most ten preserved components;
  source definitions retain the earlier five-component, power and coefficient bounds.
  Unit/model state remains private and immutable; runtime remains native, with no
  hidden filesystem/network/process/Python calls. Example I/O remains explicit.

Earlier syntax/arithmetic/initialization restrictions remain; amount initialization,
compartment expressions, rules/events/functions, imports and general unit algebra
are still outside the accepted subset. Successful checks are not a certificate.

## 3. Test Results (Verified, Not Assumed)

Local Rust/Cargo 1.96.0, CPython 3.12.12, macOS 15.5 arm64.
Stable executable SHA-256:
`edffb516113c6761af1a0eaa2e58d6e40e12272a083004eb8f7d0bb96a258d8e`.

| Exact command | Independently observed result |
|---|---|
| `cargo fmt --all --check` | Pass |
| `cargo check --locked --all-targets --all-features` | Pass |
| `cargo build --locked --all-targets --all-features` | Pass |
| `cargo test --locked --all-targets --all-features` | **144 integration passed**: previous 111 + 22 implementation + 11 reviewer tests; 0 failures/ignored |
| `cargo test --locked --doc --all-features` | **1 passed** |
| `cargo test --locked --no-default-features` | **137 integration + 1 doctest passed**; 7 export-only tests not compiled |
| `cargo clippy --locked --all-targets --all-features -- -D warnings` | Pass |
| `RUSTDOCFLAGS='-D warnings' cargo doc --locked --no-deps --all-features` | Pass |
| `cargo build --locked --offline --all-targets --all-features` | Pass with cached dependencies |
| `.venv-oracle/bin/python -m unittest discover -s tools/oracle -p 'test_*.py' -v` | **87 passed**, no failures/skips |
| `.venv-oracle/bin/python tools/oracle/corpus.py check` | **71 reference replays passed** (66 valid, 3 upstream rejections, 2 audited invalid-SBML negative cases) |
| `.venv-oracle/bin/python tools/oracle/native_check.py --native target/debug/examples/export_sbml --trusted-native` | **40 exact export matches + 31 required rejections**; 0 failed/blocked; **71 numerical NOT_RUN** |
| Same native-check command with `--python /no/such/python` | Expected exit 2; **71 ORACLE_BLOCKED**, no native passes |
| `.venv-oracle/bin/python -m py_compile tools/oracle/*.py` | Pass |
| `cargo package --list --allow-dirty` | Pass; 205 entries, no caches/virtualenv/target/artifacts |

A status consistency verified 33 local Markdown links/anchors, 26 frozen feature
IDs and evidence paths. The scaled-species README sample exported offline and passed
independent trusted SBML inspection.

The forty exports retain **227 independent SBML warnings**, not dimensional
certification. All six configured remote CI jobs remain **NOT_RUN**. Release-tag
and research SHAs do not prove the pinned wheel build source SHA.

The final stable extra probe run yielded **15 exact exports + 5 explicit
rejections**, zero accepted mismatch. The original probe sources are tracked in
`tests/species_units_review.rs::frozen_species_default_probe_acceptance`.
The twenty cases cover duplicate components, raw-alias cancellation, reversed/
multiple/future reuse, expression-first ordering, generated species/reaction
collisions, forward/default aliases, explicit and inherited metadata, dimensionless
mapping and provable conflicts. Require independent reference/inspection PASS and
an empty `native_check.differences` result for each accepted source, never raw-XML
comparison or simplification. Loop assertions (including 4 threads × 8 models)
and repeated configurations are not additional unique tests.

The checker permits an audited `SBML_INVALID` reference only for an explicitly
required strict native rejection. It first verifies the live result against the
reviewed snapshot and retains its validation errors; exporting such a fixture
fails before semantic comparison. A missing/crashed oracle cannot earn success.
Prior fixture-source/golden hashes and intentional expectation-only migrations are
recorded by the oracle; broader acceptance was not blindly regenerated.

An early WIP test invocation failed because the shared model had new fields before
semantic construction was wired. It was not counted as a pass; stable native gates
above were rerun. The native owner's last boundary audit found and fixed numeric
reinterpretation of special default names before the stable binary was tested.

## 4. What's Next (Concrete Next Steps)

After A passes, proceed to the separately reviewed canonical formatter. Preserve
source-level unit declarations and first appearance, not reconstructed generated
SBML definitions. Preserve literal-versus-InitialAssignment mode and operator
binding, then require idempotence and native/reference roundtrips. Full-profile,
numerical and release gates remain separate.

## 5. Novel Opportunities (Innovation & Monetization)

Explicit source-versus-export unit views can help model editors explain otherwise
surprising conversions and diagnose interoperability problems without embedding
Python in production. No publishing or commercial action is implied.

## Key Learnings:

1. Upstream unit reduction uses raw alias identity before expansion.
2. Invalid reference SBML can justify only a tested rejection, never export equivalence.
3. Inherited defaults and explicit source annotations require separate metadata views.
