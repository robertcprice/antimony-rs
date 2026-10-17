#!/usr/bin/env python3
"""Independent collaboration & pathway verification gate for `functions-core-v0.5-slice`.

Verifies 36 hand-authored pathway, incomplete-draft, and negative near-neighbor
models across Phases 1-5 (`arithmetic`, `symbols`, `builtins/time/rationals`,
`conditionals/piecewise`, `rules`, and `user-defined functions`). Explicitly
distinguishes simulation-ready complete models (`complete_export`), structurally
valid incomplete drafts (`incomplete_export`), and expected rejections
(`expected_rejection`). Optionally runs differential RoadRunner trajectory
comparisons on all complete-initial-state models when `--roadrunner-python` is
supplied.
"""
from __future__ import annotations
import argparse
import json
import os
import re
import subprocess
import profile_overlay
from pathlib import Path

from oracle import ROOT, digest, emit, run
from corpus import fixture_source, snapshot
from cli_check import cli_process
from functions_check import require_valid_functions_projection

MANIFEST = ROOT / 'fixtures/collaboration/manifest.json'
PROFILE = 'functions-core-v0.5-slice'


def is_simulation_ready_projection(projection: dict) -> bool:
    """Return True iff every compartment, species, and parameter has a known initial state or rule."""
    init_symbols = {ia['symbol'] for ia in projection.get('initial_assignments', [])}
    assign_rule_vars = {
        r['variable'] for r in projection.get('rules', []) if r.get('kind') == 'assignmentRule'
    }
    determined = init_symbols | assign_rule_vars
    for comp in projection.get('compartments', []):
        if comp.get('size') is None and comp['id'] not in determined:
            return False
    for spec in projection.get('species', []):
        has_scalar = spec.get('initial_concentration') is not None or spec.get('initial_amount') is not None
        if not has_scalar and spec['id'] not in determined:
            return False
    for param in projection.get('parameters', []):
        if param.get('value') is None and param['id'] not in determined:
            return False
    return True


def load_manifest(path=MANIFEST):
    if profile_overlay.PROFILE not in (PROFILE, 'modular-core-v0.8-slice'):
        raise ValueError('Collaboration gate requires explicit functions-core-v0.5-slice or successor configuration')
    manifest = json.loads(path.read_text())
    if set(manifest) != {'schema', 'implemented_profile', 'provenance', 'fixtures'}:
        raise ValueError('Unknown collaboration corpus fields')
    if manifest.get('schema') != 'antimony-collaboration-corpus-v1' or manifest.get('implemented_profile') != PROFILE:
        raise ValueError('Unexpected collaboration corpus/profile')
    validate_rows(manifest.get('fixtures'))
    return manifest


def validate_rows(rows):
    if not rows or not isinstance(rows, list):
        raise ValueError('Empty collaboration corpus')
    ids = set()
    for row in rows:
        if set(row) != {
            'id',
            'path',
            'sha256',
            'readiness',
            'expectation',
            'expectation_sha256',
            'reference_status',
            'native',
            'diagnostic_code',
        }:
            raise ValueError('Unknown collaboration fixture fields')
        if not isinstance(row['id'], str) or not re.fullmatch('[a-z][a-z0-9_]*', row['id']):
            raise ValueError('Invalid collaboration fixture ID')
        if any(not isinstance(row[k], str) or not re.fullmatch('[0-9a-f]{64}', row[k]) for k in ('sha256', 'expectation_sha256')):
            raise ValueError('Invalid collaboration fixture hash')
        if not (ROOT / row['path']).resolve().is_relative_to((ROOT / 'fixtures/collaboration').resolve()):
            raise ValueError('Collaboration source escapes owned directory')
        if row['id'] in ids or row['native'] not in ('accepted', 'rejected'):
            raise ValueError('Duplicate ID or unknown expected native outcome')
        if row['readiness'] not in ('complete', 'incomplete', 'rejected'):
            raise ValueError('Unknown readiness classification')
        ids.add(row['id'])
        if row['reference_status'] not in ('PASS', 'REFERENCE_REJECTED', 'REFERENCE_NONFINITE', 'PROJECTION_UNSUPPORTED'):
            raise ValueError('Unreviewed reference status')
        if row['native'] == 'accepted':
            if row['reference_status'] != 'PASS' or row['diagnostic_code'] is not None or row['readiness'] not in ('complete', 'incomplete'):
                raise ValueError('Accepted collaboration fixture has inconsistent metadata')
        else:
            if row['readiness'] != 'rejected' or row['diagnostic_code'] not in (
                'syntax',
                'unsupported_feature',
                'declaration_conflict',
                'initialization_cycle',
                'units_inconsistency',
            ):
                raise ValueError('Rejected collaboration fixture needs explicit diagnostic contract')
        if row['reference_status'] in ('REFERENCE_NONFINITE', 'PROJECTION_UNSUPPORTED') and (
            row['native'] != 'rejected' or row['diagnostic_code'] != 'unsupported_feature'
        ):
            raise ValueError('Unrepresented reference can only require unsupported rejection')
        fixture_source(row)
        expectation(row)


