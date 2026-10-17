# Symbol Value API Migration (`flat-model-v0.2-slice`)

**Verified bounded profile: `flat-model-v0.2-slice`.**

`antimony` is a static model compiler, not a numerical solver. A `ValidatedModel` confirms structural and semantic validity within `flat-model-v0.2-slice`, not simulation readiness or a biological/clinical certificate.

## Pre-release Return-Type Change

To support declaration-only symbols and symbolic initializers with unset dependencies without inventing values, three model accessors now return `Option<f64>` instead of `f64`:

- `Compartment::size(&self) -> Option<f64>`
- `Species::initial_concentration(&self) -> Option<f64>`
- `Parameter::value(&self) -> Option<f64>`

A missing value **never** becomes `0.0`, and an explicit unset compartment (`compartment c;`) has `size() == None` rather than the implicit `default_compartment` size of `Some(1.0)`.

## Interpreting Optional Values and Initial Expressions

For `Species` and `Parameter`, pair the numeric accessor with `initial_expression() -> Option<ExpressionId>`:

| Numeric accessor | `initial_expression()` | Meaning |
| :--- | :--- | :--- |
| `None` | `None` | Explicitly declared or inferred symbol with no initial value assigned yet. |
| `None` | `Some(expr_id)` | Symbolic initializer (`InitialAssignment`) whose dependencies do not all have known initial values. |
| `Some(val)` | `None` | Direct scalar literal initialization. |
| `Some(val)` | `Some(expr_id)` | Evaluated initialization-time value; the symbolic expression in `initial_assignments()` remains authoritative for SBML export. |

Compartment expressions remain deferred: `Compartment` has no `initial_expression()`, and explicit compartments accept only unset size (`None`) or a positive scalar literal (`Some(size)`). Numeric literals in declarations, initializations, and stoichiometry must have at most 15 significant digits and be zero or have magnitude in `[1e-307, 1e308]`; role-specific checks additionally require positive compartment sizes and positive stoichiometry.

## Example: Parsing, Analyzing, and Reading Optional Values

This fixed example uses `expect`. Applications should match `Result` and render the
returned structured `Diagnostic`; it is not a `std::error::Error` conversion.

```rust
use antimony::{IMPLEMENTED_PROFILE, Limits, SourceFile, SourceId, analyze, parse};

let limits = Limits::default();
let text = "compartment cell; species S in cell = k0 * 2; formula k0; k1 = 0.5; S =>; k1 * S;";
let source = SourceFile::new(SourceId::new(1), "example.ant", text, &limits)
    .expect("bounded source");
let parsed = parse(&source, &limits).expect("valid example syntax");
let model = analyze(&parsed).expect("supported example semantics");

assert_eq!(model.profile_id(), IMPLEMENTED_PROFILE); // "flat-model-v0.2-slice"
assert_eq!(model.compartments()[0].size(), None);    // unset explicit compartment

let s = &model.species()[0];
assert_eq!(s.initial_concentration(), None);         // k0 is unset, never 0.0
assert!(s.initial_expression().is_some());           // symbolic InitialAssignment retained

let k0 = model.parameters().iter().find(|p| p.id() == "k0").unwrap();
assert_eq!((k0.value(), k0.initial_expression()), (None, None));

let k1 = model.parameters().iter().find(|p| p.id() == "k1").unwrap();
assert_eq!((k1.value(), k1.initial_expression()), (Some(0.5), None));
```
