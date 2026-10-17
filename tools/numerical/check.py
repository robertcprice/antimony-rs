#!/usr/bin/env python3
"""Explicit dev-only native/reference SBML versus independent analytic anchors.

No builds, installation, downloads, or mutation of the original 90-case corpus.
Fresh workers isolate state; they are not security sandboxes. Trusted inputs only.
"""
from __future__ import annotations
import argparse
import json
import math
import os
from pathlib import Path
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'tools/oracle'))
from oracle import digest, emit, run as reference_run
from native_check import native_export, differences
import anchors

MANIFEST = ROOT / 'fixtures/numerical/manifest.json'
WORKER = ROOT / 'tools/numerical/worker.py'
SCHEMA = 'antimony-numerical-check-v1'
WORKER_SCHEMA = 'antimony-numerical-worker-v1'
GRID = {'start': 0, 'end': 10, 'steps': 100}
COMPARISON = {'absolute': 1e-8, 'relative': 1e-7}
SOLVER = {'name': 'cvode', 'relative_tolerance': 1e-10,
          'absolute_tolerance': 1e-12, 'maximum_num_steps': 100000,
          'variable_step_size': False, 'stiff': True}
STAGES = ('reference', 'export', 'projection', 'native_simulation',
          'reference_simulation', 'native_analytic', 'reference_analytic',
          'differential', 'invariants')
MAX_OUTPUT = 4 * 1024 * 1024


def finite(value):
    try:
        return type(value) in (int, float) and math.isfinite(value)
    except OverflowError:
        return False


def unique_object(pairs):
    out = {}
    for key, value in pairs:
        if key in out:
            raise ValueError('Duplicate JSON key')
        out[key] = value
    return out


def load_json(data):
    return json.loads(data, object_pairs_hook=unique_object,
                      parse_constant=lambda x: (_ for _ in ()).throw(ValueError('Nonfinite JSON: ' + x)))


def source(case):
    path = (ROOT / case['path']).resolve()
    if not path.is_relative_to((ROOT / 'fixtures/numerical').resolve()):
        raise ValueError('Numerical source escapes owned fixtures')
    data = path.read_bytes()
    if len(data) > 4 * 1024 * 1024 or digest(data) != case['sha256']:
        raise ValueError('Numerical fixture size/hash mismatch')
    text = data.decode('utf-8')
    if 'import' in text.lower():
        raise ValueError('Imports forbidden in numerical sources')
    return text


def validate_manifest(manifest):
    anchors.validate_manifest(manifest)
    if manifest.get('schema') != 'antimony-numerical-manifest-v1':
        raise ValueError('Wrong numerical manifest schema')
    if manifest.get('grid') != GRID or manifest.get('comparison') != COMPARISON or manifest.get('solver') != SOLVER:
        raise ValueError('Frozen grid/tolerances/solver changed; requires review')
    cases = manifest.get('cases')
    if not isinstance(cases, list) or len(cases) != 8:
        raise ValueError('Exactly eight selected numerical anchors required')
    ids = set()
    for case in cases:
        if case['id'] in ids:
            raise ValueError('Duplicate numerical anchor')
        ids.add(case['id'])
        selections = case['selections']
        if not isinstance(selections, list) or not selections or selections[0] != 'time' or len(selections) != len(set(selections)):
            raise ValueError('Invalid or duplicate selections')
        if set(selections[1:]) != set(anchors.expected_values(case['id'], 0)):
            raise ValueError('Selections do not exactly cover independent anchor')
        source(case)


def fingerprints(manifest, native, python=None):
    paths = {'native': Path(native), 'manifest': MANIFEST,
             'profile': ROOT / 'profiles/kinetic-core-v0.1.json'}
    for folder, names in [('tools/oracle', ['oracle.py', 'native_check.py', 'contracts.py', 'corpus.py']),
                          ('tools/numerical', ['check.py', 'anchors.py', 'worker.py'])]:
        paths.update({folder + '/' + name: ROOT / folder / name for name in names})
    paths.update({case['path']: ROOT / case['path'] for case in manifest['cases']})
    for name in ('environment.json', 'wheels.json', 'requirements-macos-arm64-py312.txt'):
        paths['tools/numerical/' + name] = ROOT / 'tools/numerical' / name
    environment = load_json(paths['tools/numerical/environment.json'].read_text())
    interpreter = Path(python or ROOT / '.venv-numerical/bin/python').absolute()
    paths['numerical_python'] = interpreter
    site = interpreter.parent.parent / 'lib/python3.12/site-packages'
    for relative, metadata in environment['installed_files'].items():
        path = site / relative
        if digest(path.read_bytes()) != metadata['sha256']:
            raise ValueError('Pinned installed runtime identity differs: ' + relative)
        paths['installed/' + relative] = path
    return {name: digest(path.read_bytes()) for name, path in paths.items()}


