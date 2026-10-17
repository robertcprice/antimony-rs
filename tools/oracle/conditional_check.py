#!/usr/bin/env python3
"""Exact structural conditional gate for a trusted prebuilt general-purpose CLI.

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
from native_check import differences
from cli_check import cli_process, warnings

MANIFEST = ROOT / 'fixtures/conditionals/manifest.json'
PROFILE = 'conditional-core-v0.3-slice'
ALIASES = ROOT / 'fixtures/conditionals/aliases-v1.json'


def load_manifest(path=MANIFEST):
    if profile_overlay.PROFILE not in (PROFILE, 'rules-core-v0.4-slice', 'functions-core-v0.5-slice', 'modular-core-v0.8-slice'):
        raise ValueError('Conditional gate requires explicit conditional-core-v0.3-slice or successor configuration')
    manifest = json.loads(path.read_text())
    if set(manifest) != {'schema', 'implemented_profile', 'provenance', 'fixtures'}:
        raise ValueError('Unknown conditional corpus fields')
    if manifest.get('schema') != 'antimony-conditional-corpus-v1' or manifest.get('implemented_profile') != PROFILE:
        raise ValueError('Unexpected conditionals corpus/profile')
    validate_rows(manifest.get('fixtures'))
    return manifest


def validate_rows(rows):
    if not rows or not isinstance(rows, list):
        raise ValueError('Empty conditionals corpus')
    ids = set()
    for row in rows:
        if set(row) != {'id', 'path', 'sha256', 'expectation', 'expectation_sha256', 'reference_status', 'native', 'diagnostic_code'}:
            raise ValueError('Unknown conditional fixture fields')
        if not isinstance(row['id'], str) or not re.fullmatch('[a-z][a-z0-9_]*', row['id']):
            raise ValueError('Invalid conditional fixture ID')
        if any(not isinstance(row[k], str) or not re.fullmatch('[0-9a-f]{64}', row[k]) for k in ('sha256','expectation_sha256')):
            raise ValueError('Invalid conditional fixture hash')
        if not (ROOT / row['path']).resolve().is_relative_to((ROOT / 'fixtures/conditionals').resolve()):
            raise ValueError('Conditional source escapes owned directory')
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


def load_alias_manifest(path=ALIASES):
    if profile_overlay.PROFILE not in (PROFILE, 'rules-core-v0.4-slice', 'functions-core-v0.5-slice', 'modular-core-v0.8-slice'):
        raise ValueError('Conditional alias gate requires current conditional or successor profile')
    value=json.loads(path.read_text())
    if set(value)!={'schema','implemented_profile','provenance','parent_manifest','fixtures'} or value['schema']!='antimony-conditional-alias-corpus-v1' or value['implemented_profile']!=PROFILE:
        raise ValueError('Unknown conditional alias corpus/profile')
    if value['parent_manifest']!={'path':str(MANIFEST.relative_to(ROOT)),'sha256':digest(MANIFEST.read_bytes())}:
        raise ValueError('Conditional alias parent binding changed')
    validate_rows(value['fixtures'])
    return value


def load_combined_manifests():
    base=load_manifest();aliases=load_alias_manifest()
    rows=base['fixtures']+aliases['fixtures']
    validate_rows(rows)
    return {'fixtures':rows,'manifests':[str(MANIFEST.relative_to(ROOT)),str(ALIASES.relative_to(ROOT))]}


def expectation(row):
    path = (ROOT / row['expectation']).resolve()
    if not path.is_relative_to((ROOT / 'fixtures/conditionals/reference').resolve()):
        raise ValueError('Conditionals expectation escapes owned directory')
    data = path.read_bytes()
    if digest(data) != row['expectation_sha256']:
        raise ValueError('Conditionals reference expectation hash changed')
    value = json.loads(data)
    if value.get('source_sha256') != row['sha256'] or value.get('status') != row['reference_status']:
        raise ValueError('Conditional snapshot source/status differs')
    if row['native'] == 'accepted':
        require_represented_math(value)
    if row['reference_status'] == 'PROJECTION_UNSUPPORTED':
        reasons = value.get('projection_reasons')
        allowed = {'Unrepresented MathML element: xor', 'Unrepresented MathML element: implies', 'Unrepresented MathML attributes: cn'}
        if not reasons or not set(reasons) <= allowed or 'projection' in value:
            raise ValueError('Unreviewed outside-projection reference')
    if row['reference_status'] == 'REFERENCE_NONFINITE':
        if value.get('exact_projection') != 'NOT_REPRESENTED' or not value.get('nonfinite_math') or digest(value.get('sbml', '').encode()) != value.get('sbml_sha256'):
            raise ValueError('Invalid nonfinite reference audit')
    return value


def check_case(row, cli, python=None):
    if row['reference_status'] == 'REFERENCE_NONFINITE':
        from rational_check import check_nonfinite
        return check_nonfinite(row, cli, python, expectation_loader=expectation)
    if row['reference_status'] == 'PROJECTION_UNSUPPORTED':
        return check_unrepresented(row, cli, python)
    from expression_check import check_case as exact_check
    return exact_check(row, cli, python, expectation_loader=expectation)



def require_represented_math(value):
    projection = value.get('projection')
    if not isinstance(projection, dict):
        raise ValueError('Supported conditional case requires exact projection')
    def check(node):
        if not isinstance(node, dict) or type(node.get('type')) is not int:
            raise ValueError('Null or unrepresented conditional math')
        for child in node.get('children', []):
            check(child)
    for entry in projection.get('initial_assignments', []):
        check(entry.get('math'))
    for entry in projection.get('reactions', []):
        check(entry.get('math'))
    for entry in projection.get('rules', []):
        check(entry.get('math'))
    for entry in projection.get('function_definitions', []):
        check(entry.get('math'))
    for entry in projection.get('events', []):
        check(entry.get('trigger', {}).get('math'))
        if entry.get('delay') is not None:
            check(entry['delay'])
        if entry.get('priority') is not None:
            check(entry['priority'])
        for ea in entry.get('assignments', []):
            check(ea.get('math'))


def check_unrepresented(row, cli, python=None):
    source = fixture_source(row);expected = expectation(row)
    reference = run('convert', source, python)
    record = {'fixture': row['id'], 'source_sha256': row['sha256'], 'status': 'FAIL',
              'reference_status': reference['status'], 'numerical': 'NOT_RUN', 'exact_projection': 'NOT_REPRESENTED'}
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
    record['commands'] = {cmd: {k:v for k,v in value.items() if k != 'artifact'} for cmd,value in values.items()}
    for value in values.values():
        errors = [d for d in value.get('report',{}).get('diagnostics',[]) if d['severity']=='error']
        if value['status'] != 'REJECTED' or value.get('artifact') or not any(d['code']=='unsupported_feature' for d in errors):
            record['reason'] = 'Deferred feature must reject without artifact'
            return record
    record.update(status='PASS', outcome='expected_rejection')
    return record


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
        manifest = load_combined_manifests()
        if args.current_profile or profile_overlay.PROFILE == 'modular-core-v0.8-slice':
            manifest = profile_overlay.conditionals(manifest)
        paths = [MANIFEST, ALIASES, cli, ROOT / 'docs/cli-report.schema.json', ROOT / 'profiles/conditional-core-v0.3.json', profile_overlay.PATH, *Path(__file__).parent.glob('*.py')]
        paths += [(ROOT / row[k]) for row in manifest['fixtures'] for k in ('path', 'expectation')]
        before = fingerprint(paths)
        records = [check_case(row, cli, args.python) for row in manifest['fixtures']]
        after = fingerprint(paths)
        changed = [p for p in before if before[p] != after[p]]
        ok = bool(records) and not changed and all(r['status'] == 'PASS' for r in records)
        emit({'schema': 'antimony-conditional-check-v1', 'implemented_profile': profile_overlay.PROFILE, 'corpus_profile': PROFILE,
              'status': 'PASS' if ok else 'FAIL', 'comparison': 'exact structural projection; no algebraic or floating tolerance',
              'manifests': manifest['manifests'], 'inputs': before, 'changed_inputs': changed, 'cases': records,
              'counts': {'cases': len(records), 'supported_exports': sum(r.get('outcome') == 'supported_export' for r in records),
                         'expected_rejections': sum(r.get('outcome') == 'expected_rejection' for r in records),
                         'failed_or_blocked': sum(r['status'] != 'PASS' for r in records), 'numerical_not_run': len(records)}})
        return 0 if ok else 2
    except (OSError, ValueError, KeyError, TypeError) as error:
        emit({'schema': 'antimony-conditional-check-v1', 'status': 'FAIL', 'reason': str(error)})
        return 2


if __name__ == '__main__':
    raise SystemExit(main())