def expectation(row):
    path = (ROOT / row['expectation']).resolve()
    if not path.is_relative_to((ROOT / 'fixtures/collaboration/reference').resolve()):
        raise ValueError('Collaboration expectation escapes owned directory')
    data = path.read_bytes()
    if digest(data) != row['expectation_sha256']:
        raise ValueError('Collaboration reference expectation hash changed')
    value = json.loads(data)
    if value.get('source_sha256') != row['sha256'] or value.get('status') != row['reference_status']:
        raise ValueError('Collaboration snapshot source/status differs')
    if row['native'] == 'accepted':
        require_valid_functions_projection(value)
        ready = is_simulation_ready_projection(value['projection'])
        if row['readiness'] == 'complete' and not ready:
            raise ValueError(f"Fixture {row['id']} marked complete but has unset initial state")
        if row['readiness'] == 'incomplete' and ready:
            raise ValueError(f"Fixture {row['id']} marked incomplete but all initial state is set")
    if row['reference_status'] == 'PROJECTION_UNSUPPORTED':
        reasons = value.get('projection_reasons')
        allowed = {
            'Unrepresented SBML element: algebraicRule',
            'Rule metadata is outside projection v1',
            'Unrepresented MathML element: xor',
            'Unrepresented MathML element: implies',
            'Unrepresented MathML attributes: cn',
        }
        if not reasons or not set(reasons) <= allowed or 'projection' in value:
            raise ValueError('Unreviewed outside-projection reference')
    return value


def run_roadrunner_differential(ref_sbml: str, native_sbml: str, rr_python: str) -> dict:
    """Compare Reference SBML and Native SBML trajectories in libroadrunner over [0, 10]."""
    script = """
import json, sys
import numpy as np
import roadrunner

payload = json.loads(sys.stdin.read())
rr_ref = roadrunner.RoadRunner(payload["ref_sbml"])
rr_nat = roadrunner.RoadRunner(payload["native_sbml"])
for rr in (rr_ref, rr_nat):
    rr.setIntegrator("cvode")
    integ = rr.getIntegrator()
    integ.setValue("relative_tolerance", 1e-10)
    integ.setValue("absolute_tolerance", 1e-12)
    integ.setValue("maximum_num_steps", 100000)
    integ.setValue("variable_step_size", False)
    integ.setValue("stiff", True)

floats_ref = sorted(rr_ref.model.getFloatingSpeciesIds())
floats_nat = sorted(rr_nat.model.getFloatingSpeciesIds())
params_ref = sorted(rr_ref.model.getGlobalParameterIds())
params_nat = sorted(rr_nat.model.getGlobalParameterIds())
if floats_ref != floats_nat or params_ref != params_nat:
    print(json.dumps({"status": "FAIL", "reason": "Floating species or global parameter IDs differ"}))
    sys.exit(0)

sel = ["time"] + [f"[{s}]" for s in floats_ref] + params_ref
res_ref = np.asarray(rr_ref.simulate(0.0, 10.0, 101, selections=sel), dtype=float)
res_nat = np.asarray(rr_nat.simulate(0.0, 10.0, 101, selections=sel), dtype=float)

if res_ref.shape != res_nat.shape or not np.all(np.isfinite(res_ref)) or not np.all(np.isfinite(res_nat)):
    print(json.dumps({"status": "FAIL", "reason": "Nonfinite or mismatched trajectory shape"}))
    sys.exit(0)

max_diff = float(np.max(np.abs(res_ref - res_nat)))
print(json.dumps({
    "status": "PASS" if max_diff <= 1e-8 else "FAIL",
    "max_abs_diff": max_diff,
    "columns": len(sel),
    "points": int(res_ref.shape[0]),
}))
"""
    proc = subprocess.run(
        [rr_python, '-c', script],
        input=json.dumps({'ref_sbml': ref_sbml, 'native_sbml': native_sbml}),
        text=True,
        capture_output=True,
        check=False,
    )
    if proc.returncode != 0 or not proc.stdout.strip():
        return {'status': 'FAIL', 'reason': proc.stderr.strip() or 'RoadRunner subprocess failed'}
    return json.loads(proc.stdout)


