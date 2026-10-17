# Contributing to `antimony-rs`

Thank you for your interest in contributing to `antimony-rs`! Our goal is to build a rock-solid, 100% pure-Rust implementation of the [Antimony](https://tellurium.readthedocs.io/en/latest/antimony.html) biological modeling language, bidirectional SBML Level 2 & Level 3 Core translation, and native ODE / discrete-event / Metabolic Control Analysis (MCA) capabilities.

## Architecture First

Before opening a pull request, please read:
- [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) — Directory layout (`src/model/`, `src/semantic/`, `src/sbml/`, `src/simulation/`) and end-to-end compilation/simulation pipeline.
- [`docs/MODULAR_AND_EVENTS.md`](docs/MODULAR_AND_EVENTS.md) — Current language scope (Phases 1–13).

## High-Impact Areas for Contributors

1. **BioModels Corpus & Edge-Case Expansion**
   - Curating additional SBML Level 2 (v1–v5) and Level 3 (v1–v2) models from BioModels and testing roundtrip decompilation (`read_sbml` / `sbml_to_antimony`) and simulation parity.
2. **Metabolic Control Analysis (MCA) Extensions**
   - Expanding [`src/simulation/steady_state.rs`](src/simulation/steady_state.rs) beyond unscaled/scaled elasticity matrices ($\varepsilon_S^v$, $\tilde{\varepsilon}_S^v$) to include flux control coefficients ($C_{e_i}^J$) and concentration control coefficients ($C_{e_i}^S$) via the Reder structural matrix method.
3. **Stiff ODE Solvers & Adaptive Step Control**
   - Adding an optional Rosenbrock / BDF stiff solver alongside the current adaptive RK4 + bisection event locator in [`src/simulation/integrator.rs`](src/simulation/integrator.rs).
4. **Remaining Antimony Language Constructs**
   - Algebraic rules (`0 = ...`), general `delay(x, tau)` expressions, and SBML `comp` package hierarchical export.
5. **WebAssembly (`wasm32-unknown-unknown`) & Python (`PyO3`) Bindings**
   - Because `antimony-rs` has zero C/C++ dependencies and `#![forbid(unsafe_code)]`, it compiles cleanly to WebAssembly for browser-based pathway editors and Tellurium-style notebooks.

## Verification Checklist Before Submitting a PR

Every pull request must pass the native test and lint matrix:

```sh
cargo fmt --all --check
cargo clippy --locked --all-targets --all-features -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --locked --no-deps --all-features
cargo test --locked --all-targets --all-features
cargo test --locked --doc --all-features
cargo test --locked --no-default-features
cargo test --locked --no-default-features --features cli
```

## Non-Negotiable Engineering Rules

- **`#![forbid(unsafe_code)]`**: All library and CLI code must remain 100% safe Rust with zero C/C++ runtime dependencies.
- **No Silent Fallbacks**: Never guess a rate constant, clamp non-finite numbers silently, or drop unsupported SBML/Antimony rules without an explicit `Diagnostic`.
- **Clean Module Hierarchy**: Multi-file subsystems belong in `src/<subsystem>/mod.rs` with focused sibling modules (`model`, `semantic`, `sbml`, `simulation`).
