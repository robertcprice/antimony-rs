# Substance-Only Species, Dynamic Compartments, Reaction Overrides, Events, Modular Composition, SBML Decompilation, Multi-File Imports, and Pure-Rust Simulation (`modular-core-v0.8-slice`)

**Verified bounded checkpoint (2026-10-15).** The implemented profile is
`modular-core-v0.8-slice`; all twenty maintained structural, CLI, formatter, collaboration,
SBML roundtrip, and numerical gates pass on immutable reviewed binaries alongside the native
pure-Rust simulation and BioModels import test suites.
This extends the flat `functions-core-v0.5-slice` foundation with **Phase 6** (`substanceOnly`
species, symbolic/rule-controlled compartments, and reaction kinetic-law overrides),
**Phase 7** (discrete `at` events), **Phase 8** (multi-module hierarchy, ports, `M.x`
member overrides, `is` aliasing, and reference-faithful flattening to SBML L3V2 Core),
**Phase 9** (`module` keyword synonym, `delete M.x` submodel member deletions, and
`timeconv` / `extentconv` submodel conversion factors), **Phase 10** (native bidirectional
SBML L3V2 Core reading and canonical Antimony decompilation via `read_sbml`, `sbml_to_antimony`,
and `examples/import_sbml.rs`), **Phase 11** (real-world SBML Level 2 v1–v5 and Level 3 v1–v2
BioModels import compatibility, `<notes>`/`<annotation>` skipping, and reaction-scoped local
parameter lifting), **Phase 12** (safe multi-file `import "other.ant"` composition via
`parse_with_imports` and `load_file`), and **Phase 13** (native pure-Rust stoichiometry matrix
extraction `model.stoichiometry_matrix()` and RK4 ODE + discrete-event simulator `model.simulate()`).

---

## 1. What You Can Write (Phases 6–13)

### Phase 6 — `substanceOnly` Species, Dynamic Compartments, and Reaction Overrides

- **`substanceOnly` species (`hasOnlySubstanceUnits="true"`):**
  - Declare amount-based species with `substanceOnly species S in cell, P;`, `var substanceOnly species S = 5;`, or `const substanceOnly species $B = 2;`.
  - Scalar initializers on `substanceOnly` species populate `Species::initial_amount()` and export `initialAmount="..."` on SBML `<species hasOnlySubstanceUnits="true" .../>` instead of `initialConcentration="..."`.
  - `substanceOnly` sticky promotion: redeclaring an existing species with `substanceOnly species S` promotes it to `hasOnlySubstanceUnits="true"`, while later plain `species S = 10` initializers preserve the `substanceOnly` flag and set `initialAmount="10"`.
- **Symbolic and dynamic compartments (`=`, `:=`, `'=`):**
  - Compartments can be initialized via symbolic expressions (`compartment c = 2 * v0;`, `v0 = 1.5;`), emitting an SBML `<initialAssignment symbol="c">` while omitting the static `size` attribute when unresolvable or governed by an assignment rule.
  - Compartments can be controlled by continuous assignment rules (`compartment c; c := 1 + 0.5 * time;`) or rate rules (`compartment c = 1; c' = 0.1 * c;`), with `constant="false"` automatically set on `<compartment>` and full participation in static/transient cycle detection and unit checking.
- **Reaction kinetic-law overrides (`J = ...` and `J := ...`):**
  - Named reactions and generated unnamed reaction IDs (`_J0`, `_J1`, ...) can have their empty or prior kinetic law set or replaced later in source order via `J = k2 * S;` or `J := k2 * S;`.
  - Updates the reaction's `<kineticLaw>` in place without creating a spurious parameter or `<assignmentRule>`, and re-runs participant discovery, auto-parameter inference, and kinetic-law unit checking.

### Phase 7 — Discrete Events (`at` Triggers, `after` Delays, Options, and Event Assignments)

- **Named and unnamed events:**
  - `at (time >= 5): S = S + 10, k = 0.5;`
  - `E_bolus: at 0.5 after (time > 2), t0 = false, persistent = false, fromTrigger = false, priority = 10: X = 0;`
  - Unnamed events receive deterministic reference-matching generated IDs (`_E0`, `_E1`, ...), advancing past any user-declared `_E{n}` symbols in source order.
