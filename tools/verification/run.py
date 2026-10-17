#!/usr/bin/env python3
"""Build and check the observed local source under a bounded, offline receipt.

This opt-in developer tool executes trusted Cargo/dependency code. It is neither
an OS sandbox nor a hermetic/reproducible-build attestation. No downloads or
package installs are attempted. See docs/SOURCE_EVIDENCE.md for the exact scope.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import shutil
import signal
import subprocess
import sys
import threading
import time
import tomllib

ROOT = Path(__file__).resolve().parents[2]
SCHEMA = 'antimony-current-source-receipt-v1'
SOURCE_DIRS = ('src', 'tests', 'examples', 'tools', 'fixtures', 'profiles', '.github')
SOURCE_FILES = ('Cargo.toml', 'Cargo.lock', 'build.rs', 'rust-toolchain', 'rust-toolchain.toml')
EXCLUDED_EVIDENCE = ('tools/oracle/native-evidence.json', 'tools/oracle/formatter-evidence.json',
                     'tools/oracle/cli-evidence.json', 'tools/numerical/evidence.json',
                     'tools/workloads/evidence.json', 'tools/verification/evidence.json')
ENV_ALLOWLIST = ('PATH', 'HOME', 'CARGO_HOME', 'TMPDIR', 'TMP', 'TEMP', 'SDKROOT',
                 'MACOSX_DEPLOYMENT_TARGET', 'DEVELOPER_DIR', 'SYSTEMROOT')
LOG_LIMIT = 16 * 1024 * 1024


def digest(data):
    return hashlib.sha256(data).hexdigest()


def file_hash(path):
    h = hashlib.sha256()
    with Path(path).open('rb') as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b''):
            h.update(chunk)
    return h.hexdigest()


def strict_json(text):
    def unique(pairs):
        out = {}
        for key, value in pairs:
            if key in out:
                raise ValueError('duplicate JSON key: ' + key)
            out[key] = value
        return out
    return json.loads(text, object_pairs_hook=unique,
                      parse_constant=lambda x: (_ for _ in ()).throw(ValueError('nonfinite JSON: ' + x)))


def tree_files(base):
    """Discover each tree anew, rejecting links rather than following outside it."""
    if base.is_symlink():
        raise ValueError('symlink input tree: ' + str(base))
    if not base.exists():
        return []
    result = []
    for directory, dirs, names in os.walk(base, followlinks=False):
        dirs[:] = sorted(d for d in dirs if d not in ('__pycache__', '.git', 'target'))
        for name in dirs + names:
            if (Path(directory) / name).is_symlink():
                raise ValueError('symlink input: ' + str(Path(directory) / name))
        for name in sorted(names):
            path = Path(directory) / name
            if path.suffix not in ('.pyc', '.pyo'):
                if not path.is_file():
                    raise ValueError('nonregular input: ' + str(path))
                result.append(path)
    return result


def source_snapshot(root=ROOT):
    """Includes untracked files; additions/deletions are visible on rediscovery."""
    paths = [root / name for name in SOURCE_FILES if (root / name).exists() or (root / name).is_symlink()]
    for folder in SOURCE_DIRS:
        paths.extend(tree_files(root / folder))
    result = {}
    for path in sorted(paths):
        relative = path.relative_to(root).as_posix()
        if relative not in EXCLUDED_EVIDENCE:
            if path.is_symlink() or not path.is_file():
                raise ValueError('nonregular source: ' + relative)
            result[relative] = file_hash(path)
    if 'Cargo.toml' not in result or 'Cargo.lock' not in result:
        raise ValueError('Cargo manifest and lockfile are required')
    return result


def changed(before, after):
    return sorted(key for key in before.keys() | after.keys() if before.get(key) != after.get(key))


def config_snapshot(root, env):
    """Cargo searches cwd ancestors and CARGO_HOME; include absent paths as well."""
    paths = set()
    for parent in (root, *root.parents):
        paths.update((parent / '.cargo/config', parent / '.cargo/config.toml'))
    home = Path(env.get('CARGO_HOME', str(Path(env['HOME']) / '.cargo'))).expanduser()
    paths.update((home / 'config', home / 'config.toml'))
    # Config includes may extend this scope: reject them instead of silently omitting inputs.
    out = {}
    for path in sorted(paths):
        if path.exists() or path.is_symlink():
            if path.is_symlink() or not path.is_file():
                raise ValueError('nonregular Cargo config: ' + str(path))
            data = path.read_bytes()
            config = tomllib.loads(data.decode())
            allowed = {'build': {'jobs'}, 'net': {'offline'},
                       'term': {'color', 'quiet', 'verbose'}}
            if any(section not in allowed or not isinstance(values, dict)
                   or not set(values) <= allowed[section]
                   for section, values in config.items()):
                raise ValueError('Cargo config overrides/includes/wrappers are outside receipt scope: ' + str(path))
            out[str(path)] = digest(data)
        else:
            out[str(path)] = None
    return out


def environment(target, tools):
    env = {key: os.environ[key] for key in ENV_ALLOWLIST if key in os.environ}
    env.setdefault('HOME', str(Path.home()))
    env.update(CARGO_TARGET_DIR=str(target), CARGO_NET_OFFLINE='true', CARGO_TERM_COLOR='never',
               RUSTC=tools['rustc'], RUSTDOC=tools['rustdoc'], RUSTDOCFLAGS='-D warnings',
               PYTHONDONTWRITEBYTECODE='1', LANG='C', LC_ALL='C')
    return env


def terminate(child):
    """Kill the owned POSIX process group, including Cargo's compiler children."""
    try:
        os.killpg(child.pid, signal.SIGKILL)
    except ProcessLookupError:
        pass
    child.wait()


