"""Stdlib fail-closed protocol/schema and CLI compatibility gate regressions."""
import copy
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch
import cli_check as c
from oracle import ROOT
import profile_overlay


def report(command='check', code=0):
    return {'schema': 'antimony-cli-report-v1', 'command': command,
            'status': {0: 'success', 1: 'error', 2: 'rejected'}[code], 'exit_code': code,
            'profile_id': profile_overlay.PROFILE,
            'source': {'display': 'input', 'lossy': False},
            'model': {'id': '__main', **{k: 0 for k in ('compartments', 'species', 'parameters', 'reactions', 'initial_assignments', 'unit_definitions')}} if code == 0 else None,
            'output': None if command == 'check' else {'destination': 'file', 'path': {'display': 'output', 'lossy': False}, 'written': code == 0},
            'diagnostics': [] if code == 0 else [diagnostic()]}


def diagnostic():
    return {'stage': 'syntax', 'severity': 'error', 'code': 'syntax', 'message': 'bad', 'labels': [], 'notes': []}


class CliCheckerTests(unittest.TestCase):
    def fake(self, command='check', code=0, data=None, mutate=None, stderr=b''):
        def run(argv, **kwargs):
            value = report(command, code)
            value['source']['display'] = argv[2]
            if command != 'check':
                value['output']['path']['display'] = argv[-1]
                if data is not None:
                    Path(argv[-1]).write_bytes(data)
            if mutate:
                mutate(value)
            return subprocess.CompletedProcess(argv, code, json.dumps(value).encode(), stderr)
        return run

    def test_schema_accepts_minimal_reports(self):
        for command in ('check', 'format', 'export'):
            for code in (0, 1, 2):
                with self.subTest(command=command, code=code):
                    c.validate_report(report(command, code), command, code, 'a=1;')

    def test_schema_rejects_missing_unknown_wrongtype_and_enum_fields(self):
        cases = []
        r = report(); del r['schema']; cases.append(r)
        r = report(); r['extra'] = True; cases.append(r)
        r = report(); r['exit_code'] = False; cases.append(r)
        r = report(); r['status'] = 'ok'; cases.append(r)
        r = report(); r['model']['parameters'] = -1; cases.append(r)
        r = report(); r['source']['lossy'] = 'false'; cases.append(r)
        for value in cases:
            with self.subTest(value=value), self.assertRaises(ValueError):
                c.validate_report(value, 'check', 0, 'a=1;')

    def test_protocol_status_exit_model_and_output_invariants(self):
        cases = []
        r = report(); r['command'] = 'export'; cases.append(r)
        r = report(); r['status'] = 'error'; cases.append(r)
        r = report(); r['model'] = None; cases.append(r)
        r = report(); r['source'] = None; cases.append(r)
        r = report(); r['output'] = report('export')['output']; cases.append(r)
        r = report(); r['diagnostics'] = [diagnostic()]; cases.append(r)
        r = report(); r['profile_id'] = 'full-antimony'; cases.append(r)
        for value in cases:
            with self.subTest(value=value), self.assertRaises(ValueError):
                c.validate_report(value, 'check', 0, 'a=1;')

    def test_rejection_requires_error_and_unwritten_output(self):
        r = report('format', 2); r['output']['written'] = True
        with self.assertRaises(ValueError): c.validate_report(r, 'format', 2, 'a=1;')
        r = report('format', 2); r['diagnostics'] = []
        with self.assertRaises(ValueError): c.validate_report(r, 'format', 2, 'a=1;')

    def test_locations_use_original_unicode_scalar_and_byte_coordinates(self):
        text = 'aβ😀\tz\r\nx\ry\n'
        self.assertEqual(c.position(text, 8), {'line': 1, 'column': 5, 'byte_column': 9})
        self.assertEqual(c.position(text, 11), {'line': 2, 'column': 1, 'byte_column': 1})
        self.assertEqual(c.position(text, len(text.encode())), {'line': 4, 'column': 1, 'byte_column': 1})
        for offset in (-1, 2, 4, 99):
            with self.subTest(offset=offset), self.assertRaises((ValueError, UnicodeError)):
                c.position(text, offset)

    def test_bad_or_reversed_labels_are_rejected(self):
        for start, end in [(2, 3), (3, 1), (0, 100)]:
            r = report('check', 2)
            r['diagnostics'][0]['labels'] = [{'style': 'primary', 'source_id': 0, 'start_byte': start,
                'end_byte': end, 'start': {'line': 1, 'column': 1, 'byte_column': 1},
                'end': {'line': 1, 'column': 2, 'byte_column': 2}, 'message': 'here'}]
            with self.subTest(start=start, end=end), self.assertRaises((ValueError, UnicodeError)):
                c.validate_report(r, 'check', 2, 'βa;')

    def test_success_keeps_exact_artifact_bytes(self):
        data = b'a = 1;\r\n'
        with patch('subprocess.run', side_effect=self.fake('format', data=data)):
            r = c.cli_process('/cli', 'format', 'a=1;')
            self.assertEqual(r['status'], 'SUCCESS'); self.assertEqual(r['artifact'], data)

    def test_rejection_artifact_never_earns_credit(self):
        for data in (b'', b'a=1;'):
            with patch('subprocess.run', side_effect=self.fake('format', 2, data)):
                self.assertEqual(c.cli_process('/cli', 'format', 'bad;')['status'], 'FAIL')

    def test_artifact_success_requires_nonempty_bounded_utf8(self):
        for data in (None, b'', b'\xff', b'1234'):
            with patch('cli_check.MAX_INPUT', 3), patch('subprocess.run', side_effect=self.fake('format', data=data)):
                self.assertEqual(c.cli_process('/cli', 'format', 'a=1;')['status'], 'FAIL')

    def test_json_mode_rejects_stderr_and_wrong_paths(self):
        def wrong_source(r): r['source']['display'] = 'different'
        def wrong_output(r): r['output']['path']['display'] = 'different'
        for mutate, stderr in [(wrong_source, b''), (wrong_output, b''), (None, b'warning')]:
            with patch('subprocess.run', side_effect=self.fake('format', data=b'a=1;', mutate=mutate, stderr=stderr)):
                self.assertEqual(c.cli_process('/cli', 'format', 'a=1;')['status'], 'FAIL')

    def test_crash_malformed_json_and_timeout_fail_not_reject(self):
        for result in [subprocess.CompletedProcess([], -11, b'', b''),
                       subprocess.CompletedProcess([], 0, b'{}\n{}', b''),
                       subprocess.CompletedProcess([], 0, b'\xff', b'')]:
            with patch('subprocess.run', return_value=result):
                self.assertEqual(c.cli_process('/cli', 'check', 'a=1;')['status'], 'FAIL')
        with patch('subprocess.run', side_effect=subprocess.TimeoutExpired('cli', 1)):
            self.assertEqual(c.cli_process('/cli', 'check', 'a=1;')['status'], 'FAIL')

    def test_duplicate_json_keys_fail_closed(self):
        with self.assertRaises(ValueError):
            json.loads('{"status":"error","status":"success"}', object_pairs_hook=c.unique_object)

    def test_reference_inspection_unavailable_blocks_supported_case(self):
        fixture, projection, result = self.positive_mocks()
        with patch('cli_check.native.check_case', return_value=self.baseline()), \
             patch('cli_check.cli_process', return_value=result), \
             patch('cli_check.formatting.native_format', return_value={'status': 'FORMATTED', 'bytes': b'a=1;'}), \
             patch('cli_check.native.native_export', return_value={'status': 'EXPORTED', 'xml': 'a=1;'}), \
             patch('cli_check.run', return_value={'status': 'ORACLE_BLOCKED'}):
            self.assertEqual(c.check_case(fixture, '/cli', '/export', '/format')['status'], 'ORACLE_BLOCKED')

    def test_cli_cannot_accept_native_rejection(self):
        with patch('cli_check.native.check_case', return_value=self.baseline(True)), \
             patch('cli_check.cli_process', return_value={'status': 'SUCCESS', 'report': report()}):
            self.assertEqual(c.check_case(self.fixture(), '/cli', '/export', '/format')['status'], 'FAIL')

    def test_missing_prebuilt_cli_is_blocked(self):
        with patch('subprocess.run', side_effect=FileNotFoundError('cli')):
            self.assertEqual(c.cli_process('/missing', 'check', 'a=1;')['status'], 'ORACLE_BLOCKED')

    def baseline(self, rejected=False):
        return {'status': 'PASS', 'reference_status': 'PASS',
                'outcome': 'expected_rejection' if rejected else 'supported_export',
                'native_process': {'stderr': '', 'diagnostic': {'stage': 'syntax', 'code': 'syntax'}}}

    def fixture(self):
        return json.loads((ROOT / 'fixtures/manifests/original-v1.json').read_text())['fixtures'][0]

    def test_missing_oracle_blocks_even_negative_cases_before_cli(self):
        for reference_status in ('ORACLE_BLOCKED', 'FAIL'):
            b = self.baseline(True); b['status'] = reference_status
            with patch('cli_check.native.check_case', return_value=b), patch('cli_check.cli_process') as process:
                r = c.check_case(self.fixture(), '/cli', '/export', '/format')
                self.assertEqual(r['status'], reference_status); process.assert_not_called()

    def test_expected_rejection_compares_native_stage_and_code(self):
        with patch('cli_check.native.check_case', return_value=self.baseline(True)):
            with patch('cli_check.cli_process', return_value={'status': 'REJECTED', 'report': report('check', 2)}):
                r = c.check_case(self.fixture(), '/cli', '/export', '/format')
                self.assertEqual(r['status'], 'PASS'); self.assertEqual(r['outcome'], 'expected_rejection')
            wrong = report('check', 2); wrong['diagnostics'][0]['code'] = 'unsupported_feature'
            with patch('cli_check.cli_process', return_value={'status': 'REJECTED', 'report': wrong}):
                self.assertEqual(c.check_case(self.fixture(), '/cli', '/export', '/format')['status'], 'FAIL')

    def positive_mocks(self):
        fixture = self.fixture()
        projection = json.loads((ROOT / fixture['expectation']).read_text())['projection']
        r = report(); r['model'] = {'id': projection['model']['id'], **{k: len(projection[k]) for k in ('compartments', 'species', 'parameters', 'reactions', 'initial_assignments', 'unit_definitions')}}
        result = {'status': 'SUCCESS', 'report': r, 'artifact': b'a=1;'}
        return fixture, projection, result

    def test_success_requires_example_bytes_and_independent_projection(self):
        fixture, projection, result = self.positive_mocks()
        with patch('cli_check.native.check_case', return_value=self.baseline()), \
             patch('cli_check.cli_process', return_value=result), \
             patch('cli_check.formatting.native_format', return_value={'status': 'FORMATTED', 'bytes': b'a=1;'}), \
             patch('cli_check.native.native_export', return_value={'status': 'EXPORTED', 'xml': 'a=1;'}), \
             patch('cli_check.run', return_value={'status': 'PASS', 'projection': projection}):
            r = c.check_case(fixture, '/cli', '/export', '/format')
            self.assertEqual(r['status'], 'PASS'); self.assertTrue(r['byte_idempotent'])
            self.assertEqual(r['numerical'], 'NOT_RUN')

    def test_wrong_artifacts_or_independent_semantics_fail(self):
        fixture, projection, result = self.positive_mocks()
        changed = copy.deepcopy(projection); changed['model']['id'] = 'wrong'
        for format_bytes, xml, inspected in [(b'wrong', 'a=1;', projection), (b'a=1;', 'wrong', projection), (b'a=1;', 'a=1;', changed)]:
            with patch('cli_check.native.check_case', return_value=self.baseline()), \
                 patch('cli_check.cli_process', return_value=result), \
                 patch('cli_check.formatting.native_format', return_value={'status': 'FORMATTED', 'bytes': format_bytes}), \
                 patch('cli_check.native.native_export', return_value={'status': 'EXPORTED', 'xml': xml}), \
                 patch('cli_check.run', return_value={'status': 'PASS', 'projection': inspected}):
                self.assertEqual(c.check_case(fixture, '/cli', '/export', '/format')['status'], 'FAIL')

    def test_warning_drift_and_non_idempotence_fail(self):
        fixture, projection, result = self.positive_mocks()
        b = self.baseline(); b['native_process']['stderr'] = 'stage=resolve severity=Warning code=unspecified_units message=unknown\n'
        with patch('cli_check.native.check_case', return_value=b), patch('cli_check.cli_process', return_value=result):
            self.assertEqual(c.check_case(fixture, '/cli', '/export', '/format')['status'], 'FAIL')
        second = dict(result, artifact=b'changed')
        with patch('cli_check.native.check_case', return_value=self.baseline()), \
             patch('cli_check.cli_process', side_effect=[result, result, result, second]), \
             patch('cli_check.formatting.native_format', return_value={'status': 'FORMATTED', 'bytes': b'a=1;'}):
            self.assertEqual(c.check_case(fixture, '/cli', '/export', '/format')['status'], 'FAIL')

    def test_missing_binary_main_is_blocked_not_pass(self):
        argv = ['cli_check', '--cli', '/missing', '--native', '/missing', '--formatter', '/missing', '--trusted-native']
        with patch('sys.argv', argv), patch('cli_check.emit') as emit:
            self.assertEqual(c.main(), 2)
            value = emit.call_args.args[0]
            self.assertEqual(value['status'], 'ORACLE_BLOCKED'); self.assertEqual(value['counts']['supported_cli'], 0)

    def test_trusted_acknowledgment_required(self):
        argv = ['cli_check', '--cli', '/missing', '--native', '/missing', '--formatter', '/missing']
        with patch('sys.argv', argv), self.assertRaises(SystemExit) as exited:
            c.main()
        self.assertEqual(exited.exception.code, 2)

    def test_mutated_input_fails_aggregate_gate(self):
        with tempfile.TemporaryDirectory() as cwd:
            paths = []
            for name in ('cli', 'native', 'formatter'):
                path = Path(cwd) / name; path.write_text('initial'); path.chmod(0o700); paths.append(path)
            def fake(fixture, cli, *args):
                Path(cli).write_text('changed')
                return {'fixture': fixture['id'], 'status': 'PASS', 'outcome': 'supported_cli'}
            argv = ['cli_check', '--cli', str(paths[0]), '--native', str(paths[1]), '--formatter', str(paths[2]), '--fixture', 'first_conversion', '--trusted-native']
            with patch('sys.argv', argv), patch('cli_check.check_case', side_effect=fake), patch('cli_check.emit') as emit:
                self.assertEqual(c.main(), 2)
                self.assertIn('cli', emit.call_args.args[0]['changed_inputs'])


if __name__ == '__main__':
    unittest.main()