- **Trigger expressions, `after` delays, and event options:**
  - Any Boolean-valued expression (`>`, `>=`, `<`, `<=`, `==`, `!=`, `&&`, `||`, `!`, `true`, `false`, `True()`, `False()`, or `piecewise` returning Boolean) is accepted in `at <trigger>: ...` or `at <delay> after <trigger>: ...` and exported under `<trigger initialValue="..." persistent="..."><math>...</math></trigger>`.
  - Supports `<delay><math>...</math></delay>` (`at <delay> after <trigger>`), `<priority><math>...</math></priority>` (`priority = <expr>`), `initialValue` (`t0 = true/false`), `persistent` (`persistent = true/false`), and `useValuesFromTriggerTime` (`fromTrigger = true/false`).
  - Numeric triggers (`at (time + 1): ...`) reject with `DiagnosticCode::UnsupportedFeature`.
- **Sequential event assignments (`target = expr, ...`):**
  - Event assignments may target variable parameters, compartments, or species (`constant="false"`).
  - Targeting a `const` symbol (`const k = 1; at (time > 1): k = 2;`), an assignment-rule target (`x := 2; at (time > 1): x = 3;`), a reaction ID, a function, or a unit rejects with `DiagnosticCode::DeclarationConflict`.
  - Duplicate targets within the same event (`at (time > 1): x = 1, x = 2;`) reject with `DiagnosticCode::DeclarationConflict`.
  - Undeclared symbols assigned on an event LHS (`at (time > 1): z = 5;`) are inferred as non-constant model parameters (`constant="false"`), matching upstream Antimony `3.2.0`.

### Phase 8 — Reusable Modules, Submodels, Ports, `M.x` Member Access, and `is` Aliasing

- **Reusable module definitions (`model Name(ports...) ... end` and `module Name(ports...) ... end`):**
  - Define zero or more reusable `model` or `module` (`model *Name(...)` / `module *Name(...)`) blocks in a file, where the last explicit block (or the last `*`-marked block) becomes the exported root model, matching Antimony `3.2.0`.
  - Formal port headers (`model step(S, P, k) ... end`) bind to internal species, parameters, or compartments and expose them for positional argument wiring at instantiation sites.
- **Named and anonymous submodel instantiations:**
  - `A: sub(S1, S2, k_a);` instantiates `sub` under prefix `A_` and aliases formal ports `S -> S1`, `P -> S2`, `k -> k_a`.
  - `sub(S1, S2);` (anonymous instantiation) receives deterministic generated submodel prefixes (`_sys0_`, `_sys1_`, ...).
  - Multi-level hierarchy (`leaf -> mid -> top`) flattens transitively in declaration order with `A_B_x` prefixing, while port-aliased symbols adopt the host symbol's unprefixed `SId`.
- **Hierarchical member access (`M.x`) and `is` aliasing:**
  - Read or override submodel members from the parent model: `A.k = 0.25;`, `A.S = 10;`, `A.x := 2 * u;`, `A.x' = -0.1 * A.x;`, `total := A.S + B.S;`, or `A.x is shared_x;`.
  - Local same-scope aliasing (`a is b;`) and port-bound aliasing (`A: sub(x);` or `A.x is x;`) merge symbol declarations, initializers, rules, reactions, and event assignments according to Antimony source order (host overrides applied after `A: sub(...)` win over submodel defaults).
  - Cyclic module instantiations (`A` instantiating `A`, or `A -> B -> A`) and forward module instantiations reject with `DiagnosticCode::DeclarationConflict` / `DiagnosticCode::UnsupportedFeature`.

### Phase 9 — `module` Synonym, `delete M.x` Submodel Deletions, and `timeconv` / `extentconv` Conversion Factors

- **`module` keyword synonym:**
  - `module Name(ports...) ... end` and `module *Name(ports...) ... end` are accepted identically to `model` blocks and preserved by `format_antimony`.
- **Submodel member deletions (`delete M1.x, M1.y;`):**
  - Delete unwanted submodel reactions, events, rules, species, parameters, or compartments from an instantiated submodel (`delete M1.J_leak, M1.W;`).
  - Deleting top-level variables (`delete x;`), port-synchronized submodel variables (`M1: sub(A); delete M1.S;`), or submodel symbols still referenced by remaining reactions/rules/events rejects at compile time with `DiagnosticCode::DeclarationConflict` (preventing dangling SBML references).