def command(argv, cwd, env, logs, name, timeout=1200, limit=LOG_LIMIT):
    """Drain both pipes with independent hard retained-byte caps and a deadline."""
    started = time.monotonic()
    record = {'name': name, 'argv': [str(a) for a in argv], 'cwd': str(cwd),
              'timeout_seconds': timeout, 'status': 'FAIL', 'returncode': None}
    paths = {kind: logs / (name + '.' + kind + '.log') for kind in ('stdout', 'stderr')}
    try:
        child = subprocess.Popen(record['argv'], cwd=cwd, env=env, stdout=subprocess.PIPE,
                                 stderr=subprocess.PIPE, start_new_session=True)
    except OSError as error:
        record.update(status='BLOCKED', reason=str(error), duration_seconds=time.monotonic()-started)
        return record
    overflow = threading.Event()
    io_errors = []

    def drain(stream, path):
        retained = 0
        try:
            with path.open('xb') as output:
                while chunk := stream.read(65536):
                    room = max(0, limit - retained)
                    output.write(chunk[:room])
                    retained += min(room, len(chunk))
                    if len(chunk) > room:
                        overflow.set()
        except OSError as error:
            io_errors.append(str(error))
            overflow.set()
        finally:
            stream.close()

    threads = [threading.Thread(target=drain, args=(getattr(child, kind), path), daemon=True)
               for kind, path in paths.items()]
    for thread in threads:
        thread.start()
    reason = None
    while child.poll() is None:
        if overflow.is_set():
            reason = 'output_limit_or_log_io_error'
            terminate(child)
            break
        if time.monotonic() - started > timeout:
            reason = 'timeout'
            terminate(child)
            break
        time.sleep(0.02)
    for thread in threads:
        thread.join(timeout=2)
    if any(thread.is_alive() for thread in threads):
        # A descendant may keep inherited descriptors open after its parent exits.
        terminate(child)
        for thread in threads:
            thread.join(timeout=2)
        reason = 'descendant_kept_output_open'
    if overflow.is_set():
        reason = reason or 'output_limit_or_log_io_error'
    record.update(returncode=child.returncode, duration_seconds=time.monotonic()-started,
                  status='PASS' if child.returncode == 0 and reason is None else 'FAIL',
                  logs={kind: {'path': str(path), 'sha256': file_hash(path), 'bytes': path.stat().st_size}
                        for kind, path in paths.items() if path.exists()})
    if reason:
        record['reason'] = reason
    if io_errors:
        record['log_errors'] = io_errors
    return record


