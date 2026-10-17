# Functions Evidence & Oracle Verification Plan (`functions-core-v0.5-slice`)

This document records the oracle projection audit and verification architecture for `functions-core-v0.5-slice`.

## 1. Semantic Projection Audit & Oracle Updates

The semantic projection in [`tools/oracle/oracle.py`](../tools/oracle/oracle.py) (`antimony-rs-oracle-v1`) already projected `<listOfFunctionDefinitions>` as:
```json
{
  "id": "...",
  "name": null,
  "sbo_term": null,
  "annotation": null,
  "math": {
    "type": 267,
    "children": [
      {"type": 260, "name": "p1"},
      "...body AST..."
    ]
  }
}
```
and user-defined function calls as `{"type": 268, "name": "..."}` (`AST_FUNCTION` = `268`, with `"children"` when arguments are present).

Three oracle/gate guards were added before freezing the Phase 5 corpus:
1. **`tools/oracle/oracle.py::projection_limits`:**
   - Rejects `<functionDefinition>` elements that omit `<math>` (`"FunctionDefinition without math is outside projection v1"`), catching upstream empty function bodies (`function f(x) end`, `function f(x) ; end`) as `PROJECTION_UNSUPPORTED`.
   - Rejects unprojected attributes beyond `{"id"}` on `<functionDefinition>` (`"FunctionDefinition metadata is outside projection v1"`).
2. **`tools/oracle/conditional_check.py::require_represented_math`:**
   - Extends the positive non-null math and represented-AST guard to inspect every entry in `projection.get("function_definitions", [])`.
3. **`tools/oracle/rational_reference.py::worker`:**
   - Extends nonfinite MathML (`INF`, `-INF`, `NaN`) scanning to inspect `m.getListOfFunctionDefinitions()` roots so nonfinite literals inside function bodies (`inf`, `nan`, `(1/0)`) are audited as `REFERENCE_NONFINITE`.

## 2. Dedicated Functions Gate (`tools/oracle/functions_check.py`)

[`tools/oracle/functions_check.py`](../tools/oracle/functions_check.py) enforces function-specific structural invariants on the frozen 164-case `antimony-functions-corpus-v1` manifest (`fixtures/functions/manifest.json`):

1. **Manifest & Snapshot Integrity:**
   - Verifies SHA-256 hashes of all 164 `.ant` fixtures, all 164 reference `.json` snapshots, `profiles/functions-core-v0.5.json`, and `fixtures/manifests/functions-core-v5-overlay.json`.
2. **Function Definition Projection Guards (`require_valid_functions_projection`):**
   - Every positive fixture must emit a non-empty `function_definitions` list.
   - Each `function_definition` must have exact keys `{"id", "name", "sbo_term", "annotation", "math"}` with `name=None, sbo_term=None, annotation=None`, unique `id`, and no collision with the model `id` or any compartment, species, parameter, reaction, or unit definition `id`.
   - Each `function_definition["math"]` must have `type == 267` (`AST_LAMBDA`) with `children` where `children[:-1]` are unique `AST_NAME` (`260`) `<bvar>` nodes and `children[-1]` is a non-null body AST.
   - Byte-idempotent formatting (`antimony format`) and exact SBML projection equality on both original and formatted sources are required for all 68 positive cases.
3. **Profile Overlay Discipline (`fixtures/manifests/functions-core-v5-overlay.json`):**
   - Historical manifests (`original-v1.json`, `conditionals/manifest.json`, `rules/manifest.json`) and historical evidence receipts (`tools/oracle/conditional-evidence.json`, `tools/oracle/rules-evidence.json`) remain untouched.
   - The original 90-case corpus, arithmetic, and workload suites bind to `functions-core-v0.5-slice` via the explicit hash-bound overlay `fixtures/manifests/functions-core-v5-overlay.json` (`antimony-native-overlay-v7`, retaining 58/32 on `original-v1.json`).

## 3. Numerical Trajectory Validation (`tools/numerical/functions_anchors.py`)

[`tools/numerical/functions_anchors.py`](../tools/numerical/functions_anchors.py) and [`tools/numerical/function_check.py`](../tools/numerical/function_check.py) validate two closed-form user-defined function models (`function_kinetic_law_and_observable` and `nested_function_rate_and_boundary_rule`) in RoadRunner `2.10.0` against both upstream Antimony reference SBML trajectories and independently hand-derived closed-form analytic equations over 101 grid points on $[0, 10]$.
