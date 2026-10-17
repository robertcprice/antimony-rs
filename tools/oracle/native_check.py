#!/usr/bin/env python3
"""Explicit trusted native executable -> independent SBML/reference comparison.

Build separately; this program never runs Cargo, downloads, or guesses support.
"""
from __future__ import annotations
import argparse
import profile_overlay
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile

from oracle import ROOT, digest, emit, run
from corpus import MANIFEST, fixture_source, snapshot
from contracts import PROFILE_ID, STAGES, validate_manifest, validate_record

MAX_XML = 16 * 1024 * 1024

def differences(expected, observed, path='$', limit=20):
    """Structural differences, no algebraic/floating-point canonicalization."""
    out = []
    def visit(a, b, at):
        if len(out) >= limit:
            return
        if isinstance(a, dict) and isinstance(b, dict):
            for key in sorted(a.keys() | b.keys()):
                if key not in a or key not in b:
                    out.append({'path': at + '.' + key, 'expected': a.get(key), 'observed': b.get(key), 'missing_key': True})
                else:
                    visit(a[key], b[key], at + '.' + key)
        elif isinstance(a, list) and isinstance(b, list):
            if len(a) != len(b):
                out.append({'path': at, 'expected_length': len(a), 'observed_length': len(b)})
            for i, (left, right) in enumerate(zip(a, b)):
                visit(left, right, f'{at}[{i}]')
        elif type(a) is bool or type(b) is bool:
            if type(a) is not type(b) or a != b:
                out.append({'path': at, 'expected': a, 'observed': b})
        elif a != b:
            out.append({'path': at, 'expected': a, 'observed': b})
    visit(expected, observed, path)
    return out[:limit]

def diagnostic(stderr):
    """Parse explicit CLI stage/kind/code protocol; no prose inference."""
    match = re.search(r'^stage=(syntax|resolve|export) code=([A-Za-z0-9_]+) message=(.*)$', stderr, re.MULTILINE)
    if not match:
        return None
    parsed = dict(zip(('stage', 'code', 'message'), match.groups()))
    kinds = {'syntax':'syntax', 'unresolved_symbol':'semantic', 'declaration_conflict':'semantic',
             'unsupported_feature':'unsupported', 'units_inconsistency':'semantic',
             'initialization_cycle':'semantic', 'resource_limit':'resource', 'export':'export'}
    if parsed['code'] not in kinds:
        return None
    parsed['kind'] = kinds[parsed['code']]
    return parsed

def native_export(executable, text, timeout=30):
    with tempfile.TemporaryDirectory(prefix='antimony-native-') as cwd:
        input_path, output_path = Path(cwd) / 'model.ant', Path(cwd) / 'model.xml'
        input_path.write_text(text)
        try:
            child = subprocess.run([str(executable), str(input_path), str(output_path)],
                cwd=cwd, capture_output=True, text=True, timeout=timeout, check=False)
        except (OSError, subprocess.TimeoutExpired) as e:
            return {'status': 'FAIL', 'reason': f'Native process unavailable/timeout: {e}'}
        out = {'returncode': child.returncode, 'stderr': child.stderr[-8000:]}
        if child.returncode == 2:
            parsed = diagnostic(child.stderr)
            if parsed is None or output_path.exists():
                return {'status': 'FAIL', 'reason': 'Native rejection lacks explicit diagnostic or left an output artifact', **out}
            return {'status': 'REJECTED', 'diagnostic': parsed, **out}
        if child.returncode != 0:
            return {'status': 'FAIL', 'reason': 'Native process crashed or returned unexpected exit status', **out}
        if not output_path.is_file() or output_path.stat().st_size > MAX_XML:
            return {'status': 'FAIL', 'reason': 'Native output missing or exceeds 16 MiB', **out}
        try:
            xml = output_path.read_text()
        except (OSError, UnicodeError) as e:
            return {'status': 'FAIL', 'reason': str(e), **out}
        return {'status': 'EXPORTED', 'xml': xml, 'sbml_sha256': digest(xml.encode()), **out}

