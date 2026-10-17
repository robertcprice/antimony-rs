# Rules Evidence & Oracle Verification Plan (`rules-core-v0.4-slice`)

This document records the oracle projection audit and verification architecture for `rules-core-v0.4-slice`, grounded in [`artifacts/rules-phase4/evidence-design/projection-audit.json`](../artifacts/rules-phase4/evidence-design/projection-audit.json).

## 1. Existing Semantic Projection Audit

The semantic projection in [`tools/oracle/oracle.py`](../tools/oracle/oracle.py) (`antimony-semantic-projection-v1`) was audited against 9 read-only assertions in `artifacts/rules-phase4/evidence-design/projection-audit.json`:

1. **Already represented without schema changes:**
   - Ordered `rules` list (`[{"kind": "assignmentRule" | "rateRule" | "algebraicRule", "variable": ..., "math": ...}]`).
   - Ordered `initial_assignments` list (`[{"symbol": ..., "math": ...}]`), verifying that rules are never misprojected as `<initialAssignment>`.
   - Target `parameters` (`id`, `constant`, `value`, `units`, `substance_units`, `has_only_substance_units`) and `species` (`id`, `compartment`, `initial_concentration`, `initial_amount`, `substance_units`, `has_only_substance_units`, `boundary_condition`, `constant`), capturing `constant=false`, lexical scalar population, and source-order `boundaryCondition` promotion.
   - `reactions` (`id`, `reversible`, `compartment`, `reactants`, `products`, `modifiers`, `kinetic_law`), capturing indirect vs direct rule modifier behavior.

2. **Two Oracle Gaps Identified in `projection-audit.json`:**
   - **Gap 1 (`rule_metadata_currently_unprojected`):** `ALLOWED_XML` in `tools/oracle/oracle.py` permits `metaid` and `sboTerm` on `assignmentRule`, `rateRule`, and `algebraicRule`, while `semantic_projection()` projects only `variable` and `math`. If a model ever emitted rule `metaid` or `sboTerm`, `projection_limits()` did not flag it.
     - **Fix:** In `tools/oracle/oracle.py::projection_limits`, reject any `assignmentRule`, `rateRule`, or `algebraicRule` element whose XML attributes exceed `{"variable"}` with `"Rule metadata is outside projection v1"`.
   - **Gap 2 (`conditional_math_guard_currently_ignores_rules`):** `require_represented_math()` in `tools/oracle/conditional_check.py` inspected `initial_assignments` and `reactions`, but did not inspect `projection.get("rules", [])`.
     - **Fix:** Extend `require_represented_math()` in `tools/oracle/conditional_check.py` to require non-null normalized `math` on every projected rule as well.

## 2. Dedicated Rules Gate (`tools/oracle/rules_check.py`)

In addition to closing the two gaps above, [`tools/oracle/rules_check.py`](../tools/oracle/rules_check.py) enforces rule-specific invariants on the frozen `antimony-rules-corpus-v1` manifest (`fixtures/rules/manifest.json`):

1. **Manifest & Snapshot Integrity:**
   - Every fixture file hash, reference snapshot hash, and `rules-core-v0.4.json` profile hash is verified before running comparisons.
   - Positive (`target_supported`), expected syntax/semantic rejection (`expected_rejection`), and explicit out-of-slice deferral (`unsupported_profile`, including intentionally stricter unit checks where documented) are verified against frozen reference snapshots.
2. **Rule Projection Guards on Positive Exports:**
   - Every positive rule fixture must emit a non-empty `rules` projection where every rule has `kind in {"assignmentRule", "rateRule"}`, a non-empty `variable` referencing an existing species or parameter in the projection, non-null normalized `math`, no duplicate rule targets, and `constant == False` on every rule target.
   - Formatter idempotence (`antimony-rs format`) and semantic preservation across formatting are checked for every positive fixture.
3. **Profile Overlay Discipline (`fixtures/manifests/rules-core-v4-overlay.json`):**
   - Historical manifests (`original-v1.json`, `curated-v1.json`, `conditionals/manifest.json`) and historical evidence receipts (`tools/oracle/conditional-evidence.json`) remain untouched.
   - The two assignment-rule fixtures in `original-v1.json` (`assignment_rule` and `formula_assignment_rule`) transition from `unsupported_profile` (56/34) under `conditional-core-v0.3-slice` to `target_supported` (58/32) under `rules-core-v0.4-slice` via the explicit hash-bound overlay `fixtures/manifests/rules-core-v4-overlay.json`.

## 3. Numerical Trajectory Validation (`tools/numerical/rules_anchors.py`)

Structural SBML acceptance is kept distinct from numerical trajectory validation:
- [`tools/numerical/rules_anchors.py`](../tools/numerical/rules_anchors.py) and [`tools/numerical/rules_check.py`](../tools/numerical/rules_check.py) validate native-exported SBML in RoadRunner `2.10.0` (`.venv-numerical/bin/python`) against:
  1. Upstream Antimony -> SBML -> RoadRunner reference trajectories, and
  2. Separately derived closed-form analytic ODE / algebraic solutions (exponential decay rate rule with dependent assignment rule, coupled linear harmonic/two-state rate rules, species boundary rate rule driving mass-action conversion, and piecewise-switched rate/assignment hybrid trajectory).
