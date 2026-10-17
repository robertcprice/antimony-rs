# `antimony-rs` Crate Architecture & Repository Map

`antimony-rs` is a 100% pure-Rust compiler, formatter, bidirectional SBML Level 2 / Level 3 translator, stoichiometry analyzer, ODE + discrete-event simulator, steady-state solver, and Metabolic Control Analysis (MCA) elasticity engine for the Antimony biological modeling language.

---

## 1. End-to-End Data Flow

```text
Antimony Source (.ant) ──┬──> syntax::parse(...)               (single-file sandboxed)
                         └──> syntax::parse_with_imports(...)  (multi-file relative imports)
                                      │
                                      ▼
                               ParsedDocument
                                 │          │
                                 │          └──> format::format_antimony(...) ──> Canonical .ant
                                 ▼
                         semantic::analyze(...)
                         (module flattening, symbol table, unit checking, initialization)
                                 │
                                 ▼
                          ValidatedModel  ◄────── sbml::read_sbml(...) ◄── SBML L2/L3 XML
                            │    │    │                                         │
                            │    │    └─────────> sbml::write_sbml(...) ────────┴──> sbml_to_antimony(...)
                            │    │
                            │    ├──────────────> simulation::StoichiometryMatrix
                            │    ├──────────────> simulation::simulate(...)       (RK4 ODE + Events)
                            │    └──────────────> simulation::steady_state(...)   (Steady-State & MCA Elasticities)
```

---

## 2. Source Tree (`src/`)

Every multi-file subsystem in `src/` follows a uniform directory-module layout (`src/<subsystem>/mod.rs`):

```text
src/
├── lib.rs                              # Crate root, public API exports, and safety contract
├── source.rs                           # SourceFile buffer, SourceId, Span, and line/column resolution
├── limits.rs                           # Explicit complexity and resource ceilings (Limits, LimitKind)
├── diagnostic.rs                       # Structured compiler/validator diagnostics (Diagnostic, DiagnosticCode)
├── builtins.rs                         # Built-in mathematical functions (Builtin) and constants (NumericConstant)
├── conditional.rs                      # Relational (ComparisonOp), logical (LogicalOp), and piecewise nodes
│
├── syntax/                             # Lexical analysis, recursive-descent parsing, and import resolution
│   ├── mod.rs                          # Syntax AST definitions and module exports
│   ├── lexer.rs                        # Token scanner with span tracking and token-budget enforcement
│   ├── parser.rs                       # Statement, declaration, reaction, rule, and module block parser
│   ├── expression.rs                   # Iterative Pratt parser for arithmetic, Boolean, and function expressions
│   ├── events.rs                       # Discrete `at` event trigger, delay, option, and assignment parser
│   ├── functions.rs                    # User-defined `function ... end` block parser
│   ├── imports.rs                      # Multi-file `import "other.ant"` loader (`parse_with_imports`, `load_file`)
│   ├── numeric.rs                      # IEEE-754 mantissa/exponent range and precision envelope validator
│   ├── rational.rs                     # Exact rational literal (`(n/d)`) recognizer
│   ├── reserved.rs                     # Reserved Antimony keywords, constants, and builtin identifiers
│   └── units.rs                        # Compound unit-expression parser (`unit u = ...;`, `has ...`)
│
├── semantic/                           # Semantic analysis, module flattening, initialization, and unit checking
│   ├── mod.rs                          # Semantic analyzer entry point (`analyze`)
│   ├── modules.rs                      # Submodel instantiation, port aliasing, `M.x`, `is`, `delete`, `timeconv`/`extentconv`
│   ├── symbols.rs                      # Source-ordered symbol table, role promotion, and reaction overrides
│   ├── initialization.rs               # Acyclic dependency resolution and InitialAssignment synthesis
│   ├── evaluation.rs                   # Compile-time constant/symbolic expression evaluator
│   ├── expression_normalization.rs     # Reference-faithful expression tree normalization
│   ├── units.rs                        # Dimensional analysis and unit compatibility verification
│   └── unit_mapping.rs                 # Species concentration-to-substance unit synthesis
│
├── model/                              # Validated intermediate representation (IR)
│   ├── mod.rs                          # ValidatedModel, Compartment, Species, Parameter, Reaction, Rule, Event
│   └── units.rs                        # Validated UnitDefinition, UnitComponent, and DimensionalStatus
│
├── format/                             # Canonical Antimony source code formatter
│   ├── mod.rs                          # Formatter module root
│   └── antimony.rs                     # Deterministic, byte-idempotent `format_antimony` emitter
│
├── sbml/                               # Bidirectional SBML Level 2 (v1–v5) & Level 3 (v1–v2) Core I/O
│   ├── mod.rs                          # SBML module root (`write_sbml`, `read_sbml`, `sbml_to_antimony`)
│   ├── write.rs                        # SBML L3V2 Core XML serializer
│   ├── read.rs                         # SBML L2/L3 Core reader, BioModels local-parameter lifter, and decompiler
│   └── units.rs                        # SBML `<listOfUnitDefinitions>` XML serializer
│
├── simulation/                         # Pure-Rust numerical engine & Metabolic Control Analysis (MCA)
│   ├── mod.rs                          # Simulation module root and public exports
│   ├── stoichiometry.rs                # Net stoichiometry matrix N (`StoichiometryMatrix`)
│   ├── evaluator.rs                    # Pre-indexed runtime state (`ModelRuntime`) and expression evaluator
│   ├── events.rs                       # Discrete-event trigger crossing, delay queue, and priority execution
│   ├── integrator.rs                   # RK4 ODE + bisection discrete-event time-course solver (`simulate`)
│   └── steady_state.rs                 # Steady-state solver (`steady_state`) and MCA elasticity matrices (`ElasticityMatrix`)
│
└── bin/                                # Unified CLI binary (`antimony`)
    ├── antimony.rs                     # Binary entry point
    └── antimony_cli/                   # Argument parsing, safe staged file I/O, and JSON report formatting
```