def dependencies(metadata, root):
    """Record actual extracted registry trees plus lockfile package checksums."""
    lock = tomllib.loads((root / 'Cargo.lock').read_text())
    locked = {(p['name'], p['version'], p.get('source')): p.get('checksum') for p in lock['package']}
    out = {}
    for package in metadata['packages']:
        if package['source'] is None:
            if Path(package['manifest_path']).resolve() != root / 'Cargo.toml':
                raise ValueError('external path dependency is outside receipt scope')
            continue
        key = (package['name'], package['version'], package['source'])
        checksum = locked.get(key)
        if not checksum or not package['source'].startswith('registry+'):
            raise ValueError('unlocked/non-registry dependency: ' + package['name'])
        base = Path(package['manifest_path']).parent
        files = {p.relative_to(base).as_posix(): file_hash(p) for p in tree_files(base)}
        out[package['id']] = {'name': package['name'], 'version': package['version'],
                             'source': package['source'], 'lock_checksum': checksum,
                             'directory': str(base), 'files': files,
                             'tree_sha256': digest(json.dumps(files, sort_keys=True).encode())}
    return out


def toolchain_snapshot(base):
    """Homebrew toolchains contain executable file links; bind link and target bytes.

    Directory links and missing targets remain unsupported. This is not a recursive
    discovery of the operating system dynamic-library closure.
    """
    out = {}
    for directory, dirs, names in os.walk(base, followlinks=False):
        for name in dirs:
            if (Path(directory)/name).is_symlink():
                raise ValueError('toolchain directory symlink outside scope')
        for name in names:
            path = Path(directory)/name
            if not path.is_file():
                raise ValueError('missing/nonregular toolchain input: ' + str(path))
            out[path.relative_to(base).as_posix()] = {
                'sha256':file_hash(path), 'resolved_path':str(path.resolve()),
                'symlink':os.readlink(path) if path.is_symlink() else None}
    return out


def native_plan(cargo, target):
    """Complete four-feature local gates; the final artifact build restores SBML CLI."""
    common = ['--offline', '--locked', '--target-dir', str(target)]
    plan = [('fmt', [cargo, 'fmt', '--all', '--check'])]
    modes = [('all', ['--all-features']), ('default', []),
             ('core', ['--no-default-features']), ('cli_only', ['--no-default-features', '--features', 'cli'])]
    for mode, flags in modes:
        for verb in ('check', 'build', 'test', 'clippy'):
            args = [cargo, verb, *common, '--all-targets', *flags]
            if verb == 'clippy':
                args.extend(['--', '-D', 'warnings'])
            plan.append((mode + '_' + verb, args))
        plan.append((mode + '_doctest', [cargo, 'test', *common, '--doc', *flags]))
        plan.append((mode + '_rustdoc', [cargo, 'doc', *common, '--no-deps', *flags]))
    plan.append(('final_artifacts', [cargo, 'build', *common, '--all-features', '--bin', 'antimony',
                                     '--examples', '--message-format=json']))
    return plan


def artifact_paths(text, target):
    found = {}
    finished = False
    required = {'antimony': 'bin', 'export_sbml': 'example', 'format_antimony': 'example'}
    for line in text.splitlines():
        item = strict_json(line)
        if item.get('reason') == 'build-finished':
            finished = item.get('success') is True
        if item.get('reason') != 'compiler-artifact' or not item.get('executable'):
            continue
        name = item['target']['name']
        if name not in required or required[name] not in item['target']['kind']:
            continue
        if not {'cli', 'sbml'} <= set(item.get('features', [])):
            raise ValueError('final executable lacks required features')
        path = Path(item['executable']).resolve()
        if not path.is_relative_to(target.resolve()) or not path.is_file() or not os.access(path, os.X_OK):
            raise ValueError('invalid or missing Cargo executable artifact')
        found[name] = {'path': str(path), 'sha256': file_hash(path), 'cargo_fresh': item.get('fresh')}
    if not finished or set(found) != set(required):
        raise ValueError('final Cargo build did not report every required executable')
    return found


