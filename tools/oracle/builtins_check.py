#!/usr/bin/env python3
"""Exact structural builtins gate for a trusted prebuilt general-purpose CLI.

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

MANIFEST = ROOT / 'fixtures/manifests/builtins-v2.json'
PROFILE = 'math-core-v0.2-slice'


def load_manifest(path=MANIFEST):
    manifest = json.loads(path.read_text())
    if set(manifest) != {'schema', 'implemented_profile', 'provenance', 'fixtures'}:
        raise ValueError('Unknown builtin corpus fields')
    if manifest.get('schema') != 'antimony-builtins-corpus-v2' or manifest.get('implemented_profile') != PROFILE:
        raise ValueError('Unexpected builtins corpus/profile')
    rows = manifest.get('fixtures')
    if not rows or not isinstance(rows, list):
        raise ValueError('Empty builtins corpus')
    ids = set()
    for row in rows:
        if set(row) != {'id', 'path', 'sha256', 'expectation', 'expectation_sha256', 'reference_status', 'native', 'diagnostic_code'}:
            raise ValueError('Unknown builtin fixture fields')
        if not isinstance(row['id'], str) or not re.fullmatch('[a-z][a-z0-9_]*', row['id']):
            raise ValueError('Invalid builtin fixture ID')
        if any(not isinstance(row[k], str) or not re.fullmatch('[0-9a-f]{64}', row[k]) for k in ('sha256','expectation_sha256')):
            raise ValueError('Invalid builtin fixture hash')
        if not (ROOT / row['path']).resolve().is_relative_to((ROOT / 'fixtures/builtins').resolve()):
            raise ValueError('Builtin source escapes owned directory')
        if row['id'] in ids or row['native'] not in ('accepted', 'rejected'):
            raise ValueError('Duplicate ID or unknown expected native outcome')
        ids.add(row['id'])
        if row['reference_status'] not in ('PASS', 'REFERENCE_REJECTED'):
            raise ValueError('Unreviewed reference status')
        if row['native'] == 'accepted' and (row['reference_status'] != 'PASS' or row['diagnostic_code'] is not None):
            raise ValueError('Accepted fixture lacks reference support')
        if row['native'] == 'rejected' and row['diagnostic_code'] not in ('syntax', 'unsupported_feature', 'declaration_conflict', 'initialization_cycle', 'units_inconsistency'):
            raise ValueError('Negative case needs explicit diagnostic contract')
        fixture_source(row)
        expectation(row)
    return manifest


def expectation(row):
    path = (ROOT / row['expectation']).resolve()
    if not path.is_relative_to((ROOT / 'fixtures/manifests/builtins-v2').resolve()):
        raise ValueError('Builtins expectation escapes owned directory')
    data = path.read_bytes()
    if digest(data) != row['expectation_sha256']:
        raise ValueError('Builtins reference expectation hash changed')
    return json.loads(data)


def check_case(row, cli, python=None):
    from expression_check import check_case as exact_check
    return exact_check(row, cli, python, expectation_loader=expectation)


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
        paths = [MANIFEST, cli, ROOT / 'docs/cli-report.schema.json', *Path(__file__).parent.glob('*.py')]
        paths += [(ROOT / row[k]) for row in manifest['fixtures'] for k in ('path', 'expectation')]
        before = fingerprint(paths)
        records = [check_case(row, cli, args.python) for row in manifest['fixtures']]
        after = fingerprint(paths)
        changed = [p for p in before if before[p] != after[p]]
        ok = bool(records) and not changed and all(r['status'] == 'PASS' for r in records)
        emit({'schema': 'antimony-builtins-check-v2', 'implemented_profile': profile_overlay.PROFILE, 'corpus_profile': PROFILE,
              'status': 'PASS' if ok else 'FAIL', 'comparison': 'exact structural projection; no algebraic or floating tolerance',
              'inputs': before, 'changed_inputs': changed, 'cases': records,
              'counts': {'cases': len(records), 'supported_exports': sum(r.get('outcome') == 'supported_export' for r in records),
                         'expected_rejections': sum(r.get('outcome') == 'expected_rejection' for r in records),
                         'failed_or_blocked': sum(r['status'] != 'PASS' for r in records), 'numerical_not_run': len(records)}})
        return 0 if ok else 2
    except (OSError, ValueError, KeyError, TypeError) as error:
        emit({'schema': 'antimony-builtins-check-v2', 'status': 'FAIL', 'reason': str(error)})
        return 2


if __name__ == '__main__':
    raise SystemExit(main())