---

## 3. Runnable Examples (`examples/`)

| Example | Purpose | Command |
|---|---|---|
| [`examples/export_sbml.rs`](../examples/export_sbml.rs) | Compile an Antimony `.ant` file to SBML L3V2 Core XML | `cargo run --example export_sbml -- INPUT.ant [OUTPUT.xml]` |
| [`examples/import_sbml.rs`](../examples/import_sbml.rs) | Decompile an SBML L2/L3 Core `.xml` file to canonical Antimony | `cargo run --example import_sbml -- INPUT.xml [OUTPUT.ant]` |
| [`examples/format_antimony.rs`](../examples/format_antimony.rs) | Format an Antimony `.ant` file into canonical layout | `cargo run --example format_antimony -- INPUT.ant [OUTPUT.ant]` |
| [`examples/ode_trajectory_csv.rs`](../examples/ode_trajectory_csv.rs) | Load an Antimony model (with relative `import` support) and export its ODE + event trajectory to CSV | `cargo run --example ode_trajectory_csv -- INPUT.ant [T_END] [STEPS] [OUTPUT.csv]` |
| [`examples/events_and_modules.rs`](../examples/events_and_modules.rs) | Walkthrough of modular composition, `delete`, `timeconv`/`extentconv`, events, and SBML roundtripping | `cargo run --example events_and_modules --all-features` |

---

## 4. Integration Test Suites (`tests/`)

| Test Suite | Coverage Area |
|---|---|
| `tests/simulation_and_stoichiometry.rs` | Pure-Rust `StoichiometryMatrix`, RK4 + discrete-event `simulate()`, `steady_state()`, MCA `ElasticityMatrix`, and 62-model corpus simulation |
| `tests/multi_file_imports.rs` | `parse_with_imports` and `load_file` (nested/diamond imports, flat main statements, cycle & path-traversal rejection) |
| `tests/sbml_roundtrip.rs` | Bidirectional `write_sbml -> sbml_to_antimony -> read_sbml` across 400+ models + wild BioModels SBML L2V4 & L3V1 import and simulation |
| `tests/modular_slice.rs` | Phases 6–9 (`substanceOnly`, dynamic compartments, reaction overrides, discrete `at` events, modules, ports, `M.x`, `is`, `delete`, `timeconv`/`extentconv`) |
| `tests/collaboration_robustness.rs` | Independent 36-model pathway collaboration corpus |
| `tests/functions_ergonomics.rs` | User-defined `function ... end` declarations and calls |
| `tests/rules_ergonomics.rs` | Assignment (`:=`) and rate (`'=`) rules |
| `tests/conditionals*.rs` | Boolean comparisons, logical operators, and `piecewise(...)` |
| `tests/builtins*.rs` | Mathematical built-ins, `pi`, `exponentiale`, and `time` |
| `tests/rational*.rs` | Exact rational literals and arithmetic precedence |
| `tests/units*.rs`, `tests/species_units*.rs` | Compound unit definitions, model unit defaults, and dimensional checking |
| `tests/formatting.rs`, `tests/format_review.rs` | Canonical Antimony formatting and byte-idempotence |
| `tests/cli*.rs` | Unified `antimony` CLI contract, JSON report schema, and filesystem safety |
| `tests/foundation.rs`, `tests/hardening.rs`, `tests/vertical_slice.rs` | Resource limits, security hardening, and core compilation slice |