def checker_plan(root, binaries, oracle, numerical):
    exporter = binaries['export_sbml']['path']
    formatter = binaries['format_antimony']['path']
    cli = binaries['antimony']['path']
    trusted = ['--native', exporter, '--trusted-native']
    return [
        ('native_corpus', [oracle, str(root/'tools/oracle/native_check.py'), *trusted, '--python', oracle],
         'antimony-rs-native-check-v1', 90),
        ('roundtrip', [oracle, str(root/'tools/oracle/roundtrip_check.py'), *trusted, '--formatter', formatter, '--python', oracle],
         'antimony-rs-roundtrip-check-v1', 90),
        ('cli', [oracle, str(root/'tools/oracle/cli_check.py'), *trusted, '--formatter', formatter, '--cli', cli, '--python', oracle],
         'antimony-rs-cli-check-v1', 90),
        ('numerical', [oracle, str(root/'tools/numerical/check.py'), *trusted, '--python', numerical, '--reference-python', oracle],
         'antimony-numerical-check-v1', 8),
        ('workloads', [oracle, str(root/'tools/workloads/check.py'), *trusted, '--formatter', formatter, '--python', oracle],
         'antimony-workload-check-v1', 86),
    ]


def expected_inventory(root, schema):
    """Read expected identities/claims from fingerprinted inputs, never checker output."""
    if schema == 'antimony-numerical-check-v1':
        cases = strict_json((root/'fixtures/numerical/manifest.json').read_text())['cases']
    elif schema == 'antimony-workload-check-v1':
        cases = strict_json((root/'fixtures/workloads/manifest.json').read_text())['cases'] + strict_json(
            (root/'tools/workloads/generated-v1.json').read_text())['cases']
    else:
        cases = strict_json((root/'fixtures/manifests/original-v1.json').read_text())['fixtures']
    result = {case['id']: case for case in cases}
    if len(result) != len(cases):
        raise ValueError('duplicate expected inventory identity')
    return result


def require(condition, message):
    if not condition:
        raise ValueError(message)


def rejection_matches(expected, diagnostic):
    stage, kind = diagnostic.get('stage'), diagnostic.get('kind')
    return (stage in ('syntax', 'resolve', 'export') and (
        expected == 'semantic_rejected' and stage == 'resolve' and kind == 'semantic'
        or expected == 'malformed' and stage == 'syntax' and kind == 'syntax'
        or expected == 'unsupported_profile' and kind == 'unsupported'
        or expected == 'strict_profile_rejected' and stage == 'resolve' and kind in ('semantic', 'unsupported')))


