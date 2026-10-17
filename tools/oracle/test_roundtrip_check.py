"""Fail-closed formatter process and roundtrip checker tests (stdlib only)."""
import copy
import json
from pathlib import Path
import subprocess
import unittest
from unittest.mock import patch
import roundtrip_check as r
from oracle import ROOT

class RoundtripCheckerTests(unittest.TestCase):
    def setUp(self):
        self.fixture=json.loads((ROOT/'fixtures/manifests/original-v1.json').read_text())['fixtures'][0]
        self.projection=json.loads((ROOT/self.fixture['expectation']).read_text())['projection']
        self.baseline={'status':'PASS','outcome':'supported_export','reference_status':'PASS',
                       'native_stages':dict(syntax='PASS',resolve='PASS',export='PASS',numerical='NOT_RUN')}
        self.formatted={'status':'FORMATTED','text':'p = 2;\n','bytes':b'p = 2;\n','formatted_sha256':'a'*64}
    def process(self,code=0,data=b'p=2;',stderr=''):
        def fake(argv,**kw):
            if data is not None: Path(argv[2]).write_bytes(data)
            return subprocess.CompletedProcess(argv,code,'',stderr)
        return fake
    def test_success_preserves_exact_bytes(self):
        with patch('subprocess.run',side_effect=self.process(data=b'p=2;\r\n')):
            out=r.native_format('/formatter','p=2;')
            self.assertEqual(out['status'],'FORMATTED');self.assertEqual(out['bytes'],b'p=2;\r\n')
    def test_rejection_requires_no_artifact(self):
        for data in (b'',b'p=2;'):
            with patch('subprocess.run',side_effect=self.process(2,data,'stage=syntax code=syntax message=bad')):
                self.assertEqual(r.native_format('/formatter','bad')['status'],'FAIL')
    def test_rejection_requires_classified_diagnostic(self):
        with patch('subprocess.run',side_effect=self.process(2,None,'bad source')):
            self.assertEqual(r.native_format('/formatter','bad')['status'],'FAIL')
        with patch('subprocess.run',side_effect=self.process(2,None,'stage=resolve code=units_inconsistency message=bad')):
            self.assertEqual(r.native_format('/formatter','bad')['status'],'REJECTED')
    def test_crash_is_not_rejection(self):
        with patch('subprocess.run',side_effect=self.process(-11,None)):
            self.assertEqual(r.native_format('/formatter','bad')['status'],'FAIL')
    def test_empty_and_missing_output_fail(self):
        for data in (b'',None):
            with patch('subprocess.run',side_effect=self.process(data=data)):
                self.assertEqual(r.native_format('/formatter','p=2;')['status'],'FAIL')
    def test_non_utf8_and_import_outputs_fail(self):
        for data in (b'\xff',b'import "elsewhere.ant";'):
            with patch('subprocess.run',side_effect=self.process(data=data)):
                self.assertEqual(r.native_format('/formatter','p=2;')['status'],'FAIL')
    def test_output_size_limit(self):
        with patch('roundtrip_check.MAX_INPUT',3),patch('subprocess.run',side_effect=self.process(data=b'1234')):
            self.assertEqual(r.native_format('/formatter','p=2;')['status'],'FAIL')
    def test_timeout_fails(self):
        with patch('subprocess.run',side_effect=subprocess.TimeoutExpired('format',1)):
            self.assertEqual(r.native_format('/formatter','p=2;')['status'],'FAIL')
    def test_missing_original_oracle_prevents_formatter(self):
        baseline=dict(self.baseline,status='ORACLE_BLOCKED')
        with patch('roundtrip_check.native.check_case',return_value=baseline),patch('roundtrip_check.native_format') as fmt:
            out=r.check_case(self.fixture,'/formatter','/exporter')
            self.assertEqual(out['status'],'ORACLE_BLOCKED');fmt.assert_not_called()
    def test_non_idempotent_output_fails_before_reference(self):
        second=dict(self.formatted,bytes=b'p=2;')
        with patch('roundtrip_check.native.check_case',return_value=self.baseline),patch('roundtrip_check.native_format',side_effect=[self.formatted,second]),patch('roundtrip_check.run') as oracle:
            out=r.check_case(self.fixture,'/formatter','/exporter')
            self.assertEqual(out['idempotence'],'FAIL');oracle.assert_not_called()
    def test_changed_formatted_reference_semantics_fails(self):
        changed=copy.deepcopy(self.projection);changed['id']='different'
        with patch('roundtrip_check.native.check_case',return_value=self.baseline),patch('roundtrip_check.native_format',return_value=self.formatted),patch('roundtrip_check.run',return_value={'status':'PASS','projection':changed}),patch('roundtrip_check.native.native_export') as exporter:
            out=r.check_case(self.fixture,'/formatter','/exporter')
            self.assertEqual(out['status'],'FAIL');self.assertTrue(out['formatted_reference_differences']);exporter.assert_not_called()
    def test_changed_native_formatted_semantics_fails(self):
        changed=copy.deepcopy(self.projection);changed['id']='different'
        refs=[{'status':'PASS','projection':self.projection},{'status':'PASS','projection':changed}]
        with patch('roundtrip_check.native.check_case',return_value=self.baseline),patch('roundtrip_check.native_format',return_value=self.formatted),patch('roundtrip_check.run',side_effect=refs),patch('roundtrip_check.native.native_export',return_value={'status':'EXPORTED','xml':'xml'}):
            out=r.check_case(self.fixture,'/formatter','/exporter')
            self.assertEqual(out['status'],'FAIL');self.assertTrue(out['formatted_native_differences'])
    def test_both_semantic_paths_and_idempotence_required(self):
        with patch('roundtrip_check.native.check_case',return_value=self.baseline),patch('roundtrip_check.native_format',return_value=self.formatted),patch('roundtrip_check.run',return_value={'status':'PASS','projection':self.projection}),patch('roundtrip_check.native.native_export',return_value={'status':'EXPORTED','xml':'xml'}):
            out=r.check_case(self.fixture,'/formatter','/exporter')
            self.assertEqual(out['status'],'PASS');self.assertEqual(out['roundtrip_outcome'],'supported_roundtrip');self.assertEqual(out['idempotence'],'PASS');self.assertEqual(out['native_stages']['numerical'],'NOT_RUN')
    def test_invalid_reference_negative_can_only_reject(self):
        f=dict(self.fixture,reference_status='SBML_INVALID',native_profile_expectation='strict_profile_rejected')
        rejection={'status':'REJECTED','diagnostic':{'stage':'resolve','kind':'semantic','code':'units_inconsistency'}}
        baseline=dict(self.baseline,reference_status='SBML_INVALID',outcome='expected_rejection')
        with patch('roundtrip_check.native.check_case',return_value=baseline),patch('roundtrip_check.native_format',return_value=rejection),patch('roundtrip_check.run') as oracle:
            out=r.check_case(f,'/formatter','/exporter');self.assertEqual(out['status'],'PASS');self.assertEqual(out['roundtrip_outcome'],'expected_rejection');self.assertEqual(out['idempotence'],'NOT_RUN');oracle.assert_not_called()
        with patch('roundtrip_check.native.check_case',return_value=baseline),patch('roundtrip_check.native_format',return_value=self.formatted),patch('roundtrip_check.run') as oracle:
            out=r.check_case(f,'/formatter','/exporter');self.assertEqual(out['status'],'FAIL');self.assertEqual(out['roundtrip_outcome'],'unexpected_acceptance');oracle.assert_not_called()
    def test_formatted_oracle_unavailability_is_blocked(self):
        with patch('roundtrip_check.native.check_case',return_value=self.baseline),patch('roundtrip_check.native_format',return_value=self.formatted),patch('roundtrip_check.run',return_value={'status':'ORACLE_BLOCKED'}):
            self.assertEqual(r.check_case(self.fixture,'/formatter','/exporter')['status'],'ORACLE_BLOCKED')
    def test_unsupported_formatted_reference_never_compares_projection(self):
        with patch('roundtrip_check.native.check_case',return_value=self.baseline),patch('roundtrip_check.native_format',return_value=self.formatted),patch('roundtrip_check.run',return_value={'status':'PROJECTION_UNSUPPORTED'}),patch('roundtrip_check.native.native_export') as exporter:
            self.assertEqual(r.check_case(self.fixture,'/formatter','/exporter')['status'],'FAIL');exporter.assert_not_called()
    def test_binary_or_manifest_mutation_fails_aggregate_gate(self):
        import tempfile
        for mutate in ('formatter','exporter','manifest'):
            with self.subTest(mutate=mutate),tempfile.TemporaryDirectory() as cwd:
                paths={key:Path(cwd)/key for key in ('formatter','exporter','manifest')}
                for key in ('formatter','exporter'):
                    paths[key].write_text('executable');paths[key].chmod(0o700)
                paths['manifest'].write_bytes((ROOT/'fixtures/manifests/original-v1.json').read_bytes())
                def case(*args):
                    with paths[mutate].open('a') as f: f.write(' ')
                    return {'status':'PASS','roundtrip_outcome':'supported_roundtrip','idempotence':'PASS'}
                argv=['check','--formatter',str(paths['formatter']),'--native',str(paths['exporter']),'--trusted-native','--fixture',self.fixture['id']]
                with patch('sys.argv',argv),patch('roundtrip_check.MANIFEST',paths['manifest']),patch('roundtrip_check.check_case',side_effect=case),patch('roundtrip_check.emit') as emit:
                    self.assertEqual(r.main(),2)
                    self.assertEqual(emit.call_args.args[0]['status'],'FAIL')
    def test_trusted_execution_acknowledgment_required(self):
        with patch('sys.argv',['check','--formatter','/x','--native','/y']),patch('sys.stderr'):
            with self.assertRaises(SystemExit) as e: r.main()
            self.assertEqual(e.exception.code,2)
