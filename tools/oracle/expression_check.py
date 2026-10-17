#!/usr/bin/env python3
"""Exact structural arithmetic gate for a trusted prebuilt general-purpose CLI.

Fixtures are a test inventory, never a native input whitelist. No builds,
installation or numerical-trajectory claims. Reference subprocesses are not a
sandbox; only hash-pinned, import-free original project fixtures are submitted.
"""
from __future__ import annotations
import argparse
import json
import os
import profile_overlay
from pathlib import Path

from oracle import ROOT, digest, emit, run
from corpus import fixture_source, snapshot
from native_check import differences
from cli_check import cli_process, warnings

MANIFEST = ROOT / 'fixtures/manifests/arithmetic-v1.json'
PROFILE = 'arithmetic-v0.1-slice'


def load_manifest(path=MANIFEST):
    manifest = json.loads(path.read_text())
    if manifest.get('schema') != 'antimony-arithmetic-corpus-v1' or manifest.get('implemented_profile') != PROFILE:
        raise ValueError('Unexpected arithmetic corpus/profile')
    rows = manifest.get('fixtures')
    if not rows or not isinstance(rows, list):
        raise ValueError('Empty arithmetic corpus')
    ids = set()
    for row in rows:
        if row['id'] in ids or row['native'] not in ('accepted', 'rejected'):
            raise ValueError('Duplicate ID or unknown expected native outcome')
        ids.add(row['id'])
        if row['reference_status'] not in ('PASS', 'REFERENCE_REJECTED'):
            raise ValueError('Unreviewed reference status')
        if row['native'] == 'accepted' and (row['reference_status'] != 'PASS' or row['diagnostic_code'] is not None):
            raise ValueError('Accepted fixture lacks reference support')
        if row['native'] == 'rejected' and row['diagnostic_code'] not in ('syntax', 'unsupported_feature', 'declaration_conflict'):
            raise ValueError('Negative case needs explicit diagnostic contract')
        fixture_source(row)
        expectation(row)
    return manifest


def expectation(row):
    path = (ROOT / row['expectation']).resolve()
    if not path.is_relative_to((ROOT / 'fixtures/manifests/arithmetic-v1').resolve()):
        raise ValueError('Arithmetic expectation escapes owned directory')
    data = path.read_bytes()
    if digest(data) != row['expectation_sha256']:
        raise ValueError('Arithmetic reference expectation hash changed')
    return json.loads(data)


def check_case(row, cli, python=None, expectation_loader=None):
    expected = expectation_loader or expectation
    record = {'fixture': row['id'], 'source_sha256': row['sha256'],
              'status': 'FAIL', 'numerical': 'NOT_RUN', 'commands': {}}
    source = fixture_source(row)
    ref = run('convert', source, python)
    record['reference_status'] = ref['status']
    if ref['status'] not in ('PASS', 'REFERENCE_REJECTED'):
        record.update(status=ref['status'], reason='Reference unavailable or outside projection vocabulary')
        return record
    if ref['status'] != row['reference_status'] or snapshot(ref) != expected(row):
        record['reason'] = 'Live reference differs from reviewed exact expectation'
        return record
    record['oracle'] = ref['oracle']
    results = {cmd: cli_process(cli, cmd, source) for cmd in ('check', 'format', 'export')}
    record['commands'] = {cmd: {k: v for k, v in result.items() if k != 'artifact'} for cmd, result in results.items()}
    if row['native'] == 'rejected':
        for value in results.values():
            errors = [d for d in value.get('report', {}).get('diagnostics', []) if d['severity'] == 'error']
            if value['status'] != 'REJECTED' or not any(d['code'] == row['diagnostic_code'] for d in errors):
                record['reason'] = 'Negative case did not meet explicit rejection/no-artifact contract'
                return record
        record.update(status='PASS', outcome='expected_rejection')
        return record
    if any(value['status'] != 'SUCCESS' for value in results.values()):
        record['reason'] = 'Expected supported expression rejected or CLI protocol failed'
        return record
    if not all(warnings(x['report']) == warnings(results['check']['report']) for x in results.values()):
        record['reason'] = 'Command warning sets differ'
        return record
    formatted = results['format']['artifact'].decode()
    if 'import' in formatted.lower():
        record['reason'] = 'Forbidden import in generated source'
        return record
    twice = cli_process(cli, 'format', formatted)
    formatted_export = cli_process(cli, 'export', formatted)
    record['format_twice_status'] = twice['status']
    record['formatted_export_status'] = formatted_export['status']
    if twice['status'] != 'SUCCESS' or twice['artifact'] != results['format']['artifact']:
        record['reason'] = 'Formatting is not byte-idempotent'
        return record
    if formatted_export['status'] != 'SUCCESS':
        record['reason'] = 'Formatted source no longer exports'
        return record
    checks = {
        'native_export': run('inspect', results['export']['artifact'].decode(), python),
        'formatted_export': run('inspect', formatted_export['artifact'].decode(), python),
        'formatted_reference': run('convert', formatted, python),
    }
    record['comparisons'] = {}
    for name, inspected in checks.items():
        delta = differences(ref['projection'], inspected['projection']) if inspected['status'] == 'PASS' else []
        record['comparisons'][name] = {'status': inspected['status'], 'differences': delta, 'messages': inspected.get('messages', [])}
        if inspected['status'] != 'PASS' or delta:
            record['reason'] = 'Exact reference-normalized structure differs or independent validation failed'
            return record
    record.update(status='PASS', outcome='supported_export',
                  formatted_sha256=digest(formatted.encode()),
                  sbml_sha256=digest(results['export']['artifact']))
    return record