def simulate(xml, selections, python=None, timeout=60):
    request = {'schema': WORKER_SCHEMA, 'sbml': xml, 'sbml_sha256': digest(xml.encode()),
               **GRID, 'selections': selections, 'solver': SOLVER}
    interpreter = str(python or ROOT / '.venv-numerical/bin/python')
    env = {k: v for k, v in os.environ.items() if not k.startswith('PYTHON')}
    try:
        with tempfile.TemporaryDirectory(prefix='antimony-numerical-') as cwd:
            child = subprocess.run([interpreter, '-I', str(WORKER)], input=json.dumps(request),
                                   text=True, capture_output=True, cwd=cwd, env=env,
                                   timeout=timeout, check=False)
    except FileNotFoundError as error:
        return {'status': 'ORACLE_BLOCKED', 'reason': str(error)}
    except (OSError, subprocess.TimeoutExpired) as error:
        return {'status': 'ORACLE_ERROR', 'reason': str(error)}
    try:
        if len(child.stdout.encode()) > MAX_OUTPUT:
            raise ValueError('Oversized worker report')
        response = load_json(child.stdout)
        if not isinstance(response, dict) or response.get('schema') != WORKER_SCHEMA or response.get('status') not in ('PASS', 'ORACLE_BLOCKED', 'ORACLE_ERROR'):
            raise ValueError('Invalid worker envelope')
        if child.returncode != (0 if response['status'] == 'PASS' else 1):
            raise ValueError('Worker exit/status mismatch')
        response['process_stderr'] = child.stderr[-4000:]
        return response
    except (ValueError, TypeError) as error:
        return {'status': 'ORACLE_ERROR', 'reason': str(error), 'stderr': child.stderr[-4000:]}


def expected_tolerances(case, projection, floating_ids, values=None, anchor_contract=anchors):
    species = {x['id']: x for x in projection['species'] if not x['boundary_condition']}
    if len(floating_ids) != len(set(floating_ids)) or set(floating_ids) != set(species):
        raise ValueError('Wrong floating species identities')
    volumes = {x['id']: x['size'] for x in projection['compartments']}
    initial = values if values is not None else anchor_contract.expected_values(case['id'], 0)
    tols = [SOLVER['absolute_tolerance'] * (initial[s] if initial[s] != 0 else volumes[species[s]['compartment']]) for s in floating_ids]
    rate_vars = [r['variable'] for r in projection.get('rules', []) if r.get('kind') == 'rateRule' and r.get('variable') not in species]
    tols += [SOLVER['absolute_tolerance'] * (abs(initial[v]) if initial[v] != 0 else 1.0) for v in rate_vars]
    return tols


