#!/usr/bin/env python3
"""Record/check only repository-owned, hash-pinned fixtures (never arbitrary imports)."""
import argparse
import json
from pathlib import Path
from oracle import ROOT, SCHEMA, digest, emit, run
from contracts import PROFILE_ID, STAGES, validate_manifest, validate_record
MANIFEST = ROOT / 'fixtures/manifests/original-v1.json'

def snapshot(response):
    out = {key: response[key] for key in ['schema', 'status', 'source_sha256']}
    if 'projection' in response:
        out['projection'] = response['projection']
    if 'diagnostic' in response:
        out['diagnostic'] = response['diagnostic']
    out['messages'] = [{'id': x['id'], 'severity': x['severity'], 'category': x['category']} for x in response.get('messages', [])]
    out['oracle_versions'] = response['oracle']['packages']
    out['antimony_library_version'] = response['oracle']['antimony_library_version']
    out['antimony_bundled_libsbml'] = response['oracle'].get('antimony_bundled_libsbml_reported_by_xml')
    return out

def fixture_source(fixture):
    path = (ROOT / fixture['path']).resolve()
    if not path.is_relative_to((ROOT / 'fixtures').resolve()):
        raise ValueError('Fixture path escapes owned corpus')
    data = path.read_bytes()
    if digest(data) != fixture['sha256']:
        raise ValueError(f"Hash mismatch: {fixture['id']}; review fixture before updating manifest")
    # Defense-in-depth for this corpus; not a general Antimony safety parser.
    if 'import' in data.decode().lower():
        raise ValueError('Imports are never sent to the corpus reference process')
    return data.decode()

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('mode', choices=['check', 'record'])
    parser.add_argument('--python')
    args = parser.parse_args()
    manifest = json.loads(MANIFEST.read_text())
    try:
        validate_manifest(manifest, json.loads((ROOT / 'profiles/kinetic-core-v0.1.json').read_text()))
    except (ValueError, KeyError, TypeError) as e:
        emit({'schema': SCHEMA, 'status': 'FAIL', 'reason': str(e)})
        return 2
    reports = []
    for fixture in manifest['fixtures']:
        try:
            response = run('convert', fixture_source(fixture), args.python)
            if response['status'] not in ('PASS', 'REFERENCE_REJECTED', 'SBML_INVALID'):
                reports.append({'fixture': fixture['id'], 'status': response['status'], 'reason': response.get('reason')})
                continue
            observed = snapshot(response)
            expected = (ROOT / fixture['expectation']).resolve()
            if not expected.is_relative_to((ROOT / 'fixtures/manifests').resolve()):
                raise ValueError('Expectation path escapes fixture manifests')
            if response['status'] != fixture['reference_status']:
                raise ValueError('Observed reference status differs from reviewed intended case')
            if args.mode == 'record':
                expected.write_text(json.dumps(observed, indent=2, sort_keys=True) + '\n')
            matches = expected.exists() and json.loads(expected.read_text()) == observed
            reports.append({'fixture': fixture['id'], 'status': 'PASS' if matches else 'FAIL',
                            'reference_status': response['status'], 'native_status': 'NOT_RUN'})
        except (OSError, ValueError, KeyError) as e:
            reports.append({'fixture': fixture['id'], 'status': 'FAIL', 'reason': str(e)})
    for report, fixture in zip(reports, manifest['fixtures']):
        report.update({'schema': 'antimony-rs-compatibility-v1', 'profile_id': PROFILE_ID,
                       'feature_ids': fixture['feature_ids'], 'source_sha256': fixture['sha256'],
                       'native_stages': {stage: 'NOT_RUN' for stage in STAGES}})
        report.setdefault('reference_status', report['status'])
        validate_record(report)
    emit({'schema': SCHEMA, 'mode': args.mode, 'cases': reports,
          'counts': {status: sum(x['status'] == status for x in reports) for status in sorted({x['status'] for x in reports})}})
    return 0 if reports and all(x['status'] == 'PASS' for x in reports) else 2

if __name__ == '__main__':
    raise SystemExit(main())
