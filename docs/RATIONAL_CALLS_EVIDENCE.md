# Supplemental rational-call compatibility investigation

**Verified bounded corrective checkpoint (2026-10-14): `math-core-v0.2.1-slice`.**

This supplement records a newly discovered gap in the pinned Antimony 3.2.0
compatibility boundary after the bounded Phase 3A verification. The existing
Phase 3A receipts remain immutable historical evidence for their recorded inputs;
they did not cover this gap. Do not rewrite their reports, manifests, snapshots or
binary copies to imply that the new cases were tested then.

## Observed near-neighbors

The native owner's initial reference probes are retained at
`artifacts/conditionals-phase3b/discovery/rational-calls.json`. In that probe:

| Source expression | Pinned upstream result |
|---|---|
| `sin(1/2)`, `cos(1/2)`, `sqrt(1/2)`, `abs(1/2)`, `log(1/2)` | Rejected: unexpected number in rational notation |
| `sin(1 / 2)`, `sin(1.0/2)` | Rejected with the same diagnostic |
| `sin((1)/2)` | Accepted, with an ordinary divide subtree |
| `log(2,1/2)`, `pow(1/2,3)`, `min(1/2,3)` | Accepted, with an ordinary divide subtree |

The immutable Phase 3A native CLI accepts `a=sin(1/2);`, according to the native
owner's reproduction. That is a real reference-compatibility discrepancy, not
permission to weaken the oracle. Conversely, banning all division inside calls
would reject valid neighbors. The upstream diagnostic describes syntax handling;
it does not demonstrate a mathematical singularity or finite-real domain error.
These findings are pinned-implementation observations, not a claim that the
written Antimony language specification forbids rational numeric arguments.

## Supplemental corpus construction

1. Reuse the native owner's expanded isolated grid (`rational-grid.json`) rather
   than independently duplicating the exploratory sweep. Record each exact source,
   upstream load status and error or MathML; retain source/grouping/whitespace.
2. Select a bounded near-neighbor inventory under `fixtures/rational-calls` with
   explicit pairs: malformed unparenthesized numeric fractions versus accepted
   grouped numerators, binary/variadic arguments, and ordinary expression contexts.
   Do not infer a general grammar rule from a few examples.
3. Use fresh pinned reference processes for durable snapshots before any native
   fix. Preserve full validation diagnostics and an exact closed-vocabulary
   projection for accepted cases. Conditional-only `not(...)` discovery examples
   are informative but must not expand this numeric corrective slice.
4. Separate reference classification from desired native outcome. Reference
   rejections require a located syntax rejection; accepted sources require exact
   exported structure. Initialization domains are independent: do not choose a
   mathematically invalid positive merely because upstream loads it.

## Corrective verification procedure

A separately versioned supplemental manifest and checker must use new paths, not
mutate the 109-case builtin manifest or its snapshots. Check source/snapshot hashes,
reference runtime identity, accepted and rejected controls, and unchanged inputs.
For every supported case, exercise check, export, format, export of formatted
source, reference conversion of formatted source, and byte-idempotent formatting.
Formatting must not remove grouping that makes a source acceptable upstream.
Retain an explicit pre-fix report if the new corpus exposes native overacceptance.
Only a new independently reviewed, uniquely named binary handoff can earn post-fix
PASS credit. Rerun the existing nine gates without rewriting their previous reports.

This is structural parser/formatter/export verification. It adds no trajectories,
solver, general rational-number support or newly supported conditional language.

## Historical 57-case reference investigation

The [new manifest](../fixtures/rational-calls/manifest.json) now contains **57
original numeric-only cases**, selected from the owner's 175-case discovery grid:

- **22 reference syntax rejections**, preserving exact upstream diagnostics.
- **32 accepted non-rational AST cases**, including ordinary division neighbors.
- **3 accepted rational AST cases**: `(1/2)`, `(1.0/2)` and `sin((1/2))`.

All 57 were independently rerun through the existing fresh-process oracle capture
path. All 114 new source/snapshot hashes match the manifest; all 37 referenced
Phase 3A evidence/binary/input hashes remain unchanged. Capture/integrity records
are retained under `artifacts/rational-calls-supplement`. These are **reference
capture and integrity checks**, not native compatibility passes. Every entry
retains its original `native_expectation: NOT_SET`; the separate corrective
manifest and verified post-fix results below do not rewrite that investigation.

The broader issue includes accepted rational literals outside calls. The oracle
already distinguishes rational MathML (`cn type="rational"`, AST 259 with exact
numerator/denominator) from division (AST 47). That distinction must remain exact.
Do not compare the rational value `1/2` to an algebraically equal division tree
and call them structurally identical. The corrective contract now selects typed rational nodes, with the finite and
component-range boundaries below. The original 57-case investigation manifest
remains unchanged; the new corrective manifest freezes native expectations.