def validate_simulation(response, case, xml, projection, anchor_contract=anchors):
    if response.get('status') != 'PASS':
        raise ValueError('Simulation did not pass')
    if response.get('sbml_sha256') != digest(xml.encode()):
        raise ValueError('Worker simulated different SBML')
    if response.get('selections') != case['selections'] or response.get('column_names') != case['selections']:
        raise ValueError('Worker selection/column mismatch')
    if response.get('solver_readback') != SOLVER:
        raise ValueError('Effective solver settings differ from frozen request')
    metadata = response.get('metadata', {})
    if (metadata.get('packages') != {'libroadrunner': '2.10.0', 'numpy': '2.2.6'}
            or metadata.get('conserved_moiety_analysis') is not False
            or not isinstance(metadata.get('roadrunner_version'), str)
            or metadata['roadrunner_version'].split(';', 1)[0] != '2.10.0'):
        raise ValueError('Wrong runtime versions or conserved-moiety mode')
    rows = response.get('timeseries')
    if not isinstance(rows, list) or len(rows) != GRID['steps'] + 1:
        raise ValueError('Expected exactly 101 rows including both endpoints')
    for i, row in enumerate(rows):
        if not isinstance(row, list) or len(row) != len(case['selections']) or not all(finite(x) for x in row):
            raise ValueError('Invalid shape or nonfinite/bool trajectory value')
        t = GRID['start'] + (GRID['end'] - GRID['start']) * i / GRID['steps']
        if abs(row[0] - t) > 1e-12 or (i and row[0] <= rows[i-1][0]):
            raise ValueError('Wrong time grid; interpolation is forbidden')
    for key, values in [('effective_absolute_tolerance_before', None),
                        ('effective_absolute_tolerance_after', dict(zip(case['selections'], rows[-1])))]:
        # RoadRunner's getter computes from CURRENT amounts; after is a query,
        # not a claim about the integrator's internal tolerance history.
        expected = expected_tolerances(case, projection, response['floating_species_ids'], values, anchor_contract)
        observed = response[key]
        if not isinstance(observed, list) or len(observed) != len(expected) or any(not finite(a) or a <= 0 or not math.isclose(a, b, rel_tol=1e-12, abs_tol=1e-24) for a, b in zip(observed, expected)):
            raise ValueError('Effective absolute-tolerance query differs from amount/volume scaling')
    return rows


def compare(rows, selections, expected_rows):
    if (not isinstance(selections, list) or len(selections) < 2 or selections[0] != 'time'
            or not all(isinstance(s, str) for s in selections) or len(selections) != len(set(selections))):
        raise ValueError('Invalid comparison selections')
    if len(rows) != len(expected_rows) or not rows:
        raise ValueError('Empty or truncated comparison')
    failures, maximum, max_absolute = [], 0.0, 0.0
    for i, (row, expected) in enumerate(zip(rows, expected_rows)):
        if not isinstance(row, list) or len(row) != len(selections) or not all(finite(x) for x in row):
            raise ValueError('Malformed/nonfinite observed comparison row')
        for j, name in enumerate(selections[1:], 1):
            want = expected[name]
            if not finite(want):
                raise ValueError('Independent analytic value is nonfinite')
            error = abs(row[j] - want)
            bound = COMPARISON['absolute'] + COMPARISON['relative'] * abs(want)
            maximum = max(maximum, error / bound)
            max_absolute = max(max_absolute, error)
            if error > bound and len(failures) < 20:
                failures.append({'row': i, 'selection': name, 'observed': row[j], 'expected': want, 'bound': bound})
    return {'status': 'FAIL' if failures else 'PASS', 'max_scaled_error': maximum, 'max_absolute_error': max_absolute, 'failures': failures}


