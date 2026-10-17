#!/usr/bin/env python3
"""Trusted native formatter -> byte idempotence and exact native/reference semantics.

Build binaries explicitly first. No Cargo, downloads, imports or simulator here.
Subprocess isolation is not a sandbox; only owned corpus inputs are accepted.
"""
from __future__ import annotations
import argparse
import profile_overlay
import json
import os
from pathlib import Path
import subprocess
import tempfile

from oracle import ROOT, MAX_INPUT, digest, emit, run
from corpus import MANIFEST, fixture_source
from contracts import PROFILE_ID, STAGES, validate_manifest
import native_check as native


def native_format(executable, text, timeout=30):
    with tempfile.TemporaryDirectory(prefix='antimony-format-') as cwd:
        inp, out = Path(cwd)/'input.ant', Path(cwd)/'output.ant'
        inp.write_text(text)
        try:
            child = subprocess.run([str(executable), str(inp), str(out)], cwd=cwd,
                                   capture_output=True, text=True, timeout=timeout, check=False)
        except (OSError, UnicodeError, subprocess.TimeoutExpired) as e:
            return {'status':'FAIL', 'reason':f'Formatter unavailable/timeout: {e}'}
        detail = {'returncode':child.returncode, 'stderr':child.stderr[-8000:]}
        if child.returncode == 2:
            d = native.diagnostic(child.stderr)
            if d is None or out.exists():
                return {'status':'FAIL','reason':'Formatter rejection lacks explicit diagnostic or left an artifact',**detail}
            return {'status':'REJECTED','diagnostic':d,**detail}
        if child.returncode != 0:
            return {'status':'FAIL','reason':'Formatter crashed or returned unexpected exit status',**detail}
        if not out.is_file() or not 0 < out.stat().st_size <= MAX_INPUT:
            return {'status':'FAIL','reason':'Formatted output absent, empty or exceeds 4 MiB',**detail}
        try:
            data = out.read_bytes()
            formatted = data.decode('utf-8')
        except (OSError, UnicodeError) as e:
            return {'status':'FAIL','reason':str(e),**detail}
        # Defense in depth, not a general safe-Antimony parser or a sandbox.
        if 'import' in formatted.lower():
            return {'status':'FAIL','reason':'Formatted output contains forbidden corpus import text',**detail}
        return {'status':'FORMATTED','text':formatted,'bytes':data,
                'formatted_sha256':digest(data),**detail}


def rejection_matches(expected, diagnostic):
    stage, kind = diagnostic['stage'], diagnostic['kind']
    return (expected == 'semantic_rejected' and stage == 'resolve' and kind == 'semantic'
        or expected == 'malformed' and stage == 'syntax' and kind == 'syntax'
        or expected == 'unsupported_profile' and kind == 'unsupported'
        or expected == 'strict_profile_rejected' and stage == 'resolve' and kind in ('semantic','unsupported'))


