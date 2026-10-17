# Proposed release profile: kinetic-core-v0.1

**This is a frozen target contract, not a current support claim.** The current
`math-core-v0.2.1-slice` is a separately versioned bounded implementation extension:
its numeric functions are outside this target's excluded-function boundary. It
does not establish conformance to the feature table below. See the
[current rational correction](RATIONAL_CALLS_EVIDENCE.md) and
[builtin boundary](BUILTINS.md). See the
[current symbol boundary](SYMBOLS.md), [verified arithmetic boundary](reviews/arithmetic-review.md)
and [canonical layout/scalar limits](reviews/canonical-format-review.md) plus
the profile JSON implementation/evidence fields. We target a documented
subset of Antimony, not a divergent dialect.
Valid language outside the subset should receive an explicit unsupported-feature
error. Coverage is not universal: the inherited `import "secret.ant"` lexical
case still reports syntax. Compatible formula-to-species promotion is now part
of the symbol foundation; conflicting roles still reject.

### Current arithmetic increment

Ordinary right-nested sums/products and verified unary combinations are accepted.
The semantic normalization follows the pinned upstream group construction rather
than flattening all equivalent algebra. Source formatting retains token/grouping
order; evaluation, units and export share the normalized tree. Numeric expression
literals now allow zero or magnitude `[1e-307, 1e308]`, at most 15 significant
mantissa digits. Scientific MathML preserves mantissa/exponent and its numeric
interpretation; no excess-precision rounding or nonzero-underflow clamping is
introduced. Contiguous `--` remains invalid. Numeric builtins and time are supplied
by the separately versioned math extension; user functions and rules remain out
of scope. Unit-coefficient limits are unchanged.

### Current symbol/declaration increment

- Named headers accept `model m` and `model m()` with a newline/semicolon before
  the body; nonempty ports and separator-free same-line bodies remain deferred.
- Formula/species/compartment declarations support compatible source-ordered
  updates, const/var qualifiers and formula-to-species promotion.
- Declaration-only values remain unset; symbolic initial assignments with unset
  dependencies remain symbolic. No zero or compartment size is invented.
- Unnamed reactions receive generated identifiers with reference-checked source
  order and collision handling. Named/unnamed mixtures and law reassignment are
  included. Reaction-valued expressions remain deferred.
- `parameter` is not an upstream declaration keyword. Computed compartment sizes,
  user-function/rule/event/hierarchy support remain outside this increment.
  Numeric builtins/time are provided by the later math extension.