def check_case(fixture, executable, python=None):
    record = {'schema': 'antimony-rs-compatibility-v1', 'profile_id': PROFILE_ID,
        'fixture': fixture['id'], 'feature_ids': fixture['feature_ids'],
        'source_sha256': fixture['sha256'], 'status': 'FAIL',
        'reference_status': 'NOT_RUN', 'native_stages': {stage: 'NOT_RUN' for stage in STAGES}}
    text = fixture_source(fixture)
    reference = run('convert', text, python)
    record['reference_status'] = reference['status']
    invalid_rejection = (reference['status'] == 'SBML_INVALID'
                         and fixture['reference_status'] == 'SBML_INVALID'
                         and fixture['native_profile_expectation'] == 'strict_profile_rejected')
    if reference['status'] not in ('PASS', 'REFERENCE_REJECTED') and not invalid_rejection:
        record.update(status=reference['status'], reason=reference.get('reason', 'Reference unavailable or invalid'))
        return record
    # Both live reference and reviewed stored expectation must agree; never bless
    # a native export using an inadvertently changed reference contract.
    expected_path = (ROOT / fixture['expectation']).resolve()
    if not expected_path.is_relative_to((ROOT / 'fixtures/manifests').resolve()):
        raise ValueError('Expectation escapes fixture manifests')
    if reference['status'] != fixture['reference_status'] or json.loads(expected_path.read_text()) != snapshot(reference):
        record['reason'] = 'Live reference differs from reviewed expectation'
        return record
    record['oracle'] = reference['oracle']
    if invalid_rejection:
        record['reference_messages'] = reference.get('messages', [])
        record['reference_validation_scope'] = 'Invalid reference: native rejection only; no semantic equivalence credit'
    native = native_export(executable, text)
    record['native_process'] = {k: v for k, v in native.items() if k != 'xml'}
    expected = fixture['native_profile_expectation']
    stages = record['native_stages']
    if native['status'] == 'REJECTED':
        d = native['diagnostic']
        if d['stage'] in ('resolve', 'export'):
            stages['syntax'] = 'PASS'
        if d['stage'] == 'export':
            stages['resolve'] = 'PASS'
        stages[d['stage']] = 'UNSUPPORTED_PROFILE' if d['kind'] == 'unsupported' else 'FAIL'
        rejection_matches = (
            expected == 'semantic_rejected' and d['stage'] == 'resolve' and d['kind'] == 'semantic'
            or expected == 'malformed' and d['stage'] == 'syntax' and d['kind'] == 'syntax'
            or expected == 'unsupported_profile' and d['kind'] == 'unsupported'
            or expected == 'strict_profile_rejected' and d['stage'] == 'resolve' and d['kind'] in ('semantic', 'unsupported'))
        record.update(status='PASS' if rejection_matches else 'FAIL',
                      outcome='expected_rejection' if rejection_matches else 'unexpected_rejection')
        return record
    if native['status'] != 'EXPORTED':
        record['reason'] = native.get('reason', 'Native process failed')
        return record
    stages.update(syntax='PASS', resolve='PASS')
    if expected != 'target_supported':
        stages['export'] = 'FAIL'
        record.update(reason='Native exported a fixture requiring explicit rejection', outcome='unexpected_acceptance')
        return record
    inspection = run('inspect', native['xml'], python)
    record['sbml_inspection_status'] = inspection['status']
    record['sbml_messages'] = inspection.get('messages', [])
    if inspection['status'] != 'PASS':
        stages['export'] = 'FAIL'
        record.update(reason='Independent SBML inspection failed', inspection_detail={k:v for k,v in inspection.items() if k not in ('oracle','projection')})
        return record
    delta = differences(reference['projection'], inspection['projection'])
    stages['export'] = 'FAIL' if delta else 'PASS'
    record.update(status='FAIL' if delta else 'PASS', outcome='semantic_mismatch' if delta else 'supported_export', differences=delta)
    return record

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--native', required=True, help='Prebuilt local example executable path')
    parser.add_argument('--trusted-native', action='store_true', help='Acknowledge execution of this trusted local program')
    parser.add_argument('--python', help='Pinned oracle interpreter override')
    parser.add_argument('--fixture', action='append', help='Select fixture IDs; default all owned cases')
    parser.add_argument('--current-profile', action='store_true', help='Apply explicit versioned native-outcome overlay')
    args = parser.parse_args()
    if not args.trusted_native:
        parser.error('--trusted-native is required to execute a program')
    executable = Path(args.native).resolve()
    if not executable.is_file() or not os.access(executable, os.X_OK):
        parser.error('Native executable is absent/not executable; build it explicitly first')
    manifest_bytes = MANIFEST.read_bytes()
    manifest_hash = digest(manifest_bytes)
    manifest = json.loads(manifest_bytes)
    validate_manifest(manifest, json.loads((ROOT / 'profiles/kinetic-core-v0.1.json').read_text()))
    overlay_hash = digest(profile_overlay.PATH.read_bytes()) if args.current_profile else None
    if args.current_profile:
        manifest = profile_overlay.original(manifest)
    selected = set(args.fixture or [f['id'] for f in manifest['fixtures']])
    if not selected <= {f['id'] for f in manifest['fixtures']}:
        parser.error('Unknown fixture ID')
    executable_hash = digest(executable.read_bytes())
    records = []
    for fixture in manifest['fixtures']:
        if fixture['id'] not in selected:
            continue
        try:
            record = check_case(fixture, executable, args.python)
        except (OSError, ValueError, KeyError, TypeError) as e:
            record = {'schema':'antimony-rs-compatibility-v1', 'profile_id':PROFILE_ID,
                'fixture':fixture['id'], 'source_sha256':fixture['sha256'], 'feature_ids':fixture['feature_ids'],
                'status':'FAIL', 'reference_status':'NOT_RUN', 'reason':str(e), 'native_stages':{s:'NOT_RUN' for s in STAGES}}
        if args.current_profile:
            record.update(implemented_profile=profile_overlay.PROFILE, corpus_profile_id='kinetic-core-v0.1',
                          profile_scope='Historical corpus under expanded implementation; not a frozen-target conformance claim')
        validate_record(record)
        records.append(record)
    # Deletion/read failure after execution also invalidates evidence, rather than
    # raising before a structured report can explain the changed input.
    def changed(path, expected_hash):
        try:
            return digest(path.read_bytes()) != expected_hash
        except OSError:
            return True

    binary_changed = changed(executable, executable_hash)
    manifest_changed = changed(MANIFEST, manifest_hash)
    overlay_changed = args.current_profile and changed(profile_overlay.PATH, overlay_hash)
    success = not overlay_changed and bool(records) and not binary_changed and not manifest_changed and all(r['status'] == 'PASS' for r in records)
    emit({'schema':'antimony-rs-native-check-v1', 'status':'PASS' if success else 'FAIL',
        'implemented_profile':profile_overlay.PROFILE if args.current_profile else 'historical', 'overlay_sha256':overlay_hash, 'overlay_changed':bool(overlay_changed),
        'native_executable_sha256':executable_hash, 'native_binary_changed_during_check':binary_changed,
        'source_manifest_sha256':manifest_hash, 'manifest_changed_during_check':manifest_changed,
        'cases':records, 'counts':{
            'supported_exports':sum(r.get('outcome') == 'supported_export' for r in records),
            'expected_rejections':sum(r.get('outcome') == 'expected_rejection' for r in records),
            'failed_or_blocked':sum(r['status'] != 'PASS' for r in records),
            'numerical_not_run':len(records)}})
    return 0 if success else 2

if __name__ == '__main__':
    raise SystemExit(main())
