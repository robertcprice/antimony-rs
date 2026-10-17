# Eight original analytic numerical anchors

These are deliberately small **mathematical test models**, not biological
validation or official SBML Test Suite cases. Sources are original repository
fixtures, independently derived below, and remain within
`flat-declarations-v0.1-slice`. They do not add simulation to the Rust crate.

The [manifest](../fixtures/numerical/manifest.json) binds eight source hashes,
explicit ordered output selections, physical constants, and fixed sampling and
comparison settings. The stdlib-only [anchor implementation](../tools/numerical/anchors.py)
contains closed forms and manually written semantic preconditions; it does not
infer coefficients from SBML, a numerical solver, or measured trajectories.

## Mathematical and output conventions

Let `s` denote species concentration and `n_s = V_s s` its amount, with constant
compartment volume `V_s`. All eight sources initialize **concentrations**, not
amounts; native `initialAmount` syntax and substance-only species are not claimed.
A reaction's supplied kinetic law `v` is extent/time. Here extent and species
substance scales agree whenever specified, and no conversion factor is present.
For stoichiometric coefficient `ν`, `dn_s/dt = ν v`, hence `ds/dt = ν v/V_s`.
The model author must supply any volume factor in `v`; a compiler cannot insert
one to imitate a familiar concentration-rate equation. These distinctions follow
[SBML L3V2 Core, release 2](https://sbml.org/specifications/sbml-level-3/version-2/core/release-2/sbml-level-3-version-2-release-2-core.pdf).

A boundary reservoir is held fixed despite appearing as a reactant; it supplies
material from outside the evolving subsystem. It is not appropriate to impose a
closed-system balance on that reservoir plus its products. In these fixtures,
upstream boundary species have `boundaryCondition=true`, `constant=false`.

The simulation selection contract is explicit: `S` requests amount, `[S]`
requests concentration. Both are requested for **every species**, including
boundary species. Compartment sizes and all parameters are also selected. A
solver adapter must verify the installed API's selection interpretation and exact
returned column names, rather than assume a default selection order.

`species S,P in cell` does **not** put both species in `cell`: the compartment
qualifier belongs to the corresponding entry. Explicit fixtures use
`species S in cell, P in cell`. Structural preconditions independently verify
ownership, sizes, initial values, math trees, stoichiometry, boundary flags,
initial assignments, units and absence of unmodeled constructs before simulation.

## Frozen execution and comparison contract

- Time grid: `t_i = i/10`, `i=0,...,100`; start 0, end 10, **101 rows**, 100 intervals.
  Both endpoints and every interior time must be checked. There are no zero-duration
  or empty successful cases. Explicit `time` in a rate expression remains an
  unsupported-feature negative case, not a new native capability.
- Intended integrator: CVODE; relative tolerance `1e-10`, absolute tolerance
  `1e-12`, maximum steps 100000, variable-step output disabled, stiff mode enabled.
  The runtime owner must verify and record effective settings using the installed
  API. These settings are proposals until that readback succeeds; no default
  solver assumption earns evidence.
- Every finite observation `U` must satisfy
  `abs(U-E) <= 1e-8 + 1e-7*abs(E)` against analytic `E`. The same frozen rule is
  used for invariants and native/reference comparisons, not tuned after a failure.
  The absolute term handles zeros without division by zero; it must not be
  replaced by a relative-only ratio or enlarged using an erroneous observation.
- Check **each cell**, not just an average or endpoint. Report maxima with the
  corresponding case, column and time. Nonfinite values always fail this finite
  anchor set, including two matching NaNs/infinities. Reject nonnumeric values
  and booleans masquerading as numbers before arithmetic.
- Fresh simulator/model state for each path and case; never start reference from
  native's final state. Do not reset by overwriting initial values from an analytic
  curve: the original exported initialization is what is being tested.

The [official semantic-suite instructions](https://github.com/sbmlteam/sbml-test-suite/blob/develop/cases/semantic/README.md)
use `steps+1` rows and the expected-value-scaled additive tolerance rule. We adopt
that comparison form, not its case inventory or certification. RoadRunner's
[`simulate` tutorial](https://sys-bio.github.io/roadrunner/docs-build/tutorial/running_simulations.html)
distinguishes output points from intervals. These documentation references do not
substitute for pinning and testing the actual installed runtime.

## Units decay

<a id="units-decay"></a>
Source: [units_decay.ant](../fixtures/numerical/units_decay.ant).

`V=1`, `s(0)=3`, `k=0.4`; reaction `S =>` has `v=k*S*cell`.
Thus `dn_S/dt=-k s V`, `ds/dt=-0.4s` and

```text
s(t) = 3 exp(-0.4t)
n_S(t) = s(t)
```

The derivative of the proposed solution is `-0.4 s`; its initial value is 3.
There is no conserved species total because material leaves through a sink.
The concentration, amount and rate constant stay finite and nonnegative.

This anchor declares time `second`, substance and extent `mole`, volume `litre`,
and parameter unit `per_second = second^-1`. Preconditions require the exact
model defaults and inverse-time unit definition (kind second, exponent -1,
scale 0, multiplier 1). Unit annotations do not numerically rescale `k` or time.
This tests declared metadata, not universal dimensional consistency; retained
unit warnings remain visible. Scaled time metadata is not covered by these eight.

## Reversible

Source: [reversible.ant](../fixtures/numerical/reversible.ant).

Default volume 1; `a(0)=3`, `b(0)=1`; supplied **net** kinetic law is
`v=0.3a-0.1b`. `->` means reversible but does not synthesize a reverse law.

```text
a' = -0.3a + 0.1b
b' =  0.3a - 0.1b
a+b = 4
b = 4-a  =>  a' = -0.4a + 0.4

a(t) = 1 + 2 exp(-0.4t)
b(t) = 3 - 2 exp(-0.4t)
```

Initial conditions follow at `t=0`; differentiating gives the stated ODEs.
Amounts equal concentrations here. The amount sum is exactly 4, with equilibrium
`a=1`, `b=3` and zero net flux. The finite grid need not reach equilibrium exactly.

## Nonunit no factor

<a id="nonunit-no-factor"></a>
Source: [nonunit_no_factor.ant](../fixtures/numerical/nonunit_no_factor.ant).

Both species have `V=3`; `s(0)=2`, `p(0)=0`; the actual law is `v=0.6s`, **without**
a volume factor. Therefore `s'=-0.6s/3=-0.2s`, `p'=0.2s`, not `s'=-0.6s`.

```text
s(t) = 2 exp(-0.2t)
p(t) = 2 (1-exp(-0.2t))
n_S = 3s; n_P = 3p; n_S+n_P = 6
```

The initial amounts are 6 and 0. Differentiation proves both equations and the
closed-system amount invariant. At zero, the implementation uses `expm1` for the
small complement rather than subtraction of nearly equal numbers. Inserting an
unsupplied volume factor causes a large, readily detectable rate error.

## Boundary

Source: [boundary.ant](../fixtures/numerical/boundary.ant).

Default volume 1; reservoir `B=2` has `$B` in the reaction. Product starts at 1.
The law `v=0.3B` is constantly 0.6. Since B is boundary-held:

```text
b(t) = 2
p'(t) = 0.6
p(t) = 1 + 0.6t
```

The initial values and derivatives are immediate. Amounts equal concentrations.
`b+p` grows; enforcing its conservation would be wrong. The independent invariant
is `b=2`; the product value at `t=10` is exactly 7.

## Source drain

<a id="source-drain"></a>
Source: [source_drain.ant](../fixtures/numerical/source_drain.ant).

Default volume 1; `s(0)=0`; source law `q=0.6`, sink law `0.2s`.

```text
s' = 0.6 - 0.2s
s(t) = 3 (1-exp(-0.2t))
```

Differentiation yields `0.6 exp(-0.2t)`, which equals the right-hand side.
Initial amount and concentration are both zero; subsequent values increase
toward 3. This tests the absolute tolerance at an exact zero and a nonconstant
open system, not a mass-conservation condition.

## Forward initial

<a id="forward-initial"></a>
Source: [forward_initial.ant](../fixtures/numerical/forward_initial.ant).

The source order is `S=base+delta`, forward-declared `k=p/2`, `p=0.8`, `base=2`,
`delta=1`, then `held=S`. Acyclic initialization gives `S(0)=3`, `k=0.4`,
`held=3`. Default volume is 1. The sink law is `k*S`.

```text
s(t) = 3 exp(-0.4t)
n_S(t) = s(t)
k(t) = 0.4; p(t) = 0.8; base(t) = 2; delta(t) = 1; held(t) = 3
```

Initial assignments run at initialization, not at every time. In particular,
`held` must remain 3 while S decays. Upstream marks `held.constant=false` because
it depends initially on a floating species, but there is no rule or reaction
changing it; a nonconstant flag alone does not create dynamics. The exact three
InitialAssignment trees and target sequence `S,k,held` are preconditions. Merely
matching the t=0 row cannot test this distinction.

## Stoichiometry

Source: [stoichiometry.ant](../fixtures/numerical/stoichiometry.ant).

Default volume 1; `a(0)=4`, `b(0)=0`; `2 A => B` with supplied law `v=0.2a`.
Stoichiometry affects the derivative, **not an invented reaction order**.

```text
a' = -2v = -0.4a
b' = v = 0.2a
a(t) = 4 exp(-0.4t)
b(t) = 2 (1-exp(-0.4t))
a(t) + 2b(t) = 4
```

These initial conditions and derivatives follow directly. Do not square A because
it has stoichiometry 2; do not omit the factor 2 from consumption. Amounts equal
concentrations and the weighted amount invariant is 4.

## Unequal transfer

<a id="unequal-transfer"></a>
Source: [unequal_transfer.ant](../fixtures/numerical/unequal_transfer.ant).

A occupies `V_A=2`, B occupies `V_B=5`. Initial concentrations are `a(0)=3`,
`b(0)=1`, hence initial amounts 6 and 5. Law `v=0.4a*V_A=0.8a` transfers equal
amounts, not equal concentrations:

```text
a' = -v/2 = -0.4a
b' =  v/5 =  0.16a
a(t) = 3 exp(-0.4t)
b(t) = 1 + 1.2 (1-exp(-0.4t))
n_A(t) = 2a(t); n_B(t) = 5b(t)
n_A+n_B = 2a+5b = 11
```

Differentiating b gives `0.48 exp(-0.4t)=0.16a`; initial values match.
The unweighted concentration sum is **not** conserved. A same-volume assumption
or conflated amount/concentration output fails even if both species trajectories
look qualitatively reasonable.

## Evidence, mutation tests and limits

The initial author verification passed **19 stdlib anchor tests**, including
hand-derived ODE finite-difference checks, all 101 finite/nonnegative rows,
initial conditions, amount invariants, fixed held parameter, and mutations of
volume, stoichiometry, math trees, units, initialization, flags, source hash,
selection inventory, time bounds, NaN/infinity and tolerances. Both the native
and pinned-reference SBML for all eight fixtures passed the manual semantic
preconditions, and all eight native/reference projections matched exactly.
These checks are **not simulation results**; numerical evidence belongs to the
separate executable harness report.

False-pass protections must include exact row/column shape and order, no `zip`
truncation, explicit time checks rather than comparing two mutually shifted grids,
finite-number checks before tolerance arithmetic, no dropped boundary/constant
columns, and fresh state per path. Tests must demonstrate that a wrong volume,
wrong stoichiometry, time shift, held-value drift, missing/extra rows, swapped
selections and nonfinite output fail. Do not use an allclose flag that considers
NaNs equal or accept a skipped/blocked simulator as a pass.

Eight smooth deterministic toy models do not establish broad SBML semantic-suite
conformance, stiff-system accuracy, events, rules, explicit time forcing, dynamic
volumes, stochastic correctness, scaled-unit conversion, or biological validity.
The same simulator on two translations still shares solver defects; independent
closed forms reduce that particular blind spot only for these stated equations.
