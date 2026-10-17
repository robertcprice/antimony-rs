#!/usr/bin/env python3
"""Exact structural user-defined functions gate for a trusted prebuilt CLI.

Fixtures are a test inventory, never a native input whitelist. No builds,
installation or numerical-trajectory claims. Reference subprocesses are not a
sandbox; only hash-pinned, import-free original project fixtures are submitted.
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
from conditional_check import require_represented_math
from rules_check import require_valid_rules_projection

MANIFEST = ROOT / 'fixtures/functions/manifest.json'
PROFILE = 'functions-core-v0.5-slice'


def load_manifest(path=MANIFEST):
    if profile_overlay.PROFILE not in (PROFILE, 'modular-core-v0.8-slice'):
        raise ValueError('Functions gate requires explicit functions-core-v0.5-slice or successor configuration')
    manifest = json.loads(path.read_text())
    if set(manifest) != {'schema', 'implemented_profile', 'provenance', 'fixtures'}:
        raise ValueError('Unknown functions corpus fields')
    if manifest.get('schema') != 'antimony-functions-corpus-v1' or manifest.get('implemented_profile') != PROFILE:
        raise ValueError('Unexpected functions corpus/profile')
    validate_rows(manifest.get('fixtures'))
    return manifest


def validate_rows(rows):
    if not rows or not isinstance(rows, list):
        raise ValueError('Empty functions corpus')
    ids = set()
    for row in rows:
        if set(row) != {'id', 'path', 'sha256', 'expectation', 'expectation_sha256', 'reference_status', 'native', 'diagnostic_code'}:
            raise ValueError('Unknown function fixture fields')
        if not isinstance(row['id'], str) or not re.fullmatch('[a-z][a-z0-9_]*', row['id']):
            raise ValueError('Invalid function fixture ID')
        if any(not isinstance(row[k], str) or not re.fullmatch('[0-9a-f]{64}', row[k]) for k in ('sha256', 'expectation_sha256')):
            raise ValueError('Invalid function fixture hash')
        if not (ROOT / row['path']).resolve().is_relative_to((ROOT / 'fixtures/functions').resolve()):
            raise ValueError('Function source escapes owned directory')
        if row['id'] in ids or row['native'] not in ('accepted', 'rejected'):
            raise ValueError('Duplicate ID or unknown expected native outcome')
        ids.add(row['id'])
        if row['reference_status'] not in ('PASS', 'REFERENCE_REJECTED', 'REFERENCE_NONFINITE', 'PROJECTION_UNSUPPORTED'):
            raise ValueError('Unreviewed reference status')
        if row['native'] == 'accepted' and (row['reference_status'] != 'PASS' or row['diagnostic_code'] is not None):
            raise ValueError('Accepted fixture lacks reference support')
        if row['native'] == 'rejected' and row['diagnostic_code'] not in ('syntax', 'unsupported_feature', 'declaration_conflict', 'initialization_cycle', 'units_inconsistency'):
            raise ValueError('Negative case needs explicit diagnostic contract')
        if row['reference_status'] in ('REFERENCE_NONFINITE', 'PROJECTION_UNSUPPORTED') and (row['native'] != 'rejected' or row['diagnostic_code'] != 'unsupported_feature'):
            raise ValueError('Unrepresented reference can only require unsupported rejection')
        fixture_source(row)
        expectation(row)


def require_valid_functions_projection(value):
    require_represented_math(value)
    require_valid_rules_projection(value)
    projection = value.get('projection')
    functions = projection.get('function_definitions')
    if not isinstance(functions, list):
        raise ValueError('Missing function_definitions list in projection')
    reserved_ids = (
        {c['id'] for c in projection.get('compartments', [])}
        | {s['id'] for s in projection.get('species', [])}
        | {p['id'] for p in projection.get('parameters', [])}
        | {r['id'] for r in projection.get('reactions', [])}
        | {u['id'] for u in projection.get('unit_definitions', [])}
        | {e['id'] for e in projection.get('events', []) if e.get('id')}
    )
    model_id = projection.get('model', {}).get('id')
    if isinstance(model_id, str) and model_id:
        reserved_ids.add(model_id)
    seen_ids = set()
    for fn in functions:
        if not isinstance(fn, dict) or set(fn) != {'id', 'name', 'sbo_term', 'annotation', 'math'}:
            raise ValueError('Invalid function_definition entry shape')
        if fn['name'] is not None or fn['sbo_term'] is not None or fn['annotation'] is not None:
            raise ValueError('FunctionDefinition metadata is outside projection v1')
        fn_id = fn['id']
        if not isinstance(fn_id, str) or not fn_id:
            raise ValueError('Empty function_definition id')
        if fn_id in seen_ids:
            raise ValueError('Duplicate function_definition id in projection')
        if fn_id in reserved_ids:
            raise ValueError('Function id collides with model or entity identifier')
        seen_ids.add(fn_id)
        math = fn['math']
        if not isinstance(math, dict) or math.get('type') != 267:
            raise ValueError('FunctionDefinition math root must be AST_LAMBDA (267)')
        children = math.get('children')
        if not isinstance(children, list) or not children:
            raise ValueError('FunctionDefinition lambda must have at least a body child')
        params = set()
        for bvar in children[:-1]:
            if not isinstance(bvar, dict) or bvar.get('type') != 260 or not isinstance(bvar.get('name'), str) or not bvar['name']:
                raise ValueError('FunctionDefinition bvar must be an AST_NAME (260) node')
            if bvar['name'] in params:
                raise ValueError('Duplicate FunctionDefinition parameter in projection')
            params.add(bvar['name'])
        body = children[-1]
        if not isinstance(body, dict) or body.get('type') == 267:
            raise ValueError('FunctionDefinition lambda body must be a non-lambda expression node')


def expectation(row):
    path = (ROOT / row['expectation']).resolve()
    if not path.is_relative_to((ROOT / 'fixtures/functions/reference').resolve()):
        raise ValueError('Functions expectation escapes owned directory')
    data = path.read_bytes()
    if digest(data) != row['expectation_sha256']:
        raise ValueError('Functions reference expectation hash changed')
    value = json.loads(data)
    if value.get('source_sha256') != row['sha256'] or value.get('status') != row['reference_status']:
        raise ValueError('Function snapshot source/status differs')
    if row['native'] == 'accepted':
        require_valid_functions_projection(value)
    if row['reference_status'] == 'PROJECTION_UNSUPPORTED':
        reasons = value.get('projection_reasons')
        allowed = {
            'Unrepresented MathML element: lambda',
            'Unrepresented MathML element: bvar',
            'FunctionDefinition without lambda math is outside projection v1',
            'FunctionDefinition metadata is outside projection v1',
            'Unrepresented SBML element: algebraicRule',
            'Rule metadata is outside projection v1',
            'Unrepresented MathML element: xor',
            'Unrepresented MathML element: implies',
            'Unrepresented MathML attributes: cn',
        }
        if not reasons or not set(reasons) <= allowed or 'projection' in value:
            raise ValueError('Unreviewed outside-projection reference')
    if row['reference_status'] == 'REFERENCE_NONFINITE':
        if value.get('exact_projection') != 'NOT_REPRESENTED' or not value.get('nonfinite_math') or digest(value.get('sbml', '').encode()) != value.get('sbml_sha256'):
            raise ValueError('Invalid nonfinite reference audit')
    return value


def check_unrepresented(row, cli, python=None):
    source = fixture_source(row)
    expected = expectation(row)
    reference = run('convert', source, python)
    record = {
        'fixture': row['id'],
        'source_sha256': row['sha256'],
        'status': 'FAIL',
        'reference_status': reference['status'],
        'numerical': 'NOT_RUN',
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
    record['oracle'] = reference['oracle']
    values = {cmd: cli_process(cli, cmd, source) for cmd in ('check', 'format', 'export')}
    record['commands'] = {cmd: {k: v for k, v in value.items() if k != 'artifact'} for cmd, value in values.items()}
    for value in values.values():
        errors = [d for d in value.get('report', {}).get('diagnostics', []) if d['severity'] == 'error']
        if value['status'] != 'REJECTED' or value.get('artifact') or not any(d['code'] == 'unsupported_feature' for d in errors):
            record['reason'] = 'Deferred feature must reject without artifact'
            return record
    record.update(status='PASS', outcome='expected_rejection')
    return record


def check_case(row, cli, python=None):
    if row['reference_status'] == 'REFERENCE_NONFINITE':
        from rational_check import check_nonfinite
        return check_nonfinite(row, cli, python, expectation_loader=expectation)
    if row['reference_status'] == 'PROJECTION_UNSUPPORTED':
        return check_unrepresented(row, cli, python)
    from expression_check import check_case as exact_check
    return exact_check(row, cli, python, expectation_loader=expectation)


def fingerprint(paths):
    return {str(p): digest(p.read_bytes()) if p.is_file() else None for p in paths}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--cli', required=True)
    parser.add_argument('--trusted-native', action='store_true')
    parser.add_argument('--python')
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
            manifest = profile_overlay.functions(manifest)
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
        records = [check_case(row, cli, args.python) for row in manifest['fixtures']]
        after = fingerprint(paths)
        changed = [p for p in before if before[p] != after[p]]
        ok = bool(records) and not changed and all(r['status'] == 'PASS' for r in records)
        emit({
            'schema': 'antimony-functions-check-v1',
            'implemented_profile': profile_overlay.PROFILE,
            'corpus_profile': PROFILE,
            'status': 'PASS' if ok else 'FAIL',
            'comparison': 'exact structural projection; no algebraic or floating tolerance',
            'manifests': [str(MANIFEST.relative_to(ROOT))],
            'inputs': before,
            'changed_inputs': changed,
            'cases': records,
            'counts': {
                'cases': len(records),
                'supported_exports': sum(r.get('outcome') == 'supported_export' for r in records),
                'expected_rejections': sum(r.get('outcome') == 'expected_rejection' for r in records),
                'failed_or_blocked': sum(r['status'] != 'PASS' for r in records),
                'numerical_not_run': len(records),
            },
        })
        return 0 if ok else 2
    except (OSError, ValueError, KeyError, TypeError) as error:
        emit({'schema': 'antimony-functions-check-v1', 'status': 'FAIL', 'reason': str(error)})
        return 2


if __name__ == '__main__':
    raise SystemExit(main())
