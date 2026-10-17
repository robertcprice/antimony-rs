#!/usr/bin/env python3
"""Trusted prebuilt CLI -> existing examples + live reviewed semantic contracts.

No builds/downloads; subprocess working-directory isolation is not a sandbox.
"""
import argparse
import profile_overlay
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile

from oracle import ROOT, digest, emit, run, MAX_INPUT
from corpus import MANIFEST, fixture_source
from contracts import validate_manifest
import native_check as native
import roundtrip_check as formatting

REPORT_SCHEMA = ROOT / 'docs/cli-report.schema.json'
MAX_REPORT = 8 * 1024 * 1024


def validate_shape(value, spec, root=None):
    """Validate exactly the closed JSON-Schema subset used by our report contract."""
    root = root or spec
    known = {'$schema', 'title', 'description', '$defs', '$ref', 'anyOf', 'type', 'enum',
             'const', 'minimum', 'minLength', 'properties', 'required', 'additionalProperties', 'items'}
    if set(spec) - known:
        raise ValueError('Unsupported schema vocabulary')
    if '$ref' in spec:
        name = spec['$ref'].removeprefix('#/$defs/')
        if name not in root['$defs']:
            raise ValueError('Unknown schema reference')
        return validate_shape(value, root['$defs'][name], root)
    if 'anyOf' in spec:
        for variant in spec['anyOf']:
            try:
                validate_shape(value, variant, root)
                return
            except ValueError:
                pass
        raise ValueError('No report schema alternative matches')
    if 'const' in spec and (type(value) is not type(spec['const']) or value != spec['const']):
        raise ValueError('Wrong report schema constant')
    if 'enum' in spec and not any(type(value) is type(x) and value == x for x in spec['enum']):
        raise ValueError('Invalid report enum')
    kind = spec.get('type')
    types = {'object': dict, 'array': list, 'string': str, 'integer': int, 'boolean': bool, 'null': type(None)}
    if kind and (kind not in types or type(value) is not types[kind]):
        raise ValueError('Wrong report value type')
    if 'minimum' in spec and value < spec['minimum']:
        raise ValueError('Report number below minimum')
    if 'minLength' in spec and len(value) < spec['minLength']:
        raise ValueError('Empty report string')
    if kind == 'object':
        if not set(spec.get('required', [])) <= set(value):
            raise ValueError('Missing report field')
        properties = spec.get('properties', {})
        if spec.get('additionalProperties') is False and set(value) - set(properties):
            raise ValueError('Unknown report field')
        for key, item in value.items():
            if key in properties:
                validate_shape(item, properties[key], root)
    if kind == 'array':
        for item in value:
            validate_shape(item, spec['items'], root)


def position(text, offset):
    data = text.encode('utf-8')
    if not 0 <= offset <= len(data):
        raise ValueError('Label offset outside original source')
    data[:offset].decode('utf-8')  # rejects mid-codepoint offsets
    starts = [0]
    i = 0
    while i < len(data):
        if data[i] == 13:
            i += 1
            if i < len(data) and data[i] == 10:
                i += 1
            starts.append(i)
        elif data[i] == 10:
            i += 1
            starts.append(i)
        else:
            i += 1
    line = max(i for i, start in enumerate(starts) if start <= offset)
    return {'line': line + 1, 'column': len(data[starts[line]:offset].decode()) + 1,
            'byte_column': offset - starts[line] + 1}


