# Antimony Rust project instructions

## Scope

This project is an independent, pure-Rust implementation of the Antimony biological modeling language, SBML L2/L3 Core translator, and native ODE/event/MCA analysis engine.

Read `README.md`, `docs/ARCHITECTURE.md`, `docs/PROFILE_V0_1.md`, `docs/DESIGN.md` and
`docs/IMPLEMENTATION_PLAN.md` before implementation. Source references are in
`docs/SOURCES.md`; compatibility gates are in `docs/VALIDATION.md`.

## Scientific and language correctness

- Upstream Antimony semantics are the target; do not invent convenient syntax.
- Distinguish valid-but-unsupported Antimony from malformed source.
- Never silently drop rules, events, annotations, units or import statements.
- No guessed constants, unknown-symbol-to-zero fallback, or amount/concentration conflation.
- Use a verified model type to gate export.
- Names, labels and user-supplied source must never cause command execution,
  filesystem reads, or network access in the core parser.
- No native C/C++ runtime dependency or global mutable registry.
- Preserve notices for any copied upstream material; reference license and
  project license approval are separate from implementation progress.

## Claims and workflow

- All design API signatures are proposals until implemented and tested.
- Profile JSON records target scope, never claimed implementation coverage.
- Do not claim native support from successful upstream Python execution.
- No publishing, remote creation, messages to collaborators or licensing decisions
  without authorization. No forced push or unrelated cleanup.
- Keep contributions modular and phase-sized. Record test commands and actual
  pass/fail/skip counts. Skips and unavailable oracles are never passes.
- Monitor context and checkpoint at about 50 tool calls or 20 minutes; give any
  long-running collaborators the same instruction. Persist findings before handoff.
- Summaries explain what was built, why it matters, verified test outcomes,
  concrete next steps and realistic novel opportunities—not a file changelog.
