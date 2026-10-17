import json
from pathlib import Path
import subprocess
import unittest
from unittest.mock import patch
import native_check as n
from oracle import ROOT

class NativeCheckerTests(unittest.TestCase):
    def test_structural_difference(self):
        self.assertEqual(n.differences({'rate':[1,2]}, {'rate':[1,3]})[0]['path'], '$.rate[1]')

    def test_missing_key_differs_from_none(self):
        self.assertTrue(n.differences({'value':None}, {}))

    def test_boolean_does_not_equal_number(self):
        self.assertTrue(n.differences(True, 1))

    def test_numerically_equal_scalar_representation(self):
        self.assertFalse(n.differences(3, 3.0))

    def test_unknown_diagnostic_is_not_accepted(self):
        self.assertIsNone(n.diagnostic('unsupported syntax!'))
        self.assertIsNone(n.diagnostic('stage=syntax code=imaginary message=hello'))

    def test_explicit_diagnostic(self):
        d = n.diagnostic('stage=resolve code=unsupported_feature message=no rules\n label')
        self.assertEqual(d['kind'], 'unsupported')
        self.assertEqual(d['stage'], 'resolve')

    def test_crash_is_not_rejection(self):
        with patch('subprocess.run', return_value=subprocess.CompletedProcess([], -11, '', 'crash')):
            self.assertEqual(n.native_export('/native', 'model')['status'], 'FAIL')

    def test_no_output_is_failure(self):
        with patch('subprocess.run', return_value=subprocess.CompletedProcess([], 0, '', '')):
            self.assertEqual(n.native_export('/native', 'model')['status'], 'FAIL')

    def test_rejection_with_artifact_fails(self):
        def fake(argv, **kw):
            Path(argv[2]).write_text('<sbml/>')
            return subprocess.CompletedProcess(argv, 2, '', 'stage=syntax code=syntax message=no')
        with patch('subprocess.run', side_effect=fake):
            self.assertEqual(n.native_export('/native', 'model')['status'], 'FAIL')

    def test_explicit_rejection(self):
        with patch('subprocess.run', return_value=subprocess.CompletedProcess([], 2, '', 'stage=syntax code=syntax message=no')):
            self.assertEqual(n.native_export('/native', 'model')['status'], 'REJECTED')

    def test_native_timeout_fails(self):
        with patch('subprocess.run', side_effect=subprocess.TimeoutExpired('native', 1)):
            self.assertEqual(n.native_export('/native', 'model')['status'], 'FAIL')

    def test_native_only_runs_if_reference_available(self):
        fixture = json.loads((ROOT/'fixtures/manifests/original-v1.json').read_text())['fixtures'][0]
        with patch('native_check.run', return_value={'status':'ORACLE_BLOCKED'}), patch('native_check.native_export') as native:
            record = n.check_case(fixture, '/native')
            self.assertEqual(record['status'], 'ORACLE_BLOCKED')
            self.assertTrue(all(x == 'NOT_RUN' for x in record['native_stages'].values()))
            native.assert_not_called()

    def test_initialization_cycle_rejection_is_semantic_not_malformed_syntax(self):
        fixture = next(f for f in json.loads((ROOT/'fixtures/manifests/original-v1.json').read_text())['fixtures'] if f['id']=='init_cycle')
        expected = json.loads((ROOT/fixture['expectation']).read_text())
        reference = {k:expected[k] for k in ('schema','status','source_sha256','diagnostic','messages')}
        reference['oracle'] = {'packages':expected['oracle_versions'], 'antimony_library_version':expected['antimony_library_version']}
        native = {'status':'REJECTED', 'diagnostic':{'stage':'resolve', 'kind':'semantic', 'code':'initialization_cycle', 'message':'cycle'}}
        with patch('native_check.run', return_value=reference), patch('native_check.native_export', return_value=native):
            record = n.check_case(fixture, '/native')
            self.assertEqual(record['status'], 'PASS')
            self.assertEqual(record['outcome'], 'expected_rejection')
            self.assertEqual(record['native_stages']['syntax'], 'PASS')
            self.assertEqual(record['native_stages']['resolve'], 'FAIL')
        native['diagnostic']['stage']='syntax'
        with patch('native_check.run', return_value=reference), patch('native_check.native_export', return_value=native):
            self.assertEqual(n.check_case(fixture, '/native')['status'], 'FAIL')

    def invalid_reference_case(self):
        f = next(f for f in json.loads((ROOT/'fixtures/manifests/original-v1.json').read_text())['fixtures'] if f['id']=='species_units_invalid_explicit')
        e = json.loads((ROOT/f['expectation']).read_text())
        r = {k:e[k] for k in ('schema','status','source_sha256','projection','messages')}
        r['oracle'] = {'packages':e['oracle_versions'], 'antimony_library_version':e['antimony_library_version'], 'antimony_bundled_libsbml_reported_by_xml':e['antimony_bundled_libsbml']}
        return f,r

    def test_reviewed_invalid_reference_can_only_credit_native_rejection(self):
        f,r = self.invalid_reference_case()
        native = {'status':'REJECTED','diagnostic':{'stage':'resolve','kind':'semantic','code':'units_inconsistency','message':'unsafe'}}
        with patch('native_check.run',return_value=r) as run, patch('native_check.native_export',return_value=native):
            record = n.check_case(f,'/native')
            self.assertEqual(record['status'],'PASS')
            self.assertEqual(record['reference_status'],'SBML_INVALID')
            self.assertEqual(record['outcome'],'expected_rejection')
            self.assertEqual(record['native_stages']['export'],'NOT_RUN')
            self.assertTrue(record['reference_messages'])
            self.assertEqual(run.call_count,1)

    def test_invalid_reference_cannot_validate_export(self):
        f,r = self.invalid_reference_case()
        with patch('native_check.run',return_value=r) as run, patch('native_check.native_export',return_value={'status':'EXPORTED','xml':'<sbml/>'}):
            record=n.check_case(f,'/native')
            self.assertEqual(record['status'],'FAIL')
            self.assertEqual(record['outcome'],'unexpected_acceptance')
            self.assertEqual(run.call_count,1)
        f['native_profile_expectation']='target_supported'
        with patch('native_check.run',return_value=r), patch('native_check.native_export') as native:
            self.assertEqual(n.check_case(f,'/native')['status'],'SBML_INVALID')
            native.assert_not_called()

    def test_expected_invalid_reference_does_not_hide_unavailable_oracle(self):
        f,_ = self.invalid_reference_case()
        with patch('native_check.run',return_value={'status':'ORACLE_BLOCKED'}),patch('native_check.native_export') as native:
            self.assertEqual(n.check_case(f,'/native')['status'],'ORACLE_BLOCKED')
            native.assert_not_called()

    def test_unexpected_invalid_reference_is_not_rejection_credit(self):
        f,r = self.invalid_reference_case();f['reference_status']='PASS'
        with patch('native_check.run',return_value=r),patch('native_check.native_export') as native:
            self.assertEqual(n.check_case(f,'/native')['status'],'SBML_INVALID')
            native.assert_not_called()


