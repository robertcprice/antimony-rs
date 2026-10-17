# Numeric builtins, constants and time

> **Verified corrective checkpoint (2026-10-14): `math-core-v0.2.1-slice`.**
> Later probes exposed rational-token and rational-versus-division gaps outside
> the original Phase 3A inventory. The correction now passes a separate 124-case
> gate and all nine retained gates. See [the corrective evidence](RATIONAL_CALLS_EVIDENCE.md).
> Earlier receipts remain historical; bounded tests are not full-language proof.

**Historical bounded Phase 3A checkpoint (2026-10-14): `math-core-v0.2-slice`.**
This is a deliberately bounded extension of the symbol foundation, not full
Antimony. [Machine-readable scope](../profiles/math-core-v0.2.json) is separate
from the frozen kinetic-core-v0.1 target, which excludes functions. Historical
corpus reuse does not redefine that target or establish target conformance.

## Try your own model

Save this ordinary source as `my_model.ant`, then change the rates, initial value,
or names. These commands accept arbitrary files; there is no fixture dispatch.

```antimony
model time_forced
  species S = 0;
  formula k = 0.2;
  input: => S; exp(-time);
  loss: S =>; k*S;
end
```

With the CLI built/enabled with `cli` and `sbml`, run:

```sh
antimony check my_model.ant
antimony format my_model.ant -o my_model.formatted.ant
antimony export my_model.ant -o my_model.xml
```

The example has explicit species/parameter initial values and the standard implicit
unit-size compartment. Unspecified physical units still generate warnings. The
reviewer checked this exact pasted source through the ordinary copied CLI,
reference comparison and idempotent formatting. **This example was not simulated**;
the crate checks, formats and translates models, but contains no solver. Exporting
this input does not certify its runtime behavior, numerical accuracy or biology.

## Expression boundary

The supported numeric functions are `sin`, `cos`, `tan`, `exp`, `ln`, unary/binary `log`,
`log10`, `sqrt`, `abs`, `floor`, `ceil`/`ceiling`, `pow`/`power`, nonempty `min`/`max`,
`pi`, `exponentiale`, and `time`/`time()`. The ordinary identifier `e` is not Euler's
constant; case is significant. The pinned reference establishes:

- `log(x)` means natural logarithm, identical to `ln(x)`.
- `log(base,x)` preserves the base as the first argument; `log10(x)` explicitly
  carries base ten in MathML.
- `sqrt(x)` exports a root with explicit degree two, not a guessed generic call.
- `min`/`max` preserve their argument list and order. The reference accepts empty
  lists, but these are explicitly unsupported by this finite-real slice.
- Time exports the SBML time symbol, never an inferred parameter called `time`.
  Initial expressions involving time remain symbolic unless an initial-state
  contract is explicitly established; the compiler does not silently invent a
  start time or run a simulator.
- Known invalid initialization domains reject, including invalid constant
  subexpressions alongside missing dependencies. Unknown values remain unknown.
  An expression in a kinetic law is not evaluated as an initializer; accepting
  it does not prove its runtime domain is safe for all states.

Conditionals, piecewise, boolean operators, user-defined functions, rules,
events/delays and general roots remain outside this increment. The oracle admits
only verified numeric MathML tags; it still rejects unrepresented vocabulary,
attributes and non-time csymbols rather than discarding them.

A reference anomaly is retained honestly: `a=time(1)` loads and emits an
InitialAssignment without math, and the pinned libSBML checks report no error.
The native wrong-arity rejection is deliberate. We do not label the reference
SBML invalid without evidence or count this case as a semantic-equivalence pass.

## Exact structural verification

`fixtures/manifests/builtins-v2.json` pins 109 original sources and upstream-only
snapshots. The final independently checked split is 66 supported exports and 43 explicit
rejections. All accepted
cases must pass check/format/export, exact independent SBML comparison,
reference conversion of formatted source and byte-idempotent formatting.

The new `math-core-v2-overlay.json` preserves and hash-pins the Phase 2 overlay,
then changes the original `function_call` and arithmetic `deferred_builtin`
fixtures' native outcomes (both use `sin`). Eight original outcomes now differ from the historical manifest: seven
Phase 2 symbol cases plus this builtin. All arithmetic54, symbols66 and
workload86 source/expected structures remain unchanged; an explicit arithmetic
overlay changes only the deferred-sin native outcome (48 accepted, six rejected). The current original90
expectation is 56 supported and 34 rejected. `--current-profile` reports identify
both the historical corpus profile and actual expanded implementation scope.

## Two selected analytic inputs

A separate development-only gate adds exactly two original models, preserving the
older eight-model numerical manifest:

1. `S(0)=0`, unit-volume compartment, `dS/dt=exp(-t)` gives
   `S(t)=1-exp(-t)`, evaluated stably as `-expm1(-t)`.
2. `S(0)=0`, unit-volume compartment, `dS/dt=1+sin(t)` gives
   `S(t)=t+1-cos(t)`, evaluated as `t+2*sin(t/2)^2`.

These are abstract numeric time models with unspecified physical units, retained
as warnings—not a dimensional or biological certificate. Amount and concentration
coincide only because the explicitly checked implicit compartment has size one.
Handwritten full structural preconditions check both reference and native output;
analytical coefficients are not inferred from either output or fitted trajectories.

Each native and reference SBML is simulated in a fresh pinned RoadRunner worker
at **101 points, t=0 through 10**, using the existing CVODE settings and absolute
`1e-8` plus relative `1e-7` comparison bound. The gate checks the solver readback,
worker/runtime identity, exact grid and selections, finite values, analytic and
native/reference agreement, fixed volume and amount/concentration identity. These
two cases do not validate every builtin, singularity, initial-time convention or
incomplete model. Unavailable workers and changed inputs never earn pass credit.

## Reproduce after an immutable reviewer handoff

The commands default to the current corrective `.2.1` harness identity. For the
historical `.2` binaries, use the explicit configuration described in the
[rational evidence guide](RATIONAL_CALLS_EVIDENCE.md#explicit-current-versus-historical-harness-configuration).

```sh
python3 tools/oracle/builtins_check.py --trusted-native --cli /absolute/path/antimony
python3 tools/oracle/native_check.py --trusted-native --current-profile --native /absolute/path/export_sbml
python3 tools/oracle/roundtrip_check.py --trusted-native --current-profile --native /absolute/path/export_sbml --formatter /absolute/path/format_antimony
python3 tools/oracle/cli_check.py --trusted-native --current-profile --cli /absolute/path/antimony --native /absolute/path/export_sbml --formatter /absolute/path/format_antimony
python3 tools/oracle/expression_check.py --trusted-native --current-profile --cli /absolute/path/antimony
python3 tools/oracle/symbols_check.py --trusted-native --cli /absolute/path/antimony
python3 tools/workloads/check.py --trusted-native --current-profile --native /absolute/path/export_sbml --formatter /absolute/path/format_antimony
python3 tools/numerical/check.py --trusted-native --native /absolute/path/export_sbml
python3 tools/numerical/builtin_check.py --trusted-native --native /absolute/path/export_sbml
```

No command builds, installs, downloads or publishes. The historical full-source
receipt recipe still has incompatible hardcoded older inventories and cannot
certify this profile; its passing unit tests are not a current receipt.

### Corpus review correction

The first 104-case draft mistakenly called five retained scope restrictions
positive cases: empty reactions with neither side populated, a negative initial
species concentration, and redefinition of the builtin unit name `seconds`.
The first gate correctly failed those cases. They are retained as explicit
negative near-neighbors; five new upstream-verified supported companions exercise
time rates, unknown runtime-domain rates, a negative **parameter** in a kinetic
law, and a genuinely custom unit identifier. This preserves intended feature
coverage without expanding the native scope to satisfy faulty test expectations.
The arithmetic gate also correctly exposed the missing explicit deferred-sin
overlay. Initial failed reports and the 104-case manifest/provenance are retained
under `artifacts/builtins-phase3/{gates-initial,corpus-initial}`.

The negative-parameter `sqrt(q)` runtime companion is structurally representable
but has a singular/invalid runtime domain at that value. It has **no numerical
validation or simulation-readiness credit**; only the two separate analytic input
models receive trajectory checks.

## Historical Phase 3A verified results

See [compact evidence](../tools/oracle/builtins-evidence.json) and the
[independent native review](reviews/builtins-review.md). Final unchanged copied
binaries passed nine maintained gates:

- **109 builtin cases: 66 exact exports and 43 expected rejections.**
- Original90, formatter90 and CLI90: **56 supported/34 rejected each**, three
  routes through the same sources—not 270 distinct models.
- Arithmetic54: **48 supported/six rejected**, with the explicit deferred-sin overlay.
- Symbols66: **54 supported/12 rejected**. Workloads86: **74 supported/12 rejected**.
- **Eight retained analytic models plus two new analytic models passed**; the two
  new cases produce four independently checked 101-point native/reference trajectories.
- **370 Python tests passed**, zero failures/skips: 208 oracle, 80 numerical,
  51 workload and 31 historical receipt tests. Receipt unit tests are not a new
  full-source certification.

The initially failed evidence runs are retained and explained above. No source or
reference snapshot was rewritten to match native output, and no tolerance or
structural-comparison relaxation was introduced.