def validate_report(report, command, returncode, text):
    validate_shape(report, json.loads(REPORT_SCHEMA.read_text()))
    if report['command'] != command or report['exit_code'] != returncode:
        raise ValueError('Command/exit report disagrees with process')
    if report['status'] != {0: 'success', 1: 'error', 2: 'rejected'}.get(returncode):
        raise ValueError('Status/exit inconsistency')
    if report['profile_id'] != profile_overlay.PROFILE:
        raise ValueError('Unexpected implemented profile')
    if report['source'] is None:
        raise ValueError('Corpus command did not identify source')
    for d in report['diagnostics']:
        for label in d['labels']:
            if label['start_byte'] > label['end_byte']:
                raise ValueError('Reversed source label')
            for endpoint in ('start', 'end'):
                if label[endpoint] != position(text, label[endpoint + '_byte']):
                    raise ValueError('Label position disagrees with original UTF-8 source')
    if returncode == 0:
        if report['model'] is None or any(d['severity'] == 'error' for d in report['diagnostics']):
            raise ValueError('Success lacks model or contains errors')
    if returncode == 2 and not any(d['severity'] == 'error' for d in report['diagnostics']):
        raise ValueError('Rejection lacks error diagnostic')
    output = report['output']
    if command == 'check':
        if output is not None:
            raise ValueError('Check unexpectedly reports an artifact')
    elif output is None or output['destination'] != 'file' or output['path'] is None:
        raise ValueError('JSON artifact command lacks file destination')
    elif output['written'] != (returncode == 0):
        raise ValueError('Output completion disagrees with command result')


def unique_object(pairs):
    out = {}
    for key, value in pairs:
        if key in out:
            raise ValueError('Duplicate JSON report key')
        out[key] = value
    return out


def cli_process(executable, command, text, timeout=30):
    with tempfile.TemporaryDirectory(prefix='antimony-cli-check-') as cwd:
        inp, out = Path(cwd) / 'model.ant', Path(cwd) / 'artifact'
        inp.write_bytes(text.encode())
        argv = [str(executable), command, str(inp), '--json']
        if command != 'check':
            argv += ['-o', str(out)]
        try:
            child = subprocess.run(argv, cwd=cwd, capture_output=True, timeout=timeout, check=False)
        except FileNotFoundError as e:
            return {'status': 'ORACLE_BLOCKED', 'reason': f'Prebuilt CLI unavailable: {e}'}
        except (OSError, subprocess.TimeoutExpired) as e:
            return {'status': 'FAIL', 'reason': f'CLI execution failed: {e}'}
        try:
            if len(child.stdout) > MAX_REPORT or child.stderr:
                raise ValueError('JSON report too large or mixed stderr output')
            report = json.loads(child.stdout.decode('utf-8'), object_pairs_hook=unique_object)
            validate_report(report, command, child.returncode, text)
            if report['source'] != {'display': str(inp), 'lossy': False}:
                raise ValueError('Report names a different source')
            if command != 'check' and report['output']['path'] != {'display': str(out), 'lossy': False}:
                raise ValueError('Report names a different artifact')
            if child.returncode not in (0, 2):
                raise ValueError('Corpus command returned unexpected usage/I/O/feature error')
            if child.returncode == 2:
                if out.exists():
                    raise ValueError('Rejected command left artifact')
                return {'status': 'REJECTED', 'report': report}
            data = None
            if command != 'check':
                maximum = MAX_INPUT if command == 'format' else native.MAX_XML
                if not out.is_file() or not 0 < out.stat().st_size <= maximum:
                    raise ValueError('Artifact absent, empty or oversized')
                data = out.read_bytes()
                data.decode('utf-8')
            elif out.exists():
                raise ValueError('Check created an artifact')
            return {'status': 'SUCCESS', 'report': report, 'artifact': data}
        except (ValueError, KeyError, TypeError, OSError, UnicodeError) as e:
            return {'status': 'FAIL', 'reason': str(e), 'returncode': child.returncode}


def warnings(report):
    return [(d['stage'], d['code'], d['message']) for d in report['diagnostics'] if d['severity'] == 'warning']


