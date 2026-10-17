# Six original structural motif workloads

These six repository-authored synthetic motifs combine existing language features
in meaningful multi-reaction networks. They are **not copied published models**,
calibrated biology, a representative sample of all users, or biological validation.
They are representative of selected modeling **patterns**, not a claim about
population coverage. No new language feature or numerical capability is added.

All six have numerical status **NOT_RUN**. The conservation laws below are
independent algebraic derivations from stoichiometry and compartment sizes;
they have not been checked against simulated trajectories. In particular, no
closed-form trajectory is invented for nonlinear binding or rational feedback.
The earlier eight analytic anchors remain a separate, unchanged numerical scope.

## Sources, contracts and evaluation scope

The [manifest](../fixtures/workloads/manifest.json) pins six source hashes,
identifiers, original provenance and NOT_RUN numerical status. The manually
written [motif contracts](../tools/workloads/motifs.py) describe full expected
semantic projections independently of a compiler or reference-generated golden.
They specify exact entities, initial values and modes, compartments, parameter
constants, reaction math trees, participant order, stoichiometry, boundary flags,
modifiers, unit metadata and absence of unsupported constructs.

The shared structural checker is [tools/workloads/check.py](../tools/workloads/check.py).
Its evidence is separate from both the original 90-case language corpus and the
eight numerical models. Formatting must preserve the same meaning and be byte
idempotent. A live reference match alone cannot bless the wrong original motif:
reference and native outputs must also match the independent manual contract.
Warnings are retained, not a global dimensional-consistency certificate.

