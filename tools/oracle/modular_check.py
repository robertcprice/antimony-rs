#!/usr/bin/env python3
"""Exact structural and RoadRunner differential gate for `modular-core-v0.8-slice` (Phases 6-8).

Verifies 40 models across Phases 6 (`substanceOnly` species, dynamic compartments,
reaction kinetic-law overrides), 7 (discrete `at` events with triggers, delays,
priorities, and options), and 8 (multi-module submodels, ports, `M.x` member
access, `is` aliasing, and flattening), including 26 simulation-ready complete
models, 2 incomplete drafts, and 12 negative near-neighbor/deferred models.
"""
from __future__ import annotations
import argparse
import json
import os
import re
import profile_overlay
from pathlib import Path

from oracle import ROOT, digest, emit, run
from corpus import fixture_source, snapshot
from cli_check import cli_process
from functions_check import require_valid_functions_projection
from collaboration_check import is_simulation_ready_projection, run_roadrunner_differential

MANIFEST = ROOT / 'fixtures/modular/manifest.json'
PROFILE = 'modular-core-v0.8-slice'


def require_valid_modular_projection(value: dict) -> None:
    require_valid_functions_projection(value)
    projection = value.get('projection')
    events = projection.get('events')
    if not isinstance(events, list):
        raise ValueError('Missing events list in projection')
    for spec in projection.get('species', []):
        if spec.get('has_only_substance_units') is True and spec.get('initial_concentration') is not None:
            raise ValueError('substanceOnly species must not set initial_concentration')
    targets = (
        {c['id']: c['constant'] for c in projection.get('compartments', [])}
        | {s['id']: s['constant'] for s in projection.get('species', [])}
        | {p['id']: p['constant'] for p in projection.get('parameters', [])}
    )
    reserved_ids = (
        set(targets)
        | {r['id'] for r in projection.get('reactions', [])}
        | {u['id'] for u in projection.get('unit_definitions', [])}
        | {f['id'] for f in projection.get('function_definitions', [])}
    )
    model_id = projection.get('model', {}).get('id')
    if isinstance(model_id, str) and model_id:
        reserved_ids.add(model_id)
    seen_event_ids = set()
    for ev in events:
        if not isinstance(ev, dict) or set(ev) != {
            'id',
            'name',
            'sbo_term',
            'annotation',
            'use_values_from_trigger_time',
            'trigger',
            'delay',
            'priority',
            'assignments',
        }:
            raise ValueError('Invalid event entry shape in projection')
        ev_id = ev['id']
        if not isinstance(ev_id, str) or not ev_id:
            raise ValueError('Empty event id in projection')
        if ev_id in seen_event_ids:
            raise ValueError('Duplicate event id in projection')
        if ev_id in reserved_ids:
            raise ValueError('Event id collides with model or entity identifier')
        seen_event_ids.add(ev_id)
        if type(ev['use_values_from_trigger_time']) is not bool:
            raise ValueError('Event use_values_from_trigger_time must be a bool')
        trig = ev['trigger']
        if (
            not isinstance(trig, dict)
            or set(trig) != {'initial_value', 'persistent', 'math'}
            or type(trig['initial_value']) is not bool
            or type(trig['persistent']) is not bool
            or not isinstance(trig['math'], dict)
        ):
            raise ValueError('Invalid event trigger in projection')
        assigns = ev['assignments']
        if not isinstance(assigns, list) or not assigns:
            raise ValueError('Event must have at least one eventAssignment')
        seen_vars = set()
        for ea in assigns:
            if not isinstance(ea, dict) or set(ea) != {'variable', 'math'}:
                raise ValueError('Invalid eventAssignment entry shape')
            var = ea['variable']
            if not isinstance(var, str) or not var:
                raise ValueError('Empty eventAssignment variable')
            if var in seen_vars:
                raise ValueError('Duplicate eventAssignment variable in same event')
            seen_vars.add(var)
            if var not in targets or targets[var] is not False:
                raise ValueError('Event assignment target must be a non-constant compartment, species, or parameter')