def check_case(case, native, python=None, reference_python=None, anchor_contract=anchors):
    record = {'id': case['id'], 'source_sha256': case['sha256'], 'status': 'FAIL',
              'stages': {s: 'NOT_RUN' for s in STAGES}}
    stages = record['stages']
    try:
        text = source(case)
        reference = reference_run('convert', text, reference_python)
        stages['reference'] = reference['status']
        if reference['status'] != 'PASS':
            record.update(status=reference['status'], reason=reference.get('reason', 'Reference conversion failed'))
            return record
        native_result = native_export(native, text)
        stages['export'] = 'PASS' if native_result['status'] == 'EXPORTED' else 'FAIL'
        if stages['export'] != 'PASS':
            record.update(reason='Native export failed', native_process=native_result)
            return record
        inspection = reference_run('inspect', native_result['xml'], reference_python)
        if inspection['status'] != 'PASS':
            stages['projection'] = inspection['status']
            record.update(status=inspection['status'], reason='Independent native SBML validation failed')
            return record
        delta = differences(reference['projection'], inspection['projection'])
        if delta:
            stages['projection'] = 'FAIL'
            record.update(reason='Native/reference structural mismatch', differences=delta)
            return record
        stages['projection'] = 'FAIL'
        for projection in (reference['projection'], inspection['projection']):
            anchor_contract.validate_preconditions(case['id'], projection)
        stages['projection'] = 'PASS'
        record.update(reference_messages=reference['messages'], native_messages=inspection['messages'],
                      reference_metadata=reference['oracle'], native_stderr=native_result['stderr'], simulations={}, comparisons={})
        trajectories = {}
        for label, xml, projection in [('native', native_result['xml'], inspection['projection']), ('reference', reference['sbml'], reference['projection'])]:
            result = simulate(xml, case['selections'], python)
            record['simulations'][label] = result
            stages[label + '_simulation'] = result['status']
            if result['status'] != 'PASS':
                record.update(status=result['status'], reason=result.get('reason', 'Simulation unavailable'))
                return record
            stages[label + '_simulation'] = 'FAIL'
            rows = validate_simulation(result, case, xml, projection, anchor_contract)
            stages[label + '_simulation'] = 'PASS'
            trajectories[label] = rows
            expected = [anchor_contract.expected_values(case['id'], GRID['start'] + (GRID['end']-GRID['start'])*i/GRID['steps']) for i in range(GRID['steps']+1)]
            comparison = compare(rows, case['selections'], expected)
            record['comparisons'][label + '_analytic'] = comparison
            stages[label + '_analytic'] = comparison['status']
        pair = compare(trajectories['native'], case['selections'], [dict(zip(case['selections'], row)) for row in trajectories['reference']])
        record['comparisons']['differential'] = pair
        stages['differential'] = pair['status']
        invariant_failures = []
        for label, rows in trajectories.items():
            for i, row in enumerate(rows):
                for name, item in anchor_contract.invariant_values(case['id'], dict(zip(case['selections'], row))).items():
                    a, b = item['observed'], item['expected']
                    if not finite(a) or not finite(b) or abs(a-b) > COMPARISON['absolute'] + COMPARISON['relative']*abs(b):
                        if len(invariant_failures) < 20:
                            invariant_failures.append({'path': label, 'row': i, 'name': name, **item})
        stages['invariants'] = 'FAIL' if invariant_failures else 'PASS'
        record['invariant_failures'] = invariant_failures
        record['status'] = 'PASS' if all(v == 'PASS' for v in stages.values()) else 'FAIL'
    except (ValueError, KeyError, TypeError, OSError, UnicodeError) as error:
        record.update(status='FAIL', reason=str(error))
    return record


def check_all(native, python=None, reference_python=None):
    manifest = load_json(MANIFEST.read_text())
    validate_manifest(manifest)
    try:
        before = fingerprints(manifest, native, python)
    except FileNotFoundError as error:
        return {'schema': SCHEMA, 'status': 'ORACLE_BLOCKED', 'reason': str(error),
                'counts': {'passed': 0, 'blocked': 8, 'failed': 0}, 'cases': []}
    cases = [check_case(case, native, python, reference_python) for case in manifest['cases']]
    after = fingerprints(manifest, native, python)
    changed = [name for name in before if before[name] != after.get(name)]
    counts = {'passed': sum(c['status'] == 'PASS' for c in cases),
              'blocked': sum(c['status'] == 'ORACLE_BLOCKED' for c in cases),
              'failed': sum(c['status'] not in ('PASS', 'ORACLE_BLOCKED') for c in cases)}
    return {'schema': SCHEMA, 'status': 'PASS' if counts['passed'] == 8 and not changed else 'FAIL',
            'counts': counts, 'cases': cases, 'input_sha256': before, 'changed_inputs': changed,
            'grid': GRID, 'comparison': COMPARISON, 'solver': SOLVER,
            'scope': 'Eight separate original analytic anchors; original ninety-case numerical statuses unchanged.'}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--native', required=True)
    parser.add_argument('--trusted-native', action='store_true')
    parser.add_argument('--python', help='Pinned numerical worker Python')
    parser.add_argument('--reference-python', help='Pinned Antimony/libSBML Python')
    args = parser.parse_args()
    if not args.trusted_native:
        parser.error('Executing the prebuilt binary requires --trusted-native')
    try:
        report = check_all(Path(args.native).resolve(), args.python, args.reference_python)
    except (OSError, ValueError, KeyError, TypeError) as error:
        report = {'schema': SCHEMA, 'status': 'FAIL', 'reason': str(error)}
    emit(report)
    return 0 if report['status'] == 'PASS' else 2


if __name__ == '__main__':
    raise SystemExit(main())
