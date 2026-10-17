# Assignment and rate rules: bounded native slice

**Verified bounded checkpoint (2026-10-14).** The implemented profile is
`rules-core-v0.4-slice`; all fourteen maintained gates passed on immutable reviewed binaries.
This extends the separately bounded arithmetic, symbols, numeric builtins,
rational and conditional slices, not the complete Antimony language or a solver.

## What you can write

- **Assignment rules (`:=`):** `a := 2 * b + 1;`, `species S := piecewise(1, time < 5, 2);`,
  `var p := x + y;`, `const S := 2;`, `species $S := 2;`.
  Target species or parameters become non-constant (`constant="false"`) and are exported
  as SBML `<assignmentRule variable="...">` elements under `<listOfRules>`.
- **Rate rules (`'=`):** `x' = -k * x;`, `species S' = k_in - k_out * S;`,
  `var x' = 1;`, `const S' = 0.5;`, `species $S' = 0.5;`.
  Target species or parameters become non-constant (`constant="false"`) and are exported
  as SBML `<rateRule variable="...">` elements under `<listOfRules>`.
- **Same-kind rule replacement:** Repeating `x := expr1; x := expr2;` or
  `x' = expr1; x' = expr2;` replaces the authoritative rule expression while preserving
  the target's first-seen rule order and retaining any symbols inferred by superseded
  expressions (`a := u; a := 2;` retains parameter `u`).
- **Rule export ordering:** Rules in `ValidatedModel::rules()` and SBML `<listOfRules>`
  are emitted in the target symbol's **first-seen declaration/reference order** in the
  source document, matching upstream Antimony rather than rule-statement or topological order.

## Ownership, lexical scalars, and species boundary promotion

1. **Ongoing rules vs. initial assignments:** Assignment rules (`:=`) clear any prior
   ordinary `=` scalar or symbolic initialization on the same target. An ordinary `=`
   after `:=` rejects (`DiagnosticCode::DeclarationConflict`), and mixing `:=` with `'=`
   on the same target rejects (`DiagnosticCode::DeclarationConflict`). Rate rules (`'=`)
   retain independent `=` initial values or `InitialAssignment`s (`x = 4; x' = -k * x;`
   or `x' = -k * x; x = 4;`), never deriving an initial value from the derivative RHS.
2. **Lexical scalar-export rule:** For an assignment-rule target (`:=`), upstream Antimony
   populates the SBML scalar attribute (`value` on `<parameter>`, `initialConcentration`
   on `<species>`) and `Parameter::value()` / `Species::initial_concentration()` **only**
   when the source RHS is a bare finite numeric literal (`a := 2.5;`) or unary-minus bare
   numeric literal (`a := -2;`). Parenthesized literals (`a := (2.5);`), unary plus
   (`a := +2.5;`), rational literals (`a := (1/2);`), Booleans (`a := true;`), constants
   (`a := pi;`), and general expressions (`a := b + 1;`) leave the scalar attribute unset
   (`None`) while exporting the normalized `<assignmentRule>` MathML.
3. **Source-order-sensitive species boundary promotion:** When an untyped symbol is first
   assigned a rule and later promoted to a species (`a := 2; species a;`, or inline
   `species S := 2;`), it becomes a boundary species (`boundaryCondition="true"`), unless
   a later `var species a;` clears the boundary flag back to `false`. By contrast, a
   species declared **before** its rule (`species S; S := 2;`) stays a floating species
   (`boundaryCondition="false"`) unless marked `const` or `$`. In SBML Level 3 Version 2,
   a non-boundary (`boundaryCondition="false"`) species determined by a rule may appear as
   a reaction modifier (`J: A -> B; S * A;`) but cannot appear as a reactant or product
   (`DiagnosticCode::DeclarationConflict`).
4. **Transient and static dependency cycles:** Assignment rules participate alongside `=`
   initializers in static all-branch dependency cycle detection. Prefix checks run at each
   assignment statement so transient cycles (`a := b; b := a; a := 1;` or
   `a = b; b := a; a = 1;`) reject immediately (`DiagnosticCode::InitializationCycle`).
   Rate rules represent `dx/dt` and allow self- and mutual feedback (`x' = -k * x;`).
5. **Conservative unit and dimension checking:** When both target and RHS expression have
   known dimensions, assignment rules require matching dimensions and rate rules require
   `dim(RHS) == dim(target) / dim(time_unit)` (`DiagnosticCode::UnitsInconsistency`).