def check_case(fixture, formatter, exporter, python=None):
    # First prove original reviewed reference AND native export/rejection. A
    # missing oracle never reaches the formatter, even for a negative fixture.
    baseline = native.check_case(fixture, exporter, python)
    record = dict(baseline)
    record['schema'] = 'antimony-rs-roundtrip-case-v1'
    record['original_native_outcome'] = record.pop('outcome',None)
    record.update(format_status='NOT_RUN',idempotence='NOT_RUN',
                  formatted_reference_status='NOT_RUN',formatted_native_status='NOT_RUN',
                  roundtrip_outcome='NOT_RUN')
    if baseline['status'] != 'PASS':
        return record
    record['status'] = 'FAIL'
    first = native_format(formatter, fixture_source(fixture))
    record['formatter_process'] = {k:v for k,v in first.items() if k not in ('text','bytes')}
    record['format_status'] = first['status']
    expected = fixture['native_profile_expectation']
    if first['status'] == 'REJECTED':
        matched = rejection_matches(expected,first['diagnostic'])
        record.update(status='PASS' if matched else 'FAIL',
                      roundtrip_outcome='expected_rejection' if matched else 'unexpected_rejection')
        return record
    if first['status'] != 'FORMATTED':
        record['reason'] = first.get('reason','Formatter failed')
        return record
    if expected != 'target_supported':
        record.update(reason='Formatter accepted a fixture requiring explicit rejection',
                      roundtrip_outcome='unexpected_acceptance')
        return record
    second = native_format(formatter,first['text'])
    record['second_formatter_process'] = {k:v for k,v in second.items() if k not in ('text','bytes')}
    if second['status'] != 'FORMATTED' or first['bytes'] != second['bytes']:
        record.update(idempotence='FAIL',reason='Formatting twice is not byte-identical')
        return record
    record['idempotence'] = 'PASS'
    reference = run('convert',first['text'],python)
    record['formatted_reference_status'] = reference['status']
    record['formatted_reference_messages'] = reference.get('messages',[])
    if reference['status'] != 'PASS':
        record.update(status=reference['status'] if reference['status']=='ORACLE_BLOCKED' else 'FAIL',
                      reason='Formatted source did not produce valid supported reference SBML')
        return record
    golden = json.loads((ROOT/fixture['expectation']).read_text())['projection']
    delta = native.differences(golden,reference['projection'])
    record['formatted_reference_differences'] = delta
    if delta:
        record.update(reason='Formatted reference semantics differ from original reviewed projection',
                      roundtrip_outcome='semantic_mismatch')
        return record
    exported = native.native_export(exporter,first['text'])
    record['formatted_native_process'] = {k:v for k,v in exported.items() if k!='xml'}
    if exported['status'] != 'EXPORTED':
        record.update(formatted_native_status='FAIL',reason='Native rejected/failed formatted source')
        return record
    inspection = run('inspect',exported['xml'],python)
    record['formatted_native_status'] = inspection['status']
    record['formatted_native_messages'] = inspection.get('messages',[])
    if inspection['status'] != 'PASS':
        record.update(status=inspection['status'] if inspection['status']=='ORACLE_BLOCKED' else 'FAIL',
                      reason='Formatted native SBML failed independent inspection')
        return record
    delta = native.differences(golden,inspection['projection'])
    record['formatted_native_differences'] = delta
    record.update(status='FAIL' if delta else 'PASS',
                  roundtrip_outcome='semantic_mismatch' if delta else 'supported_roundtrip')
    return record


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--formatter',required=True)
    p.add_argument('--native',required=True,help='Prebuilt SBML exporter')
    p.add_argument('--trusted-native',action='store_true')
    p.add_argument('--python')
    p.add_argument('--fixture',action='append')
    p.add_argument('--current-profile', action='store_true', help='Apply explicit versioned native-outcome overlay')
    args=p.parse_args()
    if not args.trusted_native:
        p.error('--trusted-native required to execute both local programs')
    binaries={'formatter':Path(args.formatter).resolve(),'exporter':Path(args.native).resolve()}
    for path in binaries.values():
        if not path.is_file() or not os.access(path,os.X_OK):
            p.error('Native binary absent/not executable; build explicitly first')
    manifest_bytes=MANIFEST.read_bytes()
    manifest_hash=digest(manifest_bytes)
    manifest=json.loads(manifest_bytes)
    validate_manifest(manifest,json.loads((ROOT/'profiles/kinetic-core-v0.1.json').read_text()))
    overlay_hash = digest(profile_overlay.PATH.read_bytes()) if args.current_profile else None
    if args.current_profile:
        manifest = profile_overlay.original(manifest)
    selected=set(args.fixture or [f['id'] for f in manifest['fixtures']])
    if not selected <= {f['id'] for f in manifest['fixtures']}:
        p.error('Unknown fixture ID')
    hashes={k:digest(v.read_bytes()) for k,v in binaries.items()}
    records=[]
    for f in manifest['fixtures']:
        if f['id'] not in selected: continue
        try:
            record=check_case(f,binaries['formatter'],binaries['exporter'],args.python)
        except (OSError,ValueError,KeyError,TypeError) as e:
            record={'schema':'antimony-rs-roundtrip-case-v1','profile_id':PROFILE_ID,
                    'fixture':f['id'],'feature_ids':f['feature_ids'],
                    'source_sha256':f['sha256'],'status':'FAIL','reference_status':'NOT_RUN',
                    'reason':str(e),'native_stages':{stage:'NOT_RUN' for stage in STAGES},
                    'format_status':'NOT_RUN','idempotence':'NOT_RUN',
                    'formatted_reference_status':'NOT_RUN','formatted_native_status':'NOT_RUN',
                    'roundtrip_outcome':'NOT_RUN'}
        if args.current_profile:
            record.update(implemented_profile=profile_overlay.PROFILE, corpus_profile_id='kinetic-core-v0.1',
                          profile_scope='Historical corpus under expanded implementation; not a frozen-target conformance claim')
        records.append(record)
    changed=any(digest(v.read_bytes())!=hashes[k] for k,v in binaries.items())
    manifest_changed=digest(MANIFEST.read_bytes())!=manifest_hash
    overlay_changed=args.current_profile and (not profile_overlay.PATH.is_file() or digest(profile_overlay.PATH.read_bytes())!=overlay_hash)
    success=not overlay_changed and bool(records) and not changed and not manifest_changed and all(r['status']=='PASS' for r in records)
    emit({'schema':'antimony-rs-roundtrip-check-v1','status':'PASS' if success else 'FAIL',
          'implemented_profile':profile_overlay.PROFILE if args.current_profile else 'historical', 'overlay_sha256':overlay_hash, 'overlay_changed':bool(overlay_changed),
          'native_executable_sha256':hashes,'native_binary_changed_during_check':changed,
          'source_manifest_sha256':manifest_hash,'manifest_changed_during_check':manifest_changed,'cases':records,
          'counts':{'supported_roundtrips':sum(r.get('roundtrip_outcome')=='supported_roundtrip' for r in records),
                    'expected_rejections':sum(r.get('roundtrip_outcome')=='expected_rejection' for r in records),
                    'byte_idempotent':sum(r.get('idempotence')=='PASS' for r in records),
                    'failed_or_blocked':sum(r['status']!='PASS' for r in records),
                    'numerical_not_run':len(records)}})
    return 0 if success else 2

if __name__=='__main__':
    raise SystemExit(main())