- **Submodel conversion factors (`timeconv = tf`, `extentconv = ef`):**
  - `M1: sub(A), timeconv = tf, extentconv = ef;` scales submodel reaction kinetic laws by `* ef / tf`, submodel rate rules by `/ tf`, explicit `time` symbols inside submodel expressions by `/ tf`, and submodel event delays by `* tf`, matching Antimony `3.2.0` and SBML Comp V1 flattening semantics.
  - Conversion factors must be named parameter symbols (`Formula`); using species/compartments rejects with `DiagnosticCode::DeclarationConflict`, and literal numeric conversion factors (`timeconv = 60`) reject with `DiagnosticCode::UnsupportedFeature` to avoid an upstream `libAntimony` bug where multiple literal conversion factors overwrite a single shared `timeconv` parameter.

### Phase 10 & Phase 11 — Native Bidirectional SBML L2/L3 Core Reader, BioModels Compatibility, and Canonical Antimony Decompiler

- **`sbml_to_antimony(xml: &str, limits: &Limits) -> Result<String, Diagnostic>`:**
  - Parses SBML Level 2 (Versions 1–5) and SBML Level 3 (Versions 1–2) Core XML (`<listOfFunctionDefinitions>`, `<listOfUnitDefinitions>`, `<listOfCompartments>`, `<listOfSpecies>`, `<listOfParameters>`, `<listOfInitialAssignments>`, `<listOfRules>`, `<listOfReactions>`, `<listOfEvents>`, and model unit defaults) and decompiles it into canonical, byte-idempotent Antimony source text.
  - Safely skips `<notes>` and `<annotation>` metadata subtrees while enforcing `LimitKind::Tokens` and `LimitKind::ExpressionDepth` on skipped XML tags.
  - Lifts reaction-scoped local parameters (`<listOfLocalParameters>` in L3 and `<listOfParameters>` inside `<kineticLaw>` in L2) to model scope (`{rxn_id}_{param_id}` when shadowed) and rewrites `<ci>` references inside the reaction's kinetic law.
- **`read_sbml(xml: &str, limits: &Limits) -> Result<ValidatedModel, Diagnostic>`:**
  - Parses and validates SBML L2/L3 Core XML directly into a native `ValidatedModel` with full resource-limit enforcement, security guards against `<!DOCTYPE`/`<!ENTITY`, and roundtrip fidelity (`write_sbml(&read_sbml(xml)?) == xml`).
- **Standalone CLI example (`examples/import_sbml.rs`):**
  - `cargo run --example import_sbml -- INPUT.xml [OUTPUT.ant]` reads an SBML L2/L3 Core XML file, validates it via `read_sbml`, and emits canonical Antimony source.

### Phase 12 — Safe Multi-File `import "other.ant"` Composition

- **`parse_with_imports(source, limits, resolver) -> Result<ParsedDocument, Diagnostic>` and `load_file(path, limits) -> Result<ValidatedModel, Diagnostic>`:**
  - Resolves top-level `import "relative/path.ant"` directives recursively before compiling the root model.
  - Supports nested imports, diamond-import deduplication, flat top-level main statements following imported `model`/`module` definitions, cycle detection (`DiagnosticCode::DeclarationConflict`), path-traversal/absolute-path rejection (`DiagnosticCode::UnsupportedFeature`), and cumulative `LimitKind::SourceBytes` enforcement across all imported files.
  - Single-file `parse(&source, &limits)` remains strictly sandboxed and rejects `import` directives when no resolver is supplied.

### Phase 13 — Pure-Rust Stoichiometry Matrix, Native ODE + Event Simulator, Steady-State Solver, & MCA Elasticities

- **`ValidatedModel::stoichiometry_matrix(&self) -> StoichiometryMatrix`:**
  - Extracts the dense $N_{\text{floating}} \times N_{\text{reactions}}$ net stoichiometry matrix in `Species` and `Reaction` declaration order (`floating_species()`, `boundary_species()`, `reactions()`, `net_stoichiometry(s, r)`, `as_rows()`).