## Try your own ordinary file

```antimony
model coupled_rules_input
  species S = 0;
  x = 4;
  k = 0.5;
  x' = -k * x;
  y := 2 * x;
  J: => S; y;
end
```

Save as `coupled_rules_input.ant`, then use the general CLI (not a fixture dispatcher):

```sh
cargo run --features cli --bin antimony -- check coupled_rules_input.ant
cargo run --features cli --bin antimony -- format coupled_rules_input.ant
cargo run --features cli --bin antimony -- export coupled_rules_input.ant
```

The crate parses, analyzes, formats and exports SBML; it does **not** integrate ODEs.
The separate two-model numerical rules gate tests the closed-form solutions of
`coupled_rate_and_assignment_rule` ($x(t)=4e^{-0.5t}$, $y(t)=8e^{-0.5t}$,
$S(t)=16(1-e^{-0.5t})$) and `boundary_species_assignment_and_rate_rule`
($u(t)=t$, $B(t)=1+0.2t$, $S(t)=t+0.1t^2$) on 101 grid points over $[0,10]$.
It is not a general DAE/ODE solver certificate or simulation readiness claim.

## Explicit exclusions and evidence

Empty rules (`a := ;`, `a' = ;`), algebraic rules (`0 = x + y;`), compartment rules
(`compartment C = 1; C := 2;`, `C' = 0.1;`), reaction kinetic-law replacement
(`J := ...`), events (`at(...)`), user-defined functions, `delay`, and submodel
hierarchy remain outside this slice and reject explicitly (`DiagnosticCode::UnsupportedFeature`).
The core-only [Rust API guide](RULES_API_NOTES.md) demonstrates `ValidatedModel::rules()`,
`Rule`, and `RuleKind`. Reference discovery and contract reconciliation are documented in
[RULES_REFERENCE.md](RULES_REFERENCE.md) and [RULES_EVIDENCE_PLAN.md](RULES_EVIDENCE_PLAN.md).

The frozen rules corpus (`fixtures/rules/manifest.json`) contains **200 cases**:
**114 exact positive exports** and **86 expected rejections**. Positive evidence requires
exact projection equality, non-null rule MathML, non-constant species/parameter rule
targets, absence of dual `InitialAssignment` on assignment-rule targets, no libSBML
Error/Fatal messages, and byte-idempotent formatting roundtrips.

Verification is recorded in the [independent review](reviews/rules-review.md) and
[compact evidence](../tools/oracle/rules-evidence.json). Python suites passed **467 tests**
(282 oracle, 103 numerical, 51 workloads, 31 verification), zero failures/skips.
The four-configuration Rust test matrix passed: all-feature **455**, default **410**,
core-only **395** and CLI-only **440** tests, plus **2 doctests** per configuration.

| Maintained gate | Verified result |
|---|---|
| Rules (`rules-core-v0.4-slice`) | 200: 114 exact exports, 86 expected rejections |
| Conditional + alias supplement | 262: 159 exact exports, 103 expected rejections |
| Rational corrective | 124: 73 exact, 51 rejections |
| Original / formatter / unified CLI | 90 each: 58 supported, 32 rejections |
| Arithmetic | 54: 48 exact, 6 rejections |
| Symbols | 66: 54 exact, 12 rejections |
| Builtins | 109: 66 exact, 43 rejections |
| Workloads | 86: 74 exact, 12 rejections |
| Retained analytic / builtin-time / piecewise / rules numerical | 8/8, 2/2, 1/1, 2/2 |

In the original 90-case corpus (and formatter/CLI routes), `rejected_assignment_rule`
and `rejected_rate_rule` are now intentionally supported under the explicit
hash-bound overlay `fixtures/manifests/rules-core-v4-overlay.json` (moving 56/34 to 58/32
when `--current-profile` is selected), while historical manifests and receipts remain
untouched.

## Reproduce the bounded checks

With trusted, prebuilt binaries:

```sh
export ANTIMONY_IMPLEMENTATION_PROFILE=rules-core-v0.4-slice
.venv-oracle/bin/python tools/oracle/rules_check.py \
  --trusted-native --cli artifacts/rules-phase4/bin/antimony
.venv-oracle/bin/python tools/numerical/rule_check.py \
  --trusted-native --native artifacts/rules-phase4/bin/export_sbml \
  --python .venv-numerical/bin/python --reference-python .venv-oracle/bin/python
```