def check_case(fixture, cli, exporter, formatter, python=None):
    baseline = native.check_case(fixture, exporter, python)
    record = {'fixture': fixture['id'], 'source_sha256': fixture['sha256'],
              'status': baseline['status'], 'reference_status': baseline['reference_status'],
              'outcome': 'NOT_RUN', 'commands': {}, 'numerical': 'NOT_RUN'}
    if baseline['status'] != 'PASS':
        record['reason'] = baseline.get('reason', 'Original reference/native gate unavailable or failed')
        return record
    text = fixture_source(fixture)
    results = {command: cli_process(cli, command, text) for command in ('check', 'format', 'export')}
    record['commands'] = {command: {k: v for k, v in value.items() if k != 'artifact'} for command, value in results.items()}
    if any(value['status'] not in ('SUCCESS', 'REJECTED') for value in results.values()):
        record['status'] = 'ORACLE_BLOCKED' if any(x['status'] == 'ORACLE_BLOCKED' for x in results.values()) else 'FAIL'
        return record
    record['status'] = 'FAIL'
    if baseline['outcome'] == 'expected_rejection':
        expected = baseline['native_process']['diagnostic']
        for result in results.values():
            errors = [d for d in result['report']['diagnostics'] if d['severity'] == 'error']
            if result['status'] != 'REJECTED' or not any(d['stage'] == expected['stage'] and d['code'] == expected['code'] for d in errors):
                record['reason'] = 'CLI rejection differs from original native stage/code'
                return record
        record.update(status='PASS', outcome='expected_rejection')
        return record
    if any(x['status'] != 'SUCCESS' for x in results.values()):
        record['reason'] = 'CLI rejected supported fixture'
        return record
    expected_warnings = re.findall(r'^stage=(\w+) severity=Warning code=(\w+) message=(.*)$', baseline['native_process'].get('stderr', ''), re.MULTILINE)
    if any(warnings(x['report']) != expected_warnings for x in results.values()):
        record['reason'] = 'CLI warning sequence differs from existing pipeline'
        return record
    golden = json.loads((ROOT / fixture['expectation']).read_text())['projection']
    model = {'id': golden['model']['id'], **{k: len(golden[k]) for k in ('compartments', 'species', 'parameters', 'reactions', 'initial_assignments', 'unit_definitions')}}
    if any(x['report']['model'] != model for x in results.values()):
        record['reason'] = 'CLI model summary differs from independently inspected model'
        return record
    existing = formatting.native_format(formatter, text)
    if existing['status'] != 'FORMATTED' or results['format']['artifact'] != existing['bytes']:
        record['reason'] = 'CLI format bytes differ from existing formatter'
        return record
    formatted = results['format']['artifact'].decode()
    if 'import' in formatted.lower():
        record['reason'] = 'Forbidden import text in generated corpus source'
        return record
    second = cli_process(cli, 'format', formatted)
    if second['status'] != 'SUCCESS' or second['artifact'] != results['format']['artifact']:
        record['reason'] = 'CLI formatting is not byte-idempotent'
        return record
    exported = native.native_export(exporter, text)
    if exported['status'] != 'EXPORTED' or results['export']['artifact'] != exported['xml'].encode():
        record['reason'] = 'CLI export bytes differ from existing exporter'
        return record
    inspection = run('inspect', results['export']['artifact'].decode(), python)
    reference = run('convert', formatted, python)
    for name, result in [('export', inspection), ('formatted_reference', reference)]:
        record[name + '_status'] = result['status']
        record[name + '_warning_ids'] = [m['id'] for m in result.get('messages', []) if m['severity'] == 'Warning']
        if result['status'] != 'PASS':
            record.update(status='ORACLE_BLOCKED' if result['status'] == 'ORACLE_BLOCKED' else 'FAIL', reason=name + ' oracle did not pass')
            return record
        delta = native.differences(golden, result['projection'])
        if delta:
            record.update(reason=name + ' differs from reviewed projection', differences=delta)
            return record
    record.update(status='PASS', outcome='supported_cli', byte_idempotent=True,
                  formatted_sha256=digest(results['format']['artifact']), sbml_sha256=digest(results['export']['artifact']))
    return record


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for flag in ('cli', 'native', 'formatter'):
        parser.add_argument('--' + flag, required=True, help='Trusted prebuilt executable')
    parser.add_argument('--trusted-native', action='store_true')
    parser.add_argument('--python')
    parser.add_argument('--fixture', action='append')
    parser.add_argument('--current-profile', action='store_true', help='Apply explicit versioned native-outcome overlay')
    args = parser.parse_args()
    if not args.trusted_native:
        parser.error('--trusted-native required; no binaries are built automatically')
    manifest_bytes = MANIFEST.read_bytes()
    manifest = json.loads(manifest_bytes)
    validate_manifest(manifest, json.loads((ROOT / 'profiles/kinetic-core-v0.1.json').read_text()))
    overlay_hash = digest(profile_overlay.PATH.read_bytes()) if args.current_profile else None
    if args.current_profile:
        manifest = profile_overlay.original(manifest)
    selected = set(args.fixture or [f['id'] for f in manifest['fixtures']])
    if not selected <= {f['id'] for f in manifest['fixtures']}:
        parser.error('Unknown fixture ID')
    binaries = {name: Path(getattr(args, name)).resolve() for name in ('cli', 'native', 'formatter')}
    missing = [name for name, path in binaries.items() if not path.is_file() or not os.access(path, os.X_OK)]
    if missing:
        emit({'schema': 'antimony-rs-cli-check-v1', 'status': 'ORACLE_BLOCKED', 'reason': 'Missing trusted prebuilt binaries: ' + ', '.join(missing), 'counts': {'supported_cli': 0, 'expected_rejections': 0, 'failed_or_blocked': len(selected), 'numerical_not_run': len(selected)}})
        return 2
    inputs = {**binaries, 'manifest': MANIFEST, 'report_schema': REPORT_SCHEMA, 'profile': ROOT / 'profiles/kinetic-core-v0.1.json'}
    inputs.update({name: ROOT / 'tools/oracle' / name for name in ('oracle.py', 'corpus.py', 'contracts.py', 'native_check.py', 'roundtrip_check.py', 'cli_check.py')})
    if args.current_profile:
        inputs['overlay'] = profile_overlay.PATH
        inputs['overlay_checker'] = Path(profile_overlay.__file__)
    hashes = {name: digest(path.read_bytes()) for name, path in inputs.items()}
    records = []
    for fixture in manifest['fixtures']:
        if fixture['id'] not in selected:
            continue
        try:
            record = check_case(fixture, binaries['cli'], binaries['native'], binaries['formatter'], args.python)
        except (ValueError, KeyError, TypeError, OSError) as e:
            record = {'fixture': fixture['id'], 'status': 'FAIL', 'reason': str(e), 'outcome': 'NOT_RUN', 'numerical': 'NOT_RUN'}
        if args.current_profile:
            record.update(implemented_profile=profile_overlay.PROFILE, corpus_profile_id='kinetic-core-v0.1',
                          profile_scope='Historical corpus under expanded implementation; not a frozen-target conformance claim')
        records.append(record)
    changed = [name for name, path in inputs.items() if not path.is_file() or digest(path.read_bytes()) != hashes[name]]
    success = bool(records) and not changed and all(r['status'] == 'PASS' for r in records)
    emit({'schema': 'antimony-rs-cli-check-v1', 'status': 'PASS' if success else 'FAIL',
          'implemented_profile':profile_overlay.PROFILE if args.current_profile else 'historical', 'input_sha256': hashes, 'changed_inputs': changed, 'cases': records,
          'counts': {'supported_cli': sum(r['outcome'] == 'supported_cli' for r in records),
                     'expected_rejections': sum(r['outcome'] == 'expected_rejection' for r in records),
                     'failed_or_blocked': sum(r['status'] != 'PASS' for r in records),
                     'numerical_not_run': len(records)}})
    return 0 if success else 2


if __name__ == '__main__':
    raise SystemExit(main())