- Historical fixtures and target feature IDs below are unchanged. Explicit
  [native-outcome overlays](SYMBOLS.md#explicit-historical-outcome-overlay) document
  which formerly required rejections are intentionally supported now.

## Included target

| Area | v0.1 contract | Required evidence |
|---|---|---|
| Source | UTF-8 input; upstream ASCII identifier grammar; comments; newline/semicolon termination | tokenizer cases and exact reference comparison |
| Model boundary | One anonymous model OR one named flat `model ... end` definition; no model instances | named/anonymous cases; reject multiple model definitions |
| Symbols | Explicit declarations plus reference-verified implicit species/parameter inference; forward references | order independence; distinguish inferred-but-uninitialized from invalid references |
| Species | Initial values, explicit compartment membership, normal and boundary/constant forms | independent constant/boundary flag snapshots |
| Compartments | Fixed positive scalar sizes; supported implicit/default compartment behavior pinned to oracle | non-unit size and default-compartment cases |
| Parameters | Scalar numeric or supported arithmetic-expression initialization; explicitly checked finite initial values | zero/negative/scientific notation and missing-value cases |
| Reactions | Named/unnamed, fixed numeric stoichiometry, source/sink, reversible and irreversible | exact species stoichiometry, direction and rate tree |
| Expressions | Numeric literals, symbols, parentheses, unary +/- and binary + - * / ^ | precedence, association and MathML cases |
| Initialization | Numeric and acyclic arithmetic initial expressions within this subset | dependency graph and cycle diagnostics |
| Units | Built-in unit identifiers and verified scalar/product/quotient/power unit definitions; unit annotations for supported entities | upstream fixtures; explicit unspecified units; mismatch diagnostics |
| Output | Deterministic canonical Antimony for profile; SBML Level 3 Version 2 Core export | reparse equivalence and independent SBML validation |
| Diagnostics | Source locations, structured codes, supported-profile explanation | negative fixtures and bounded recovery |

The unit grammar and precedence tables must be transcribed into implementation
tests from pinned upstream sources in Phase 0/1 before coding those features.
This is a release requirement, not license to silently support an unverified subset.

## Crucial semantic rules

1. **`->` is reversible in upstream Antimony; `=>` is irreversible.** A
   reversible flag does not create a reverse rate law. The supplied kinetic
   expression remains authoritative. Do not assume a familiar arrow convention.
2. Alternative arrow spellings are not in the initial profile. Source comments
   and lexer spelling appear inconsistent; resolve with executable reference
   cases before accepting aliases.
3. Preserve initialization mode: amount, concentration and substance-only
   semantics are different. No implicit numeric replacement without volume/unit
   rules demonstrated by reference tests. `substanceOnly` itself is deferred.
4. Boundary species and constant species are not synonyms in SBML. Test how
   Antimony `$`, `const`, and `var` map to both flags; preserve those distinctions.
   Specifically, `$S`/`const species S` map to boundaryCondition=true and
   constant=false in the inspected reference cases, not immutable SBML constants.
5. A rate expression may mention a species not consumed by the reaction; inspect
   reference modifier generation. Do not add it as a reactant to simplify runtime.
6. Missing values must remain explicitly unresolved or be rejected by our strict
   profile. Do not invent `k=1`, initial concentration zero, or arbitrary volumes.
   Upstream-defined defaults are allowed only when precisely mapped and reported.
7. Distinguish `=` initialization from `:=` assignment rules and `'=` rate rules.
   This profile accepts initialization only. No conversion of rules into initial values.
8. Repeated/conflicting initialization is explicitly rejected by this strict
   profile until reference behavior is implemented; rejection need not imply the
   source is invalid in full Antimony.
9. Undefined units are not dimensionless. The checker tracks known, unspecified,
   and inconsistent units separately; no complete dimensional-proof claim.
   Unit annotations do not automatically rescale expressions or values. Our
   dimensional analysis is an additional check, not an assertion that upstream
   Antimony automatically infers derived units.
10. Numeric domain/overflow errors in constant initialization are reported.
    Model-dependent expression singularities require runtime evaluation and are
    not claimed eliminated by static validation.

## Explicitly rejected in v0.1

- Function calls (including built-ins), user-defined functions and piecewise
  expressions. Ordinary mass-action, rational saturation and Hill expressions
  can still be expressed with supported arithmetic. Expand later with an explicit
  function/arity/MathML mapping table and reference fixtures.
- Assignment/rate/algebraic rules; events, delays, trigger semantics, `rateOf`.
- Nested modules, instantiation, imports, ports, symbol replacement and deletion.
- Dynamic compartments, conversion-factor constructs, `substanceOnly` species.
- Variable/named stoichiometry and FBC/flux balance optimization.
- Distributions, uncertainty syntax, layout/render instructions.
- Biological annotations, SBO terms and other metadata not represented by this
  profile. Recognize and reject rather than stripping them silently.
- DNA strands, interactions and SBOL-specific constructs.
- SBML import, arbitrary embedded XML/MathML or native code execution.

Declarations such as a human display name are rejected if not represented by the
initial grammar/IR; simple comments are explicitly nonsemantic trivia. We do not
promise arbitrary Antimony text survives formatting byte-for-byte.

## Acceptance contract

- Every included row has positive, edge and negative fixtures.
- Every excluded feature family has at least one explicit rejection fixture.
- Every supported export independently passes SBML checks with no unexplained
  errors. Warnings are categorized, not globally suppressed.
- A feature list reports implemented/passing, failing, unsupported and untested
  separately. No headline "fully compatible" or overall percentage without a
  named version, corpus, denominator and skip reasons.
- Profile restrictions must be included in docs and returned in machine-readable
  diagnostics. Expanding scope increments a versioned profile.

## First example — narrow native/reference conversion verified

```antimony
model first_conversion()
  compartment cell = 1;
  species A in cell, B in cell;
  A = 12;
  B = 0;
  k = 0.2;
  J0: A => B; k*A*cell;
end
```

The `cell` factor is written by the model author, not inserted by the library.
This example omits explicit units for readability and is not a dimensionally
certified biological model. Its native export and pinned-reference projection
now match under independent libSBML inspection, with 12 retained unit warnings;
see [Phase 2 evidence](reviews/phase2-review.md). This does not establish the
complete target contract above.