- **`ValidatedModel::simulate(&self, config: &SimulationConfig) -> Result<Trajectory, Diagnostic>` (and `simulate(model, config)`):**
  - Pure-Rust 4th-order Runge-Kutta (RK4) ODE + bisection (`1e-10`) discrete-event simulator with zero C/C++ dependencies (`src/simulation/integrator.rs`).
  - Integrates substance amounts $n_S$ ($\frac{dn_S}{dt} = \sum_j N_{S,j} v_j$) for reaction-governed floating species with concentration $[S] = n_S / V_c$, supports dynamic compartments, rate rules, topologically sorted initial assignments and assignment rules, user-defined functions, `piecewise`, and discrete `at` events (including `after` delays, `fromTrigger`, `persistent`, `t0`, and `priority`).
  - Exports time-series columns (`time`, floating species, dynamic compartments, dynamic parameters) to CSV via `Trajectory::to_csv()`.
- **`ValidatedModel::steady_state(&self, config: &SteadyStateConfig) -> Result<SteadyState, Diagnostic>` & `ElasticityMatrix`:**
  - Solves for the steady state ($\frac{d\mathbf{x}}{dt} = \mathbf{0}$) while preserving moiety conservation laws via pseudo-transient RK4 continuation and damped Newton-Raphson polishing (`src/simulation/steady_state.rs`).
  - Computes Metabolic Control Analysis (MCA) unscaled ($\frac{\partial v_j}{\partial S_i}$) and scaled ($\varepsilon_{S_i}^{v_j} = \frac{\partial v_j}{\partial S_i} \frac{S_i}{v_j}$) elasticity matrices via `model.elasticity_matrix()` or `steady_state.elasticity_matrix(&model)`.

---

## 2. Public Rust API (`modular-core-v0.8-slice`)

In addition to the existing `ValidatedModel` queries (`compartments()`, `species()`, `parameters()`,
`reactions()`, `initial_assignments()`, `rules()`, `function_definitions()`, `unit_definitions()`),
Phases 6–13 expose:

- `Compartment::initial_expression(&self) -> Option<&Expression>` — symbolic `=` initializer on a compartment when exported via `<initialAssignment>`.
- `Species::has_only_substance_units(&self) -> bool` — `true` when declared with `substanceOnly`.
- `Species::initial_amount(&self) -> Option<f64>` — resolved scalar `initialAmount` for `substanceOnly` species (distinct from `Species::initial_concentration()`).
- `ValidatedModel::events(&self) -> &[Event]` — slice of validated discrete events in source order:
  - `Event::id(&self) -> &str`
  - `Event::is_generated_id(&self) -> bool`
  - `Event::use_values_from_trigger_time(&self) -> bool`
  - `Event::delay(&self) -> Option<&Expression>`
  - `Event::priority(&self) -> Option<&Expression>`
  - `Event::trigger(&self) -> &EventTrigger` (`EventTrigger::expression()`, `initial_value()`, `persistent()`)
  - `Event::assignments(&self) -> &[EventAssignment]` (`EventAssignment::variable()`, `expression()`, `source_span()`)
- `read_sbml(xml: &str, limits: &Limits) -> Result<ValidatedModel, Diagnostic>` (under feature `sbml`) — parse and validate SBML L2/L3 Core XML into a `ValidatedModel`.
- `sbml_to_antimony(xml: &str, limits: &Limits) -> Result<String, Diagnostic>` (under feature `sbml`) — decompile SBML L2/L3 Core XML into canonical Antimony text.
- `parse_with_imports(source: &SourceFile, limits: &Limits, resolver: F) -> Result<ParsedDocument, Diagnostic>` — parse an Antimony file with a caller-supplied relative import callback.
- `load_file(path: &Path, limits: &Limits) -> Result<ValidatedModel, Diagnostic>` — load, resolve relative `import "..."` files, and validate an Antimony model from disk.
- `ValidatedModel::stoichiometry_matrix(&self) -> StoichiometryMatrix` — extract the net stoichiometry matrix for floating species across reactions.
- `ValidatedModel::simulate(&self, config: &SimulationConfig) -> Result<Trajectory, Diagnostic>` — run the pure-Rust RK4 + discrete-event simulator.
- `ValidatedModel::steady_state(&self, config: &SteadyStateConfig) -> Result<SteadyState, Diagnostic>` — solve for the model's steady state ($\frac{d\mathbf{x}}{dt} = \mathbf{0}$) and reaction fluxes.
- `ValidatedModel::elasticity_matrix(&self) -> Result<ElasticityMatrix, Diagnostic>` — compute the unscaled and scaled MCA elasticity matrices.

Run the standalone examples exercising Phases 6–13:

```sh
cargo run --example events_and_modules --all-features
cargo run --example import_sbml --all-features -- target/events_and_modules.xml
cargo run --example ode_trajectory_csv --all-features -- fixtures/modular/cases/event_unnamed_basic_bolus.ant 10 100
```

---

## 3. Try an Ordinary Modular + Event File

```antimony
function mm(s, vmax, km)
  vmax * s / (km + s)
end

module Stage(S, P)
  compartment c = 1;
  species S in c, P in c, W in c = 0;
  vmax = 1.2;
  km = 0.5;
  k_leak = 0.05;
  J: S => P; mm(S, vmax, km);
  J_leak: S => W; c * k_leak * S;
end

module *Pathway()
  compartment c = 1;
  substanceOnly species X in c = 4;
  species Y in c = 0, Z in c = 0;
  k_in = 0.5;
  tf = 2;
  ef = 1;

  J_in: X => Y; k_in * X;
  S1: Stage(Y, Z), timeconv = tf, extentconv = ef;
  delete S1.J_leak, S1.W;
  S1.vmax = 2.0;

  E_pulse: at (time >= 5): X = X + 2;
end
```

Check, format, export, roundtrip-import, or simulate in pure Rust:

```sh
cargo run --features cli --bin antimony -- check pathway.ant
cargo run --features cli --bin antimony -- format pathway.ant
cargo run --features cli --bin antimony -- export pathway.ant -o pathway.xml
cargo run --example import_sbml -- pathway.xml pathway_roundtrip.ant
cargo run --example ode_trajectory_csv -- pathway.ant 10 100 pathway.csv
```

---

## 4. Retained Exclusions

The following features remain outside `modular-core-v0.8-slice` and reject deterministically:

- Single-file sandboxed `parse(&source, &limits)` rejects `import "other.ant"` unless `parse_with_imports` or `load_file` is used
- Literal numeric submodel conversion factors (`timeconv = 60`; conversion factors must be named parameter symbols) and per-variable conversion factors (`S1 is S2 * k`)
- Display names via `is "Label"` (`S is "Glucose"`)
- Delay expressions in general formulas (`delay(x, tau)`)
- Algebraic rules (`0 = x + y - 10`)
- Unflattened SBML Hierarchical Model Composition (`comp` package XML); `antimony-rs` exports and reads the flattened SBML Core model matching `antimony.getSBMLString()`.

---

## 5. Verification Evidence (`modular-core-v0.8-slice`)

- **Frozen 48-case modular/events/Phase-6–9 corpus (`fixtures/modular/manifest.json`):**
  - **31 complete simulation-ready models** (`complete_export`): exact SBML L3V2 structural parity against Antimony `3.2.0` (`python-libsbml` `5.21.2`), byte-idempotent `format_antimony` roundtrips, and `libroadrunner` `2.10.0` trajectory parity over $t \in [0, 10]$ (`max_abs_diff <= 1e-12`).
  - **2 incomplete draft models** (`incomplete_export`): unset `substanceOnly` species amount and unset port-linked submodel parameter, verified for exact SBML structural parity without inventing fake zero defaults.
  - **15 negative cases** (`expected_rejection`): cyclic submodels, unknown submodel members, forward module calls, numeric event triggers, constant/assignment-rule event targets, duplicate same-event targets, reaction-override cycles, compartment rule conflicts, deleting referenced or port-bound submodel members, and non-parameter conversion factors.
- **Bidirectional SBML L3V2 Core roundtrip gate (`tools/oracle/sbml_roundtrip_check.py` and `tests/sbml_roundtrip.rs`):**
  - All **55 accepted modular + collaboration models** (`33` modular + `22` collaboration: `49` complete + `6` incomplete) pass `export -> import_sbml -> export` with byte-identical SBML, identical `python-libsbml` structural projections, and **49/49 `libroadrunner` trajectory matches**.
  - Native Rust integration test `tests/sbml_roundtrip.rs` additionally verifies byte-identical `write_sbml -> sbml_to_antimony -> read_sbml -> write_sbml` roundtrips across **400+ positive models** spanning all 9 fixture directories (`accepted`, `arithmetic`, `builtins`, `rational-calls`, `conditionals`, `rules`, `functions`, `collaboration/cases`, `modular/cases`).