def check_case(row, cli, python=None, rr_python=None):
    if row['reference_status'] == 'PROJECTION_UNSUPPORTED':
        from functions_check import check_unrepresented
        source = fixture_source(row)
        expected = expectation(row)
        reference = run('convert', source, python)
        record = {
            'fixture': row['id'],
            'readiness': row['readiness'],
            'source_sha256': row['sha256'],
            'status': 'FAIL',
            'reference_status': reference['status'],
            'numerical': 'NOT_APPLICABLE',
            'exact_projection': 'NOT_REPRESENTED',
        }
        if reference['status'] != 'PROJECTION_UNSUPPORTED':
            record['reason'] = 'Expected outside-projection reference changed or unavailable'
            return record
        observed = snapshot(reference)
        observed['projection_reasons'] = reference['reasons']
        if observed != expected:
            record['reason'] = 'Outside-projection audit differs from recorded reference'
            return record
        values = {cmd: cli_process(cli, cmd, source) for cmd in ('check', 'format', 'export')}
        record['commands'] = {cmd: {k: v for k, v in value.items() if k != 'artifact'} for cmd, value in values.items()}
        for value in values.values():
            errors = [d for d in value.get('report', {}).get('diagnostics', []) if d['severity'] == 'error']
            if value['status'] != 'REJECTED' or value.get('artifact') or not any(d['code'] == 'unsupported_feature' for d in errors):
                record['reason'] = 'Deferred feature must reject without artifact'
                return record
        record.update(status='PASS', outcome='expected_rejection')
        return record

    from expression_check import check_case as exact_check
    record = exact_check(row, cli, python, expectation_loader=expectation)
    record['readiness'] = row['readiness']
    if record.get('status') != 'PASS':
        return record

    if row['readiness'] == 'rejected':
        record['numerical'] = 'NOT_APPLICABLE'
    elif row['readiness'] == 'incomplete':
        record['outcome'] = 'incomplete_export'
        record['numerical'] = 'NOT_SIMULATION_READY'
    elif row['readiness'] == 'complete':
        record['outcome'] = 'complete_export'
        if rr_python:
            source = fixture_source(row)
            ref = run('convert', source, python)
            nat_export = cli_process(cli, 'export', source)
            rr_res = run_roadrunner_differential(
                ref['sbml'],
                nat_export['artifact'].decode('utf-8'),
                rr_python,
            )
            record['numerical'] = rr_res['status']
            record['numerical_detail'] = rr_res
            if rr_res['status'] != 'PASS':
                record['status'] = 'FAIL'
                record['reason'] = rr_res.get('reason', 'RoadRunner differential trajectory mismatch')
        else:
            record['numerical'] = 'NOT_RUN'
    return record


def fingerprint(paths):
    return {str(p): digest(p.read_bytes()) if p.is_file() else None for p in paths}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--cli', required=True)
    parser.add_argument('--trusted-native', action='store_true')
    parser.add_argument('--python')
    parser.add_argument('--roadrunner-python', help='Optional Python interpreter with libroadrunner for complete-model trajectory verification')
    parser.add_argument('--current-profile', action='store_true')
    args = parser.parse_args()
    if not args.trusted_native:
        parser.error('--trusted-native is required')
    cli = Path(args.cli).resolve()
    if not cli.is_file() or not os.access(cli, os.X_OK):
        parser.error('Prebuilt CLI missing or not executable')
    try:
        manifest = load_manifest()
        if args.current_profile or profile_overlay.PROFILE == 'modular-core-v0.8-slice':
            manifest = profile_overlay.collaboration(manifest)
        paths = [
            MANIFEST,
            cli,
            ROOT / 'docs/cli-report.schema.json',
            ROOT / 'profiles/functions-core-v0.5.json',
            profile_overlay.PATH,
            *Path(__file__).parent.glob('*.py'),
        ]
        paths += [(ROOT / row[k]) for row in manifest['fixtures'] for k in ('path', 'expectation')]
        before = fingerprint(paths)
        records = [check_case(row, cli, args.python, args.roadrunner_python) for row in manifest['fixtures']]
        after = fingerprint(paths)
        changed = [p for p in before if before[p] != after[p]]
        ok = bool(records) and not changed and all(r['status'] == 'PASS' for r in records)
        complete_exports = sum(r.get('outcome') == 'complete_export' for r in records)
        incomplete_exports = sum(r.get('outcome') == 'incomplete_export' for r in records)
        emit({
            'schema': 'antimony-collaboration-check-v1',
            'implemented_profile': profile_overlay.PROFILE,
            'corpus_profile': PROFILE,
            'status': 'PASS' if ok else 'FAIL',
            'comparison': 'exact structural projection + readiness classification + optional RoadRunner differential trajectories on complete models',
            'inputs': before,
            'changed_inputs': changed,
            'cases': records,
            'counts': {
                'cases': len(records),
                'supported_exports': complete_exports + incomplete_exports,
                'complete_exports': complete_exports,
                'incomplete_exports': incomplete_exports,
                'expected_rejections': sum(r.get('outcome') == 'expected_rejection' for r in records),
                'numerical_passed': sum(r.get('numerical') == 'PASS' for r in records),
                'numerical_not_simulation_ready': sum(r.get('numerical') == 'NOT_SIMULATION_READY' for r in records),
                'failed_or_blocked': sum(r['status'] != 'PASS' for r in records),
            },
        })
        return 0 if ok else 2
    except (OSError, ValueError, KeyError, TypeError) as error:
        emit({'schema': 'antimony-collaboration-check-v1', 'status': 'FAIL', 'reason': str(error)})
        return 2


if __name__ == '__main__':
    raise SystemExit(main())