Notation: lower-case `s` denotes concentration and `n_S=V_S*s` amount in the
species' fixed compartment. For stoichiometric matrix N and authored reaction
extent rates v, `dn/dt=Nv` for floating species; boundary species have a zero
effective reaction row. Thus a weight vector w with `wᵀN=0` defines a formal
amount invariant. Concentration derivatives divide each row by its own volume.
This follows [SBML L3V2 Core release 2](https://sbml.org/specifications/sbml-level-3/version-2/core/release-2/sbml-level-3-version-2-release-2-core.pdf);
a kinetic law is not automatically a concentration/time law. The invariants here
are stoichiometric moiety counts, not an independently established physical mass
balance for real compounds.

All sources bind each species explicitly: `species S in cell,P in cell`.
`species S,P in cell` would bind only P and is intentionally avoided. `->` is
reversible, but its supplied net rate remains authoritative; no extra reverse
reaction is invented. `=>` is irreversible. All species initialize concentrations,
not `initialAmount`, and have `hasOnlySubstanceUnits=false` and `constant=false`.
Only Feed in the feedback case is boundary-held.

## Enzyme binding catalysis

Source: [enzyme_binding_catalysis.ant](../fixtures/workloads/enzyme_binding_catalysis.ant).
One compartment of volume 2; four species; three scalar parameters; two reactions.
Initial concentrations `(E,S,C,P)=(1,8,0,0)`. Constants
`kon=0.3`, `koff=0.2`, `kcat=0.4`.

```antimony
Bind: E+S -> C; (kon*E*S-koff*C)*cell;
Cat: C => E+P; kcat*C*cell;
```

Let `v_b=2(0.3 e s-0.2 c)` and `v_c=2(0.4 c)`.
Amount derivatives, in the stated species order, are
`(-v_b+v_c, -v_b, v_b-v_c, v_c)`. Therefore

```text
n_E+n_C = 2
n_S+n_C+n_P = 16
```

Both weighted derivatives cancel reaction by reaction. Initial amounts are
`(2,16,0,0)`. Initial extent rates `(v_b,v_c)=(4.8,0)` are independently checked
using exact rational arithmetic; initial concentration derivatives are
`(-2.4,-2.4,2.4,0)`. Binding is genuinely bilinear; no Michaelis–Menten or
quasi-steady-state reduction is assumed. Both reactions have no modifiers because
all species named in their rates are already reaction participants.

## Branched pathway

Source: [branched_pathway.ant](../fixtures/workloads/branched_pathway.ant).
Volume 3; `(A,B,C,D)=(6,1,2,0)` initially; four irreversible reactions:

```antimony
AB: A => B; k1*A*cell;
AC: A => C; k2*A*cell;
BD: B => D; k3*B*cell;
CD: C => D; k4*C*cell;
```

The forward initialization `k1=q/2`, with later scalar `q=0.4`, must remain an
InitialAssignment, not be replaced in the export by scalar 0.2. Other parameters
are `k2=0.1`, `k3=0.3`, `k4=0.25`. All five parameters are constant.
Writing the four authored extent rates as `(v1,v2,v3,v4)` gives
`dn/dt=(-v1-v2,v1-v3,v2-v4,v3+v4)` and

```text
n_A+n_B+n_C+n_D = 27
```

Initial rates are `(3.6,1.8,0.9,1.5)` and concentration derivatives
`(-1.8,0.9,0.1,0.8)`. The model checks one-to-many splitting and many-to-one
merging in one network while preserving the initialization tree and reaction IDs.
No reaction has a modifier.

## Coupled transport catalysis

Source: [coupled_transport_catalysis.ant](../fixtures/workloads/coupled_transport_catalysis.ant).
A occupies outside volume 2; `(B,F,C,P)` occupy inside volume 5.
Initial concentrations `(A,B,F,C,P)=(4,1,2,0,0)`, giving amounts `(8,5,10,0,0)`.

```antimony
Transport: A => B; kt*A*outside;
Bind: B+F -> C; (kon*B*F-koff*C)*inside;
Cat: C => F+P; kcat*C*inside;
```

Constants are `(kt,kon,koff,kcat)=(0.2,0.3,0.1,0.05)`.
For extent rates `(v_t,v_b,v_c)`, derivatives in the stated species order are
`(-v_t,v_t-v_b,-v_b+v_c,v_b-v_c,v_c)`. Two independent formal pools remain:

```text
n_A+n_B+n_C+n_P = 13
n_F+n_C = 10
```

Initial rates `(1.6,3,0)` imply concentration derivatives
`(-0.8,-0.28,-0.6,0.6,0)`. The first rate uses **outside** volume, while binding and
catalysis use **inside** volume. Equal transported amounts do not imply equal
concentration changes across volumes. The pools are not unweighted sums of
concentrations. Reversible binding is bilinear and has no claimed closed form.

## Rational feedback

Source: [rational_feedback.ant](../fixtures/workloads/rational_feedback.ant).
Volume 2; boundary Feed initially 2, floating `(X,Y,Z)=(1,0,0.5)`.
Parameters `(q,Ki,k1,k2,d)=(0.4,2,0.3,0.2,0.1)`.

```antimony
Input: Feed => X; q*Feed/(1+(Z/Ki)^2)*cell;
XY: X => Y; k1*X*cell;
YZ: Y => Z; k2*Y*cell;
Drain: Z => ; d*Z*cell;
```

Input alone has modifier **Z**: Z affects the rate without being consumed or
produced there. Feed has `boundaryCondition=true`, `constant=false`; it is a
reservoir, not an immutable-SBML-constant claim. For rates `(v_i,v_1,v_2,v_d)`:

```text
dn_Feed/dt = 0                 n_Feed = 4
dn_X/dt = v_i-v_1
dn_Y/dt = v_1-v_2
dn_Z/dt = v_2-v_d
d(n_X+n_Y+n_Z)/dt = v_i-v_d
```

There is no closed floating-species amount invariant. Feed plus products is not
conserved because Feed is externally maintained and Drain removes material.
The denominator is at least 1 for real Z since `Ki=2`; this specific rational
formula has no denominator singularity. Initial rates are
`(128/85,3/5,0,1/10)`, with concentration derivatives
`(77/170,3/10,-1/20)` for `(X,Y,Z)`. This is a saturating inhibition motif, not an
assertion about measured feedback strength or a proven time-course solution.

## Reversible shuttle

Source: [reversible_shuttle.ant](../fixtures/workloads/reversible_shuttle.ant).
A occupies left volume 2; B and C occupy right volume 5.
Initial `(A,B,C)=(2,1,0.5)`, so amounts are `(4,5,2.5)`.

```antimony
Exchange: A -> B; kt*A*left-kr*B*right;
Convert: B => C; kb*B*right;
Return: C => A; kc*C*right;
```

Constants `(kt,kr,kb,kc)=(0.2,0.1,0.3,0.15)` give initial extent rates
`(0.3,1.5,0.375)`. For `(v_e,v_b,v_c)`, amount derivatives are
`(-v_e+v_c,v_e-v_b,v_b-v_c)` and

```text
n_A+n_B+n_C = 2a+5b+5c = 11.5
```

Initial concentration derivatives are `(0.0375,-0.24,0.225)`.
Exchange may reverse when its supplied net expression is negative; no sign clamp
or separately invented reverse rate is permitted. This model checks a cyclic
reaction network, which is not an initialization dependency cycle.

## Unit forward sequence

Source: [unit_forward_sequence.ant](../fixtures/workloads/unit_forward_sequence.ant).
Volume 4; initial `(S,I,P)=(3,0,1)`; two irreversible reactions:

```antimony
First: S => I; kfast*S*cell;
Second: I => P; kslow*I*cell;
```

The source declares model time second, substance/extent mole, volume litre, and
custom `per=1/second`. Both rate constants are annotated `per`. `kslow=0.1` is
scalar, while `kfast=kslow*2` remains a forward InitialAssignment with its original
multiplication tree. No automatic numeric rescaling is performed.

For rates `(v_f,v_s)`, amount derivatives are `(-v_f,v_f-v_s,v_s)` and

```text
n_S+n_I+n_P = 16
```

Initial rates `(2.4,0)` imply concentration derivatives `(-0.6,0.6,0)`.
The declared units and exact custom-unit representation are required structural
preconditions, not a claim that all dimensional warnings disappear. The inspected
reference retains one warning for this fixture; it is not suppressed.

## Verified author checks and limitations

Initial discovery checked all six originals through native export, pinned
Antimony conversion and independent libSBML inspection: all native/reference
projections matched exactly. Native formatting was byte-idempotent for all six.
The manual contracts were then checked against both export paths. A draft manual
power tag used caret AST 94; the live check caught it, and the contract was
corrected to the existing canonical MathML `<power/>` tag 296 already frozen in
`init_precedence.expected.json`. No model source, native behavior or generalized
comparison normalization was changed to conceal that drafting error.

The independent motif test suite has **18 tests**: exact inventory/hashes,
entity/initialization/unit/modifier contracts, eight amount-space nullspace laws
and their initial totals, exact initial reaction rates, and mutations of math,
stoichiometry, units, ownership/flags, finite numeric types and provenance/status.
These are static algebra and compiler-contract checks, **not simulation results**.
Final shared workload execution and independent review are reported separately;
no successful output from this document alone establishes a live gate.

## Reusable resource-boundary test patterns

Current `Limits` defaults are inclusive: 4 MiB source, 1,000,000 tokens, expression
depth 128, 100,000 entities and 100 diagnostics (`src/limits.rs:42`). The parser
also imposes a hard expression-depth ceiling 256 (`src/syntax/parser.rs:450`).
Relevant existing tests are `tests/hardening.rs:163` (configured limits), `:192`
(forward dependency chains), `tests/foundation.rs:180` (inclusive UTF-8 byte
budget), `tests/cli_io_review.rs:134` (real process source cap before UTF-8 decoding),
and `tests/format_review.rs:199,244` (format expansion/token budget).

A subprocess test adapter can invoke the existing `SourceFile::new`, `parse`,
`analyze`, `format_antimony` and `write_sbml` APIs with **small custom limits**,
without adding production CLI flags or exposing filesystem access in the core.
Each boundary should include below/exact/above counts, a parent deadline and clean
termination, explicit `resource_limit`, valid original-source spans and no output
artifact on rejection. Test parentheses, unary nesting, left-associative chains
and forward initialization separately: their depths need not share an off-by-one
convention. This proposal is not an already executed subprocess limit matrix and
not an asymptotic performance, allocation bound or hostile-input sandbox claim.