def load_manifest(path=MANIFEST):
    if profile_overlay.PROFILE != PROFILE:
        raise ValueError('Modular gate requires explicit modular-core-v0.8-slice configuration')
    manifest = json.loads(path.read_text())
    if set(manifest) != {'schema', 'implemented_profile', 'provenance', 'fixtures'}:
        raise ValueError('Unknown modular corpus fields')
    if manifest.get('schema') != 'antimony-modular-corpus-v1' or manifest.get('implemented_profile') != PROFILE:
        raise ValueError('Unexpected modular corpus/profile')
    validate_rows(manifest.get('fixtures'))
    return manifest


def validate_rows(rows):
    if not rows or not isinstance(rows, list):
        raise ValueError('Empty modular corpus')
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
            raise ValueError('Unknown modular fixture fields')
        if not isinstance(row['id'], str) or not re.fullmatch('[a-z][a-z0-9_]*', row['id']):
            raise ValueError('Invalid modular fixture ID')
        if any(not isinstance(row[k], str) or not re.fullmatch('[0-9a-f]{64}', row[k]) for k in ('sha256', 'expectation_sha256')):
            raise ValueError('Invalid modular fixture hash')
        if not (ROOT / row['path']).resolve().is_relative_to((ROOT / 'fixtures/modular').resolve()):
            raise ValueError('Modular source escapes owned directory')
        if row['id'] in ids or row['native'] not in ('accepted', 'rejected'):
            raise ValueError('Duplicate ID or unknown expected native outcome')
        if row['readiness'] not in ('complete', 'incomplete', 'rejected'):
            raise ValueError('Unknown readiness classification')
        ids.add(row['id'])
        if row['reference_status'] not in ('PASS', 'REFERENCE_REJECTED', 'REFERENCE_NONFINITE', 'PROJECTION_UNSUPPORTED'):
            raise ValueError('Unreviewed reference status')
        if row['native'] == 'accepted':
            if row['reference_status'] != 'PASS' or row['diagnostic_code'] is not None or row['readiness'] not in ('complete', 'incomplete'):
                raise ValueError('Accepted modular fixture has inconsistent metadata')
        else:
            if row['readiness'] != 'rejected' or row['diagnostic_code'] not in (
                'syntax',
                'unsupported_feature',
                'declaration_conflict',
                'unresolved_symbol',
                'initialization_cycle',
                'units_inconsistency',
            ):
                raise ValueError('Rejected modular fixture needs explicit diagnostic contract')
        fixture_source(row)
        expectation(row)


def expectation(row):
    path = (ROOT / row['expectation']).resolve()
    if not path.is_relative_to((ROOT / 'fixtures/modular/reference').resolve()):
        raise ValueError('Modular expectation escapes owned directory')
    data = path.read_bytes()
    if digest(data) != row['expectation_sha256']:
        raise ValueError('Modular reference expectation hash changed')
    value = json.loads(data)
    if value.get('source_sha256') != row['sha256'] or value.get('status') != row['reference_status']:
        raise ValueError('Modular snapshot source/status differs')
    if row['native'] == 'accepted':
        require_valid_modular_projection(value)
        ready = is_simulation_ready_projection(value['projection'])
        if row['readiness'] == 'complete' and not ready:
            raise ValueError(f"Fixture {row['id']} marked complete but has unset initial state")
        if row['readiness'] == 'incomplete' and ready:
            raise ValueError(f"Fixture {row['id']} marked incomplete but all initial state is set")
    return value


def check_case(row, cli, python=None, rr_python=None):
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
    args = parser.parse_args()
    if not args.trusted_native:
        parser.error('--trusted-native is required')
    cli = Path(args.cli).resolve()
    if not cli.is_file() or not os.access(cli, os.X_OK):
        parser.error('Prebuilt CLI missing or not executable')
    try:
        manifest = load_manifest()
        paths = [
            MANIFEST,
            cli,
            ROOT / 'docs/cli-report.schema.json',
            ROOT / 'profiles/modular-core-v0.8.json',
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
            'schema': 'antimony-modular-check-v1',
            'implemented_profile': profile_overlay.PROFILE,
            'corpus_profile': PROFILE,
            'status': 'PASS' if ok else 'FAIL',
            'comparison': 'exact structural projection + readiness classification + optional RoadRunner differential trajectories on complete models',
            'manifests': [str(MANIFEST.relative_to(ROOT))],
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
        emit({'schema': 'antimony-modular-check-v1', 'status': 'FAIL', 'reason': str(error)})
        return 2


if __name__ == '__main__':
    raise SystemExit(main())