Signs and spelling need special care: the discovery grid accepts `sin(+1/2)` and
`sin(1/+2)` as division; negative signs do not provide the same escape. In the
captured near-neighbors, `sin(1e14/2)` rejects whereas `sin(1e15/2)` accepts. This is
another reason not to approximate the upstream token boundary with a generic
integer-expression or mathematical-value rule. No additional MathML tags or
projection normalization were introduced for this supplement.

## Authorized corrective contract (Phase 3A.1)

The separate [corrective manifest](../fixtures/rational-calls/corrective-v1.json)
contains **124 cases: 73 required supported exports and 51 required rejections**.
These are expectations frozen from reference capture and the approved finite
scope **before corrective native results**, not a reported native PASS count.

- Preserve unreduced signed numerator/denominator, including negative denominators
  and zero numerators. Rational atoms are dimensionless, unlike ordinary numeric
  literals whose existing dimensional inference is unchanged.
- Recognize the pinned primary canonical numeric spelling (15-digit precision,
  plain signed integers below `1e15`) in full grouping parentheses. Function-open
  rational tokens reject as syntax; positive signs and separately grouped operands
  retain their independently verified division interpretation.
- Use safe wider intermediates, then check **final** numerator and denominator
  against inclusive signed 32-bit bounds after all unary collapsing. For example,
  `-(-2147483648/2)` rejects but `-(-(-2147483648/2))` accepts. Final out-of-range
  rational components are syntax errors, matching reference-invalid inputs.
- A rational zero denominator is explicitly unsupported in every context. The
  ordinary division-rate cases `1/0` and `(1)/0` retain their previous structural
  acceptance; none receive runtime-safety or trajectory credit.
- Existing scalar precision/magnitude exclusions and negative initial species
  restrictions remain in force.

The 51 negatives comprise 38 reference-invalid syntax cases, six reference-loaded
nonfinite rational exports, six retained scalar-envelope exclusions and one
negative initial species. The nonfinite reference exports retain their complete
XML and validator messages in a separate `REFERENCE_NONFINITE` audit. That status
is **not** finite semantic equivalence or SBML-invalid evidence. `infinity` and
`notanumber` remain outside the ordinary exact-projection vocabulary; no comparison
normalization or tolerance was widened.

## Explicit current versus historical harness configuration

The default maintained harness configuration is now `math-core-v0.2.1-slice`.
Only that exact identity and the known historical `math-core-v0.2-slice` are valid:

```sh
# New corrective gate: requires an independently reviewed .2.1 binary.
python3 tools/oracle/rational_check.py --trusted-native --cli /absolute/path/new-antimony

# Reproduce an older builtin checkpoint with its unchanged historical binary.
ANTIMONY_IMPLEMENTATION_PROFILE=math-core-v0.2-slice \
  python3 tools/oracle/builtins_check.py --trusted-native --cli /absolute/path/old-antimony
```

The setting selects an explicit, hash-bound overlay and exact report identity;
it is not an allow-any-profile switch. Current `.2.1` and historical `.2` overlays
have identical old-corpus outcomes; the corrective overlay also pins its parent.
The corrective checker refuses the historical configuration. Unknown profile
settings and mismatched CLI report identities fail closed. Existing original,
formatter, CLI, arithmetic and workload routes still require `--current-profile`
when reproducing their appropriate overlay contracts. Historical manifests,
reference snapshots, receipts and copied binaries have not been rewritten.

The new gate and all nine retained gates were run on the new immutable reviewer
handoff, with the full Python harness suites. New reports are retained separately
under `artifacts/rational-phase3a1/gates`; previous reports were not overwritten.

## Final verified results

See the [compact report](../tools/oracle/rational-evidence.json),
[independent native review](reviews/rational-review.md),
[machine-readable boundary](../profiles/math-core-v0.2.1.json) and
[compiled core API example](RATIONAL_API_NOTES.md).

| Maintained gate | Actual result |
|---|---|
| Rational corrective124 | 73 exact supported exports,51 expected rejections |
| Original90 / formatter90 / CLI90 | Each56 supported,34 expected rejections;56 byte-idempotent formatted sources |
| Arithmetic54 | 48 supported,6 expected rejections |
| Symbols66 | 54 supported,12 expected rejections |
| Builtins109 | 66 supported,43 expected rejections |
| Workloads86 | 74 supported,12 expected rejections |
| Retained analytic models | 8 PASS |
| Separate builtin/time analytic models | 2 PASS |

**392 Python tests PASS**, zero failures/skips:230 oracle,80 numerical,51 workload,
31 historical receipt-harness tests. The final API-guide Rust snippet was built,
compiled and executed against the core-only library; all three commands passed.
Independent review records the17-command native feature matrix and separately
authored inputs. Source/binary identities were stable during testing.

Only the retained eight and two builtin/time analytic models have trajectory
credit. The124 rational cases add no trajectory claim, and nonfinite-reference
negatives earn rejection credit only. The suites overlap; do not sum their case
counts into unique-model coverage. No tolerance or exact-projection relaxation,
new full-source receipt, publication, upstream endorsement or licensing decision
is implied. Initial supplemental harness-development failures are retained under
`artifacts/rational-calls-supplement`; they were corrected before this final run.