def fingerprint(paths):
    return {str(p): digest(p.read_bytes()) if p.is_file() else None for p in paths}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--cli', required=True)
    parser.add_argument('--trusted-native', action='store_true')
    parser.add_argument('--python')
    parser.add_argument('--current-profile', action='store_true', help='Apply explicit arithmetic native-outcome overlay')
    args = parser.parse_args()
    if not args.trusted_native:
        parser.error('--trusted-native is required')
    cli = Path(args.cli).resolve()
    if not cli.is_file() or not os.access(cli, os.X_OK):
        parser.error('Prebuilt CLI missing or not executable')
    try:
        manifest = load_manifest()
        if args.current_profile:
            manifest = profile_overlay.arithmetic(manifest)
        paths = [MANIFEST, cli, ROOT / 'docs/cli-report.schema.json', *Path(__file__).parent.glob('*.py')]
        if args.current_profile:
            paths.append(profile_overlay.PATH)
        paths += [(ROOT / row[k]) for row in manifest['fixtures'] for k in ('path', 'expectation')]
        before = fingerprint(paths)
        records = [check_case(row, cli, args.python) for row in manifest['fixtures']]
        after = fingerprint(paths)
        changed = [p for p in before if before[p] != after[p]]
        ok = bool(records) and not changed and all(r['status'] == 'PASS' for r in records)
        emit({'schema': 'antimony-arithmetic-check-v1', 'implemented_profile': profile_overlay.PROFILE, 'corpus_profile': PROFILE, 'native_outcome_overlay': str(profile_overlay.PATH.relative_to(ROOT)) if args.current_profile else None,
              'status': 'PASS' if ok else 'FAIL', 'comparison': 'exact structural projection; no algebraic or floating tolerance',
              'inputs': before, 'changed_inputs': changed, 'cases': records,
              'counts': {'cases': len(records), 'supported_exports': sum(r.get('outcome') == 'supported_export' for r in records),
                         'expected_rejections': sum(r.get('outcome') == 'expected_rejection' for r in records),
                         'failed_or_blocked': sum(r['status'] != 'PASS' for r in records), 'numerical_not_run': len(records)}})
        return 0 if ok else 2
    except (OSError, ValueError, KeyError, TypeError) as error:
        emit({'schema': 'antimony-arithmetic-check-v1', 'status': 'FAIL', 'reason': str(error)})
        return 2


if __name__ == '__main__':
    raise SystemExit(main())