def validate_case(case, expected, schema):
    """Validate receipt-stage assertions, not duplicate scientific comparison code."""
    require(case.get('source_sha256') == expected.get('sha256', expected.get('source_sha256')), 'case source identity differs')
    if schema == 'antimony-numerical-check-v1':
        stages = ('reference', 'export', 'projection', 'native_simulation', 'reference_simulation',
                  'native_analytic', 'reference_analytic', 'differential', 'invariants')
        require(case.get('stages') == dict.fromkeys(stages, 'PASS'), 'numerical substages incomplete')
        require(case.get('invariant_failures') == [], 'numerical invariant failure')
        comparisons = case.get('comparisons', {})
        require(set(comparisons) == {'native_analytic', 'reference_analytic', 'differential'}, 'numerical comparisons missing')
        for comparison in comparisons.values():
            require(comparison.get('status') == 'PASS' and comparison.get('failures') == [], 'numerical comparison failed')
        require(set(case.get('simulations', {})) == {'native', 'reference'}, 'numerical paths missing')
        for simulation in case['simulations'].values():
            require(simulation.get('status') == 'PASS' and simulation.get('selections') == expected['selections']
                    and simulation.get('column_names') == expected['selections']
                    and len(simulation.get('timeseries', [])) == 101, 'numerical simulation incomplete')
        return
    if schema == 'antimony-workload-check-v1':
        negative = expected['kind'] == 'generated_negative'
        stages = dict.fromkeys(('reference', 'reference_contract', 'native_export', 'native_contract',
            'format', 'format_idempotence', 'formatted_reference', 'formatted_reference_contract',
            'formatted_native', 'formatted_native_contract'), 'PASS')
        if negative:
            stages.update(native_export='EXPECTED_REJECTION', format='EXPECTED_REJECTION',
                **dict.fromkeys(('format_idempotence', 'formatted_reference', 'formatted_reference_contract',
                                 'formatted_native', 'formatted_native_contract'), 'NOT_RUN'))
        outcome = 'expected_rejection' if negative else ('supported_motif' if expected['kind'] == 'motif' else 'supported_generated')
        require(case.get('stages') == stages and case.get('outcome') == outcome
                and case.get('kind') == expected['kind'] and case.get('numerical') == 'NOT_RUN', 'workload stages/outcome differ')
        require(case.get('native_status') == ('REJECTED' if negative else 'EXPORTED'), 'workload native status differs')
        reference = 'REFERENCE_REJECTED' if negative and expected['recipe']['family'] == 'cycle' else 'PASS'
        require(case.get('reference_status') == reference, 'workload reference status differs')
        if negative:
            pair = {'cycle':('resolve','initialization_cycle'), 'dimensions':('resolve','units_inconsistency'),
                    'unset':('resolve','unsupported_feature'), 'envelope':('syntax','unsupported_feature')}[expected['recipe']['family']]
            for key in ('native_diagnostic', 'formatter_diagnostic'):
                diagnostic = case.get(key, {})
                require((diagnostic.get('stage'), diagnostic.get('code')) == pair, 'workload rejection diagnostic differs')
        return
    supported = expected['native_profile_expectation'] == 'target_supported'
    require(case.get('reference_status') == expected['reference_status'], 'reference outcome differs from manifest')
    outcome_key = 'roundtrip_outcome' if schema == 'antimony-rs-roundtrip-check-v1' else 'outcome'
    success_outcome = {'antimony-rs-native-check-v1':'supported_export',
                       'antimony-rs-roundtrip-check-v1':'supported_roundtrip',
                       'antimony-rs-cli-check-v1':'supported_cli'}[schema]
    require(case.get(outcome_key) == (success_outcome if supported else 'expected_rejection'), 'case outcome differs')
    if schema == 'antimony-rs-cli-check-v1':
        require(case.get('numerical') == 'NOT_RUN', 'CLI numerical claim changed')
        commands = case.get('commands', {})
        require(set(commands) == {'check', 'format', 'export'}, 'CLI command inventory differs')
        for command in commands.values():
            require(command.get('status') == ('SUCCESS' if supported else 'REJECTED'), 'CLI command failed')
            body = command.get('report', {})
            require(body.get('status') == ('success' if supported else 'rejected')
                    and body.get('exit_code') == (0 if supported else 2), 'CLI report disagrees')
            if not supported:
                kinds = {'syntax':'syntax', 'unresolved_symbol':'semantic', 'declaration_conflict':'semantic',
                         'unsupported_feature':'unsupported', 'units_inconsistency':'semantic', 'initialization_cycle':'semantic'}
                errors = [dict(diagnostic, kind=kinds.get(diagnostic.get('code')))
                          for diagnostic in body.get('diagnostics', []) if diagnostic.get('severity') == 'error']
                require(any(rejection_matches(expected['native_profile_expectation'], diagnostic) for diagnostic in errors),
                        'CLI rejection diagnostic differs from expected family')
        if supported:
            require(case.get('byte_idempotent') is True and case.get('export_status') == 'PASS'
                    and case.get('formatted_reference_status') == 'PASS', 'CLI export/roundtrip incomplete')
        return
    stages = {'syntax':'PASS', 'resolve':'PASS', 'export':'PASS', 'numerical':'NOT_RUN'}
    process = case.get('native_process', {})
    if supported:
        require(process.get('status') == 'EXPORTED' and case.get('sbml_inspection_status') == 'PASS'
                and case.get('differences') == [], 'native export/inspection mismatch')
    else:
        diagnostic = process.get('diagnostic', {})
        require(process.get('status') == 'REJECTED' and rejection_matches(expected['native_profile_expectation'], diagnostic),
                'native rejection contract mismatch')
        stages = dict.fromkeys(stages, 'NOT_RUN')
        stage = diagnostic['stage']
        if stage in ('resolve', 'export'): stages['syntax'] = 'PASS'
        if stage == 'export': stages['resolve'] = 'PASS'
        stages[stage] = 'UNSUPPORTED_PROFILE' if diagnostic['kind'] == 'unsupported' else 'FAIL'
    require(case.get('native_stages') == stages, 'native stage claims disagree')
    if schema == 'antimony-rs-roundtrip-check-v1':
        require(case.get('original_native_outcome') == ('supported_export' if supported else 'expected_rejection'), 'roundtrip baseline missing')
        if supported:
            require(case.get('format_status') == 'FORMATTED' and case.get('idempotence') == 'PASS'
                    and case.get('formatted_reference_status') == 'PASS' and case.get('formatted_native_status') == 'PASS'
                    and case.get('formatted_reference_differences') == [] and case.get('formatted_native_differences') == [],
                    'roundtrip substages incomplete')
        else:
            require(case.get('format_status') == 'REJECTED'
                    and rejection_matches(expected['native_profile_expectation'], case.get('formatter_process', {}).get('diagnostic', {})),
                    'formatter rejection differs')
            require(all(case.get(k) == 'NOT_RUN' for k in ('idempotence', 'formatted_reference_status', 'formatted_native_status')),
                    'rejected roundtrip claims unexecuted stages')


