#!/usr/bin/env python3
"""Isolated, development-only RoadRunner adapter; never installs dependencies.

Invoke the pinned interpreter with ``-I /absolute/path/worker.py`` and one JSON
request on stdin. The parent owns the deadline, isolated working directory,
output cap, trusted-fixture provenance, and independent analytic expectations.
This is process isolation, not an OS sandbox or a general untrusted SBML runner.
"""
from __future__ import annotations

import hashlib
import importlib.metadata
import inspect
import json
import math
import platform
import re
import sys
import xml.etree.ElementTree as ET

SCHEMA = "antimony-numerical-worker-v1"
MAX_INPUT_BYTES = 8 * 1024 * 1024
MAX_SBML_BYTES = 4 * 1024 * 1024
PACKAGES = {"libroadrunner": "2.10.0", "numpy": "2.2.6"}
SOLVER = {"name": "cvode", "stiff": True, "relative_tolerance": 1e-10,
          "absolute_tolerance": 1e-12, "maximum_num_steps": 100000,
          "variable_step_size": False}


def unique_object(pairs):
    """Reject duplicate JSON keys rather than silently changing a request."""
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError(f"duplicate JSON key: {key}")
        result[key] = value
    return result


def validate(request):
    """Validate bounded inline SBML and the single reviewed simulation protocol."""
    fields = {"schema", "sbml", "sbml_sha256", "start", "end", "steps",
              "selections", "solver"}
    if not isinstance(request, dict) or set(request) != fields:
        raise ValueError("request fields do not match the worker v1 schema")
    if request["schema"] != SCHEMA:
        raise ValueError("unsupported worker schema")
    sbml = request["sbml"]
    if not isinstance(sbml, str) or len(sbml.encode("utf-8")) > MAX_SBML_BYTES:
        raise ValueError("SBML must be inline UTF-8 text within the byte limit")
    digest = hashlib.sha256(sbml.encode("utf-8")).hexdigest()
    if request["sbml_sha256"] != digest:
        raise ValueError("SBML SHA-256 mismatch")
    if re.search(r"<!\s*(?:DOCTYPE|ENTITY)\b", sbml, re.I):
        raise ValueError("DTD and entity declarations are forbidden")
    root = ET.fromstring(sbml)
    if root.tag != "{http://www.sbml.org/sbml/level3/version2/core}sbml":
        raise ValueError("expected an inline SBML Level 3 Version 2 core document")
    if root.get("level") != "3" or root.get("version") != "2":
        raise ValueError("unexpected SBML level/version")
    if any(type(request[k]) not in (int, float) or request[k] != v
           for k, v in (("start", 0), ("end", 10), ("steps", 100))):
        raise ValueError("reviewed grid is start=0, end=10, steps=100")
    if type(request["steps"]) is not int:
        raise ValueError("steps must be an integer")
    solver = request["solver"]
    if not isinstance(solver, dict) or solver != SOLVER or any(
            type(solver[k]) is not type(v) for k, v in SOLVER.items()):
        raise ValueError("solver settings must match the reviewed CVODE contract")
    sels = request["selections"]
    if (not isinstance(sels, list) or not 2 <= len(sels) <= 64
            or sels[0] != "time" or any(not isinstance(s, str) for s in sels)):
        raise ValueError("expected 2..64 explicit selections beginning with time")
    if len(set(sels)) != len(sels) or any(
            len(s) > 256 or not re.fullmatch(r"(?:[A-Za-z_][A-Za-z_0-9]*|\[[A-Za-z_][A-Za-z_0-9]*\])", s)
            for s in sels):
        raise ValueError("selections must be unique time/species/parameter identifiers")
    return digest


def plain(value):
    """Convert NumPy scalars/vectors without allowing NaN/Infinity in reports."""
    if hasattr(value, "tolist"):
        value = value.tolist()
    if isinstance(value, (tuple, list)):
        return [plain(v) for v in value]
    if isinstance(value, float) and not math.isfinite(value):
        raise ValueError("nonfinite simulator output")
    if value is None or type(value) in (str, int, float, bool):
        return value
    raise ValueError(f"unreportable simulator value type: {type(value).__name__}")