- **2 closed-form numerical modular/Phase-6 anchor models (`fixtures/numerical/modular/manifest.json`):**
  - `substance_only_reaction_override_decay`: couples a `substanceOnly` species $A(t) = 6e^{-0.4t}$, a reaction kinetic-law override (`J1 = k_fast * A`), a dynamic compartment rate rule $c(t) = 2 + 0.25t$, and an observable assignment rule $c_{\text{obs}}(t) = A(t) / c(t)$.
  - `modular_port_and_member_override_cascade`: two-stage submodel cascade (`S1: stage(X, Y); S2: stage(Y, Z);`) with `S1.k = 0.5`, `S2.k = 0.25`, `total := X + Y + Z`, verified against its closed-form bi-exponential trajectory at 101 timepoints.
- **19 newly accepted historical cases bound via `fixtures/manifests/modular-core-v8-overlay.json` (`antimony-native-overlay-v8`):**
  - 2 in `original-v1.json` (`event`, `init_compartment_expression`), 1 in `symbols-v2.json` (`deferred_compartment_expression`), 1 in `conditionals/manifest.json` (`conditional_compartment_deferred`), 8 in `rules/manifest.json` (compartment & reaction rules), 3 in `functions/manifest.json` (compartment & reaction function calls), and 4 in `collaboration/manifest.json` (event, dynamic compartment, and reaction-override models). All 19 match their unmodified frozen reference `.json` snapshots and RoadRunner trajectories while historical profile replays remain 100% intact.

| Maintained gate | Total cases | Verified outcome (`modular-core-v0.8-slice`) | Failures / blocked |
|---|---:|---|---:|
| `modular` (`tools/oracle/modular_check.py`) | 48 | 33 exact exports (31 complete + 2 incomplete) / 15 expected rejections | 0 |
| `sbml-roundtrip` (`tools/oracle/sbml_roundtrip_check.py`) | 55 | 55 byte-identical SBML $\leftrightarrow$ Antimony roundtrips + 49/49 RoadRunner trajectories PASS | 0 |
| `modular-numerical` (`tools/numerical/modular_check.py`) | 2 | 2 closed-form Phase 6–8 analytic trajectories PASS | 0 |
| `collaboration` (`tools/oracle/collaboration_check.py`) | 36 | 26 exact exports (22 complete + 4 incomplete) / 10 expected rejections | 0 |
| `functions` (`tools/oracle/functions_check.py`) | 164 | 71 exact exports / 93 expected rejections | 0 |
| `functions-numerical` (`tools/numerical/function_check.py`) | 2 | 2 closed-form ODE/function trajectories PASS | 0 |
| `rules` (`tools/oracle/rules_check.py`) | 200 | 122 exact exports / 78 expected rejections | 0 |
| `rules-numerical` (`tools/numerical/rule_check.py`) | 2 | 2 closed-form ODE/rule trajectories PASS | 0 |
| `conditional` (`tools/oracle/conditional_check.py`) | 262 | 160 exact exports / 102 expected rejections | 0 |
| `piecewise-numerical` (`tools/numerical/piecewise_check.py`) | 1 | 1 continuous-rate piecewise trajectory PASS | 0 |
| `rational` (`tools/oracle/rational_check.py`) | 124 | 73 exact exports / 51 expected rejections | 0 |
| `original` (`tools/oracle/native_check.py --current-profile`) | 90 | 60 exact exports / 30 expected rejections | 0 |
| `formatter` (`tools/oracle/roundtrip_check.py --current-profile`) | 90 | 60 byte-idempotent roundtrips / 30 rejections | 0 |
| `cli` (`tools/oracle/cli_check.py --current-profile`) | 90 | 60 supported CLI / 30 expected rejections | 0 |
| `arithmetic` (`tools/oracle/expression_check.py --current-profile`) | 54 | 48 exact exports / 6 expected rejections | 0 |
| `symbols` (`tools/oracle/symbols_check.py`) | 66 | 55 exact exports / 11 expected rejections | 0 |
| `builtins` (`tools/oracle/builtins_check.py`) | 109 | 66 exact exports / 43 expected rejections | 0 |
| `workload` (`tools/workloads/check.py --current-profile`) | 86 | 74 supported (6 motifs + 68 generated) / 12 rejections | 0 |
| `numerical` (`tools/numerical/check.py`) | 8 | 8 original analytic trajectories PASS | 0 |
| `builtin-numerical` (`tools/numerical/builtin_check.py`) | 2 | 2 builtin/time analytic trajectories PASS | 0 |