def validate_checker(report, schema, count, inventory):
    """Bind every case, outcome and required substage to the frozen expected inventory."""
    require(isinstance(report, dict) and report.get('schema') == schema and report.get('status') == 'PASS',
            'checker did not return expected PASS envelope')
    cases = report.get('cases')
    require(isinstance(cases, list) and len(cases) == count and len(inventory) == count
            and all(isinstance(case, dict) and case.get('status') == 'PASS' for case in cases), 'case count/status mismatch')
    ids = [case.get('fixture', case.get('id')) for case in cases]
    require(all(isinstance(value, str) for value in ids) and len(set(ids)) == count and set(ids) == set(inventory),
            'checker identities differ from frozen inventory')
    for identity, case in zip(ids, cases):
        validate_case(case, inventory[identity], schema)
    if schema == 'antimony-numerical-check-v1':
        counts = {'passed':count, 'blocked':0, 'failed':0}
    elif schema == 'antimony-workload-check-v1':
        counts = {'motifs':sum(c['kind'] == 'motif' for c in inventory.values()),
                  'generated_supported':sum(c['kind'] == 'generated_positive' for c in inventory.values()),
                  'expected_rejections':sum(c['kind'] == 'generated_negative' for c in inventory.values()),
                  'failed_or_blocked':0, 'numerical_not_run':count}
    else:
        positive = sum(c['native_profile_expectation'] == 'target_supported' for c in inventory.values())
        key = {'antimony-rs-native-check-v1':'supported_exports', 'antimony-rs-roundtrip-check-v1':'supported_roundtrips',
               'antimony-rs-cli-check-v1':'supported_cli'}[schema]
        counts = {key:positive, 'expected_rejections':count-positive, 'failed_or_blocked':0, 'numerical_not_run':count}
        if schema == 'antimony-rs-roundtrip-check-v1': counts['byte_idempotent'] = positive
    require(report.get('counts') == counts and all(type(v) is int for v in report['counts'].values()), 'checker counts differ')
    require(not (report.get('changed_inputs') or report.get('native_binary_changed_during_check')
                 or report.get('manifest_changed_during_check')), 'checker reports mutated inputs')
    return {'schema':schema, 'status':'PASS', 'case_count':len(cases), 'counts':counts}