def run(request):
    """Run one model using a fresh RoadRunner; analytic truth stays in the parent."""
    response = {"schema": SCHEMA, "status": "ORACLE_ERROR"}
    try:
        digest = validate(request)
        response["sbml_sha256"] = digest
    except (ValueError, TypeError, ET.ParseError, UnicodeError) as exc:
        response["reason"] = f"invalid request: {exc}"
        return response
    try:
        packages = {p: importlib.metadata.version(p) for p in PACKAGES}
        if packages != PACKAGES:
            raise ImportError(f"pinned package versions required: {PACKAGES}; found {packages}")
        import roadrunner
        import numpy  # noqa: F401 -- import verifies the installed binary dependency
    except (ImportError, importlib.metadata.PackageNotFoundError, OSError) as exc:
        response.update(status="ORACLE_BLOCKED", reason=str(exc))
        return response
    try:
        rr = roadrunner.RoadRunner(request["sbml"])
        rr.conservedMoietyAnalysis = False
        rr.setIntegrator("cvode")
        integrator = rr.getIntegrator()
        for key, value in SOLVER.items():
            if key != "name":
                integrator.setValue(key, value)
        settings = {key: plain(integrator.getValue(key)) for key in integrator.getSettings()}
        settings["name"] = integrator.getName()
        if any(settings.get(key) != value for key, value in SOLVER.items()):
            raise ValueError("CVODE setting readback differs from request")
        response["metadata"] = {
            "packages": packages, "python": platform.python_version(),
            "platform": platform.platform(), "machine": platform.machine(),
            "roadrunner_version": roadrunner.getVersionStr(),
            "extended_version": rr.getExtendedVersionInfo(),
            "api_signatures": {"RoadRunner": str(inspect.signature(roadrunner.RoadRunner)),
                               "simulate": str(inspect.signature(rr.simulate))},
            "conserved_moiety_analysis": bool(rr.conservedMoietyAnalysis),
        }
        response["solver_readback"] = {key: settings[key] for key in SOLVER}
        response["all_integrator_settings"] = settings
        response["floating_species_ids"] = list(rr.model.getFloatingSpeciesIds())
        response["effective_absolute_tolerance_before"] = plain(integrator.getAbsoluteToleranceVector())
        result = rr.simulate(start=request["start"], end=request["end"],
                             steps=request["steps"], selections=request["selections"])
        response["column_names"] = list(result.colnames)
        response["selections"] = list(request["selections"])
        if result.shape != (101, len(request["selections"])):
            raise ValueError("unexpected simulator output shape")
        if response["column_names"] != request["selections"]:
            raise ValueError("simulator selection columns differ from request")
        response["timeseries"] = plain(result)
        if any(abs(row[0] - index / 10.0) > 1e-12
               for index, row in enumerate(response["timeseries"])):
            raise ValueError("simulator returned an unexpected time grid")
        # This API recomputes from CURRENT amounts, not a stored initial solver vector.
        response["effective_absolute_tolerance_after"] = plain(integrator.getAbsoluteToleranceVector())
        response["status"] = "PASS"
    except Exception as exc:  # Native simulator exceptions must not masquerade as success.
        response["reason"] = f"simulation failed: {type(exc).__name__}: {exc}"
    return response


def main():
    """Read one capped JSON request and write exactly one finite JSON response."""
    try:
        data = sys.stdin.buffer.read(MAX_INPUT_BYTES + 1)
        if len(data) > MAX_INPUT_BYTES:
            raise ValueError("request exceeds byte limit")
        request = json.loads(data.decode("utf-8"), object_pairs_hook=unique_object,
                             parse_constant=lambda value: (_ for _ in ()).throw(ValueError(value)))
        response = run(request)
    except (ValueError, UnicodeError, OSError) as exc:
        response = {"schema": SCHEMA, "status": "ORACLE_ERROR", "reason": f"invalid input: {exc}"}
    try:
        print(json.dumps(response, sort_keys=True, allow_nan=False))
    except (OSError, ValueError):
        return 1
    return 0 if response["status"] == "PASS" else 1


if __name__ == "__main__":
    sys.exit(main())