class NativeInputMutationTests(unittest.TestCase):
    """The manifest and executable must survive the entire evidence run unchanged."""

    def run_main(self, mutation=None):
        import sys
        import tempfile
        with tempfile.TemporaryDirectory() as cwd:
            root = Path(cwd)
            binary = root / 'native'
            binary.write_bytes(b'trusted test stand-in; never executed')
            binary.chmod(0o700)
            manifest = root / 'manifest.json'
            data = json.loads(n.MANIFEST.read_text())
            manifest.write_text(json.dumps(data))
            fixture = data['fixtures'][0]
            output = []

            def check(*args):
                if mutation:
                    mutation(manifest, binary)
                return {'schema': 'antimony-rs-compatibility-v1', 'profile_id': n.PROFILE_ID,
                        'fixture': fixture['id'], 'feature_ids': fixture['feature_ids'],
                        'source_sha256': fixture['sha256'], 'status': 'PASS',
                        'reference_status': 'PASS', 'outcome': 'supported_export',
                        'native_stages': dict.fromkeys(n.STAGES, 'NOT_RUN')}

            with patch.object(n, 'MANIFEST', manifest), patch.object(n, 'check_case', side_effect=check), \
                    patch.object(n, 'emit', side_effect=output.append), patch.object(sys, 'argv', [
                        'native_check.py', '--native', str(binary), '--trusted-native',
                        '--fixture', fixture['id']]):
                code = n.main()
            return code, output[0]

    def test_unchanged_inputs_pass_and_record_manifest(self):
        code, report = self.run_main()
        self.assertEqual(code, 0)
        self.assertEqual(report['status'], 'PASS')
        self.assertEqual(len(report['source_manifest_sha256']), 64)
        self.assertFalse(report['manifest_changed_during_check'])

    def test_manifest_mutation_fails_even_when_case_passes(self):
        code, report = self.run_main(lambda manifest, _: manifest.write_bytes(manifest.read_bytes() + b'\n'))
        self.assertEqual(code, 2)
        self.assertEqual(report['status'], 'FAIL')
        self.assertTrue(report['manifest_changed_during_check'])
        self.assertEqual(report['cases'][0]['status'], 'PASS')

    def test_deleted_manifest_fails_structurally(self):
        code, report = self.run_main(lambda manifest, _: manifest.unlink())
        self.assertEqual(code, 2)
        self.assertTrue(report['manifest_changed_during_check'])

    def test_deleted_executable_fails_structurally(self):
        code, report = self.run_main(lambda _, binary: binary.unlink())
        self.assertEqual(code, 2)
        self.assertTrue(report['native_binary_changed_during_check'])
