# Collaboration & Community Brief

## Purpose

Provide a clean, reviewable, pure-Rust implementation of Antimony and SBML L2/L3 translation that can be inspected, tested, and extended alongside the upstream systems biology community.

## Core Design Principles

1. **Zero C/C++ Runtime Dependencies**: 100% safe Rust (`#![forbid(unsafe_code)]`) suitable for native CLI tools, high-performance simulation engines, and WebAssembly targets.
2. **Differential Verification**: Validated across 20 differential gates against `libAntimony` 3.2.0, `libSBML` 5.21.2, and `libRoadRunner` 2.10.0.
3. **Explicit Diagnostics**: Distinguish unsupported language constructs from syntax errors without silent fallbacks or guessed constants.

## Open Semantic & Community Topics

1. Prioritizing remaining Antimony constructs (such as general `delay(...)` expressions and algebraic rules `0 = ...`) based on real-world BioModels and Tellurium workflows.
2. Reviewing edge-case normalization and unit-checking semantics alongside upstream maintainers (Herbert Sauro, Lucian Smith, and the broader SBML/Antimony community).
3. Expanding benchmark coverage across curated BioModels and educational systems-biology curricula.
