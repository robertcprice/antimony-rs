#!/usr/bin/env python3
"""Supplemental rational token and exact structure gate for a trusted prebuilt general-purpose CLI.

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

MANIFEST = ROOT / 'fixtures/rational-calls/corrective-v1.json'
PROFILE = 'math-core-v0.2.1-slice'


def load_manifest(path=MANIFEST):
    if profile_overlay.PROFILE not in (PROFILE, 'conditional-core-v0.3-slice', 'rules-core-v0.4-slice', 'functions-core-v0.5-slice', 'modular-core-v0.8-slice'):
        raise ValueError('Rational corrective gate requires math-core-v0.2.1-slice or its conditional/rules/functions/modular successor')
    manifest = json.loads(path.read_text())
    if set(manifest) != {'schema', 'implemented_profile', 'provenance', 'reference_investigation', 'fixtures'}:
        raise ValueError('Unknown rational corpus fields')
    if manifest.get('schema') != 'antimony-rational-corrective-corpus-v1' or manifest.get('implemented_profile') != PROFILE:
        raise ValueError('Unexpected rationals corpus/profile')
    ancestor = manifest['reference_investigation']
    if set(ancestor) != {'path', 'sha256'} or ancestor['path'] != 'fixtures/rational-calls/manifest.json' or digest((ROOT / ancestor['path']).read_bytes()) != ancestor['sha256']:
        raise ValueError('Reference investigation hash changed')
    rows = manifest.get('fixtures')
    if not rows or not isinstance(rows, list):
        raise ValueError('Empty rationals corpus')
    ids = set()
    for row in rows:
        if set(row) != {'id', 'path', 'sha256', 'expectation', 'expectation_sha256', 'reference_status', 'native', 'diagnostic_code'}:
            raise ValueError('Unknown rational fixture fields')
        if not isinstance(row['id'], str) or not re.fullmatch('[a-z][a-z0-9_]*', row['id']):
            raise ValueError('Invalid rational fixture ID')
        if any(not isinstance(row[k], str) or not re.fullmatch('[0-9a-f]{64}', row[k]) for k in ('sha256','expectation_sha256')):
            raise ValueError('Invalid rational fixture hash')
        if not (ROOT / row['path']).resolve().is_relative_to((ROOT / 'fixtures/rational-calls').resolve()):
            raise ValueError('Rational source escapes owned directory')
        if row['id'] in ids or row['native'] not in ('accepted', 'rejected'):
            raise ValueError('Duplicate ID or unknown expected native outcome')
        ids.add(row['id'])
        if row['reference_status'] not in ('PASS', 'REFERENCE_REJECTED', 'REFERENCE_NONFINITE'):
            raise ValueError('Unreviewed reference status')
        if row['native'] == 'accepted' and (row['reference_status'] != 'PASS' or row['diagnostic_code'] is not None):
            raise ValueError('Accepted fixture lacks reference support')
        if row['native'] == 'rejected' and row['diagnostic_code'] not in ('syntax', 'unsupported_feature', 'declaration_conflict', 'initialization_cycle', 'units_inconsistency'):
            raise ValueError('Negative case needs explicit diagnostic contract')
        if row['reference_status'] == 'REFERENCE_NONFINITE' and (row['native'] != 'rejected' or row['diagnostic_code'] != 'unsupported_feature'):
            raise ValueError('Nonfinite reference requires explicit unsupported rejection')
        fixture_source(row)
        expectation(row)
    return manifest


def expectation(row):
    path = (ROOT / row['expectation']).resolve()
    if not path.is_relative_to((ROOT / 'fixtures/rational-calls/reference').resolve()):
        raise ValueError('Rationals expectation escapes owned directory')
    data = path.read_bytes()
    if digest(data) != row['expectation_sha256']:
        raise ValueError('Rationals reference expectation hash changed')
    value = json.loads(data)
    if value.get('source_sha256') != row['sha256'] or value.get('status') != row['reference_status']:
        raise ValueError('Reference snapshot source/status differs')
    if row['reference_status'] == 'REFERENCE_NONFINITE':
        import rational_reference
        if value.get('schema') != rational_reference.SCHEMA or value.get('exact_projection') != 'NOT_REPRESENTED' or not value.get('nonfinite_math'):
            raise ValueError('Invalid nonfinite reference audit')
        if digest(value.get('sbml', '').encode()) != value.get('sbml_sha256'):
            raise ValueError('Nonfinite reference XML hash differs')
    return value


def check_case(row, cli, python=None):
    if row['reference_status'] == 'REFERENCE_NONFINITE':
        return check_nonfinite(row, cli, python)
    from expression_check import check_case as exact_check
    return exact_check(row, cli, python, expectation_loader=expectation)



def check_nonfinite(row, cli, python=None, expectation_loader=None):
    import rational_reference
    source = fixture_source(row)
    expected = (expectation_loader or expectation)(row)
    reference = rational_reference.run(source, python)
    record = {'fixture': row['id'], 'source_sha256': row['sha256'], 'status': 'FAIL',
              'reference_status': reference['status'], 'numerical': 'NOT_RUN', 'exact_projection': 'NOT_REPRESENTED'}
    if reference['status'] != 'REFERENCE_NONFINITE':
        record['reason'] = 'Nonfinite reference audit unavailable or changed'
        return record
    if rational_reference.snapshot(reference) != expected:
        record['reason'] = 'Nonfinite reference differs from preserved exact audit'
        return record
    record['oracle'] = reference['oracle']
    values = {cmd: cli_process(cli, cmd, source) for cmd in ('check', 'format', 'export')}
    record['commands'] = {cmd: {k: v for k, v in value.items() if k != 'artifact'} for cmd, value in values.items()}
    for value in values.values():
        errors = [d for d in value.get('report', {}).get('diagnostics', []) if d['severity'] == 'error']
        if value['status'] != 'REJECTED' or value.get('artifact') or not any(d['code'] == 'unsupported_feature' for d in errors):
            record['reason'] = 'Nonfinite rational must reject without artifact'
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
    args = parser.parse_args()
    if not args.trusted_native:
        parser.error('--trusted-native is required')
    cli = Path(args.cli).resolve()
    if not cli.is_file() or not os.access(cli, os.X_OK):
        parser.error('Prebuilt CLI missing or not executable')
    try:
        manifest = load_manifest()
        paths = [MANIFEST, cli, ROOT / 'docs/cli-report.schema.json', ROOT / 'profiles/math-core-v0.2.1.json', ROOT / 'fixtures/rational-calls/manifest.json', *Path(__file__).parent.glob('*.py')]
        paths += [(ROOT / row[k]) for row in manifest['fixtures'] for k in ('path', 'expectation')]
        before = fingerprint(paths)
        records = [check_case(row, cli, args.python) for row in manifest['fixtures']]
        after = fingerprint(paths)
        changed = [p for p in before if before[p] != after[p]]
        ok = bool(records) and not changed and all(r['status'] == 'PASS' for r in records)
        emit({'schema': 'antimony-rational-corrective-check-v1', 'implemented_profile': profile_overlay.PROFILE, 'corpus_profile': PROFILE,
              'status': 'PASS' if ok else 'FAIL', 'comparison': 'exact structural projection; no algebraic or floating tolerance',
              'inputs': before, 'changed_inputs': changed, 'cases': records,
              'counts': {'cases': len(records), 'supported_exports': sum(r.get('outcome') == 'supported_export' for r in records),
                         'expected_rejections': sum(r.get('outcome') == 'expected_rejection' for r in records),
                         'failed_or_blocked': sum(r['status'] != 'PASS' for r in records), 'numerical_not_run': len(records)}})
        return 0 if ok else 2
    except (OSError, ValueError, KeyError, TypeError) as error:
        emit({'schema': 'antimony-rational-corrective-check-v1', 'status': 'FAIL', 'reason': str(error)})
        return 2


if __name__ == '__main__':
    raise SystemExit(main())