def run(root, target, output, oracle, numerical, timeout=1200):
    """Run only after every contributing source/fixture owner has frozen changes."""
    receipt = {'schema': SCHEMA, 'status': 'FAIL', 'scope': 'observed_current_source_local_build_not_reproducibility',
               'commands': [], 'artifacts': {}, 'source_scope': {'directories': SOURCE_DIRS,
               'root_files': SOURCE_FILES, 'excluded_evidence': EXCLUDED_EVIDENCE},
               'root': str(root), 'target_directory': str(target), 'host': platform.platform()}
    logs = output / 'logs'
    logs.mkdir()
    before = None
    deps_before = None
    metadata = None
    configs_before = None
    tools = {}
    tool_hashes = {}
    env = {}
    external = {}
    sysroot_before = None
    sysroot_lib = None
    receipt['planned_commands'] = ['cargo_version', 'rustc_version', 'rustc_sysroot', 'cargo_metadata'] + [
        name for name, _ in native_plan('cargo', target)] + [
        'oracle_unit', 'numerical_unit', 'workloads_unit', 'verification_unit',
        'native_corpus', 'roundtrip', 'cli', 'numerical', 'workloads']
    try:
        if os.name != 'posix':
            raise ValueError('this bounded process-group runner currently requires POSIX')
        before = source_snapshot(root)
        receipt['source_before'] = before
        for name in ('cargo', 'rustc', 'rustdoc', 'rustfmt', 'cargo-fmt', 'cargo-clippy', 'clippy-driver'):
            path = shutil.which(name)
            if not path:
                raise FileNotFoundError('required tool missing: ' + name)
            tools[name] = str(Path(path).resolve())
        tool_hashes = {name: file_hash(path) for name, path in tools.items()}
        receipt['tools'] = {name: {'path': path, 'sha256': tool_hashes[name]} for name, path in tools.items()}
        env = environment(target, tools)
        receipt['environment'] = env
        receipt['environment_policy'] = 'allowlist plus explicit overrides; other inherited variables removed'
        configs_before = config_snapshot(root, env)
        receipt['cargo_configs_before'] = configs_before
        for label, interpreter in [('oracle', oracle), ('numerical', numerical)]:
            path = Path(interpreter)
            if not path.is_file():
                raise FileNotFoundError('required interpreter missing: ' + interpreter)
            external[label + '_python'] = file_hash(path)
        receipt['interpreter_before'] = external

        def execute(name, argv):
            result = command(argv, root, env, logs, name, timeout)
            receipt['commands'].append(result)
            if result['status'] != 'PASS':
                raise RuntimeError('command did not pass: ' + name)
            return Path(result['logs']['stdout']['path']).read_text()

        receipt['cargo_version'] = execute('cargo_version', [tools['cargo'], '--version', '--verbose'])
        receipt['rustc_version'] = execute('rustc_version', [tools['rustc'], '--version', '--verbose'])
        receipt['rustc_sysroot'] = execute('rustc_sysroot', [tools['rustc'], '--print', 'sysroot']).strip()
        sysroot_lib = Path(receipt['rustc_sysroot']) / 'lib'
        sysroot_before = toolchain_snapshot(sysroot_lib)
        if not sysroot_before:
            raise ValueError('compiler sysroot libraries missing')
        receipt['sysroot_libraries_before'] = sysroot_before
        metadata = strict_json(execute('cargo_metadata', [tools['cargo'], 'metadata', '--offline', '--locked',
                                                               '--all-features', '--format-version', '1']))
        deps_before = dependencies(metadata, root)
        receipt['dependencies_before'] = deps_before
        for name, argv in native_plan(tools['cargo'], target):
            text = execute(name, argv)
            if name == 'final_artifacts':
                receipt['artifacts'] = artifact_paths(text, target)
        for name, interpreter, folder in [('oracle_unit', oracle, 'tools/oracle'),
                                          ('numerical_unit', numerical, 'tools/numerical'),
                                          ('workloads_unit', oracle, 'tools/workloads'),
                                          ('verification_unit', oracle, 'tools/verification')]:
            execute(name, [interpreter, '-m', 'unittest', 'discover', '-s', str(root/folder), '-p', 'test_*.py', '-v'])
            stderr = Path(receipt['commands'][-1]['logs']['stderr']['path']).read_text()
            match = re.search(r'Ran (\d+) tests? in', stderr)
            if not match or int(match.group(1)) == 0 or 'skipped=' in stderr or 'expected failures=' in stderr:
                raise ValueError('unit suite empty, skipped, or incomplete: ' + name)
            receipt['commands'][-1]['unit_test_count'] = int(match.group(1))
        receipt['checker_results'] = {}
        for name, argv, schema, count in checker_plan(root, receipt['artifacts'], oracle, numerical):
            inventory = expected_inventory(root, schema)
            text = execute(name, argv)
            receipt['checker_results'][name] = validate_checker(strict_json(text), schema, count, inventory)
        receipt['status'] = 'PASS'
    except (OSError, ValueError, KeyError, TypeError, RuntimeError) as error:
        receipt['reason'] = f'{type(error).__name__}: {error}'
        if isinstance(error, FileNotFoundError):
            receipt['status'] = 'BLOCKED'
    finally:
        try:
            if before is not None:
                after = source_snapshot(root)
                receipt['source_after'] = after
                receipt['changed_source_inputs'] = changed(before, after)
            if deps_before is not None:
                deps_after = dependencies(metadata, root)
                receipt['dependencies_after'] = deps_after
                receipt['changed_dependencies'] = changed(deps_before, deps_after)
            if configs_before is not None:
                configs_after = config_snapshot(root, env)
                receipt['cargo_configs_after'] = configs_after
                receipt['changed_cargo_configs'] = changed(configs_before, configs_after)
            if sysroot_before is not None:
                sysroot_after = toolchain_snapshot(sysroot_lib)
                receipt['sysroot_libraries_after'] = sysroot_after
                receipt['changed_sysroot_libraries'] = changed(sysroot_before, sysroot_after)
            receipt['changed_tool_resolution'] = [name for name, path in tools.items()
                    if shutil.which(name) is None or str(Path(shutil.which(name)).resolve()) != path]
            receipt['changed_tools'] = [name for name, path in tools.items()
                                        if name in tool_hashes and (not Path(path).is_file() or file_hash(path) != tool_hashes[name])]
            receipt['changed_interpreters'] = [label + '_python' for label, path in [('oracle', oracle), ('numerical', numerical)]
                    if label + '_python' in external and (not Path(path).is_file() or file_hash(path) != external[label + '_python'])]
            receipt['changed_artifacts'] = [name for name, item in receipt['artifacts'].items()
                    if not Path(item['path']).is_file() or file_hash(item['path']) != item['sha256']]
            if any(receipt.get(key) for key in ('changed_source_inputs', 'changed_dependencies',
                    'changed_cargo_configs', 'changed_tools', 'changed_tool_resolution',
                    'changed_sysroot_libraries', 'changed_interpreters', 'changed_artifacts')):
                receipt['status'] = 'FAIL'
                receipt['reason'] = 'inputs or built artifacts changed during receipt'
        except (OSError, ValueError, KeyError, TypeError) as error:
            receipt.update(status='FAIL', final_fingerprint_error=str(error))
        completed = {item['name'] for item in receipt['commands']}
        receipt['not_run_commands'] = [name for name in receipt['planned_commands'] if name not in completed]
    return receipt


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--execute', action='store_true', help='authorize trusted local builds and validation')
    parser.add_argument('--target-dir', type=Path, default=ROOT/'target/independent-cli-review')
    parser.add_argument('--output-dir', type=Path, required=True, help='new directory beneath ignored artifacts/')
    parser.add_argument('--oracle-python', type=Path, default=ROOT/'.venv-oracle/bin/python')
    parser.add_argument('--numerical-python', type=Path, default=ROOT/'.venv-numerical/bin/python')
    parser.add_argument('--timeout', type=int, default=1200)
    args = parser.parse_args()
    if not args.execute:
        parser.error('--execute required; no implicit builds')
    target, output = args.target_dir.resolve(), args.output_dir.resolve()
    if not target.is_relative_to(ROOT/'target') or target in (ROOT/'target', ROOT/'target/debug'):
        parser.error('choose an isolated directory below target/, not the standard target')
    if not output.is_relative_to(ROOT/'artifacts') or output == ROOT/'artifacts':
        parser.error('output must be a new child directory of ignored artifacts/')
    if not 1 <= args.timeout <= 3600:
        parser.error('timeout must be 1..3600 seconds per command')
    try:
        output.mkdir(parents=True, exist_ok=False)
        report = run(ROOT, target, output, str(args.oracle_python.absolute()),
                     str(args.numerical_python.absolute()), args.timeout)
        (output/'receipt.json').write_text(json.dumps(report, indent=2, sort_keys=True, allow_nan=False)+'\n')
        print(json.dumps({'schema': SCHEMA, 'status': report['status'], 'receipt': str(output/'receipt.json')}))
        return 0 if report['status'] == 'PASS' else 2
    except (OSError, ValueError) as error:
        print(json.dumps({'schema': SCHEMA, 'status': 'FAIL', 'reason': str(error)}))
        return 2


if __name__ == '__main__':
    raise SystemExit(main())
