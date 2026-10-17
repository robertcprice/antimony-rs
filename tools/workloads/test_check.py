"""Fail-closed orchestration regressions; fake workers are not compatibility credit."""
import copy
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import check
import generate
import generated_contracts as contracts


class CheckerTests(unittest.TestCase):
    def setUp(self):
        self.case=generate.cases()[0];self.text=generate.render(self.case['recipe']);self.expected=contracts.expected(self.case)
        self.ref={'status':'PASS','projection':copy.deepcopy(self.expected['projection']),'oracle':{'packages':{'antimony':'3.2.0','python-libsbml':'5.21.2'}},'messages':[],'sbml':'<reference />'}
        self.native={'status':'EXPORTED','xml':'<native />','sbml_sha256':'sha','stderr':''}
        self.formatted={'status':'FORMATTED','text':self.text,'bytes':self.text.encode(),'formatted_sha256':generate.sha(self.text.encode()),'stderr':''}

    def run_case(self):
        return check.check_case(self.case,self.text,self.expected,Path('/exporter'),Path('/formatter'))

    def fake_workers(self):
        return (patch.object(check,'run',return_value=self.ref),
                patch.object(check.native,'native_export',return_value=self.native),
                patch.object(check,'native_format',return_value=self.formatted))

    def test_positive_all_stages_and_no_numerical_credit(self):
        a,b,c=self.fake_workers()
        with a,b,c:record=self.run_case()
        self.assertEqual(record['status'],'PASS');self.assertEqual(set(record['stages'].values()),{'PASS'})
        self.assertEqual(record['numerical'],'NOT_RUN')

    def test_common_mode_reference_and_native_mutation_fails_independent_contract(self):
        self.ref['projection']['parameters'][0]['value']=999
        a,b,c=self.fake_workers()
        with a,b,c:record=self.run_case()
        self.assertEqual(record['status'],'FAIL');self.assertEqual(record['stages']['reference_contract'],'FAIL')

    def test_native_projection_mismatch_fails(self):
        wrong=copy.deepcopy(self.ref);wrong['projection']['parameters'][0]['value']=999
        with patch.object(check,'run',side_effect=[self.ref,wrong]),patch.object(check.native,'native_export',return_value=self.native):record=self.run_case()
        self.assertEqual(record['stages']['native_contract'],'FAIL')

    def test_missing_oracle_blocks_before_native(self):
        with patch.object(check,'run',return_value={'status':'ORACLE_BLOCKED'}),patch.object(check.native,'native_export') as exporter:
            record=self.run_case();exporter.assert_not_called()
        self.assertEqual(record['status'],'ORACLE_BLOCKED')

    def test_native_rejection_is_not_positive_success(self):
        self.native={'status':'REJECTED'};a,b,c=self.fake_workers()
        with a,b,c:record=self.run_case()
        self.assertEqual(record['status'],'FAIL')

    def test_formatter_nonidempotence_fails(self):
        other=dict(self.formatted,bytes=b'changed')
        with patch.object(check,'run',return_value=self.ref),patch.object(check.native,'native_export',return_value=self.native),patch.object(check,'native_format',side_effect=[self.formatted,other]):record=self.run_case()
        self.assertEqual(record['stages']['format_idempotence'],'FAIL')

    def test_formatted_reference_semantic_drift_fails(self):
        wrong=copy.deepcopy(self.ref);wrong['projection']['parameters'][0]['value']=999
        with patch.object(check,'run',side_effect=[self.ref,self.ref,wrong]),patch.object(check.native,'native_export',return_value=self.native),patch.object(check,'native_format',return_value=self.formatted):record=self.run_case()
        self.assertEqual(record['stages']['formatted_reference_contract'],'FAIL')

    def test_formatted_native_semantic_drift_fails(self):
        wrong=copy.deepcopy(self.ref);wrong['projection']['parameters'][0]['value']=999
        with patch.object(check,'run',side_effect=[self.ref,self.ref,self.ref,wrong]),patch.object(check.native,'native_export',return_value=self.native),patch.object(check,'native_format',return_value=self.formatted):record=self.run_case()
        self.assertEqual(record['stages']['formatted_native_contract'],'FAIL')

    def test_warning_drift_is_not_suppressed(self):
        self.formatted['stderr']='stage=resolve severity=Warning code=unspecified_units message=changed\n'
        a,b,c=self.fake_workers()
        with a,b,c:record=self.run_case()
        self.assertEqual(record['stages']['format'],'FAIL')

    def negative(self):
        self.case=generate.cases()[68];self.text=generate.render(self.case['recipe']);self.expected=contracts.expected(self.case)
        self.ref['projection']=self.expected['projection']
        diagnostic={'stage':'resolve','code':'units_inconsistency','message':'test'}
        self.native={'status':'REJECTED','diagnostic':diagnostic}
        self.formatted={'status':'REJECTED','diagnostic':diagnostic}

    def test_explicit_negative_has_only_rejection_credit(self):
        self.negative();a,b,c=self.fake_workers()
        with a,b,c:record=self.run_case()
        self.assertEqual(record['status'],'PASS');self.assertEqual(record['outcome'],'expected_rejection')
        self.assertEqual(record['stages']['format_idempotence'],'NOT_RUN')

    def test_wrong_negative_code_fails(self):
        self.negative();self.native['diagnostic']['code']='syntax';a,b,c=self.fake_workers()
        with a,b,c:record=self.run_case()
        self.assertEqual(record['status'],'FAIL')

    def test_wrong_reference_negative_status_fails(self):
        self.negative();self.ref['status']='REFERENCE_REJECTED';a,b,c=self.fake_workers()
        with a,b,c:record=self.run_case()
        self.assertEqual(record['stages']['reference'],'FAIL')

    def test_cycle_requires_reviewed_reference_diagnostic(self):
        self.case=generate.cases()[64];self.text=generate.render(self.case['recipe']);self.expected=contracts.expected(self.case)
        self.ref={'status':'REFERENCE_REJECTED','diagnostic':'unexpected syntax error'}
        a,b,c=self.fake_workers()
        with a,b,c:record=self.run_case()
        self.assertEqual(record['stages']['reference_contract'],'FAIL')

    def test_source_hash_mutation_fails(self):
        self.text+='\n';a,b,c=self.fake_workers()
        with a,b,c:record=self.run_case()
        self.assertEqual(record['status'],'FAIL')

    def test_empty_and_unknown_success_stderr_rejected(self):
        self.assertEqual(check.core_diagnostics(''),[])
        with self.assertRaises(ValueError):check.core_diagnostics('unparsed warning')

    def test_expected_default_compartment_note_is_retained(self):
        line='stage=resolve severity=Note code=default_compartment message=applied default'
        self.assertEqual(check.core_diagnostics(line),[line])

    def test_preparation_mutation_stops_before_any_execution(self):
        with patch.object(check,'fingerprint',side_effect=[{'manifest':'old'},{'manifest':'new'}]),patch.object(check.os,'access',return_value=True),patch.object(check,'check_case') as execute:
            r=check.check_all(Path('ignored'),Path('ignored'));execute.assert_not_called()
        self.assertEqual(r['status'],'FAIL');self.assertEqual(r['changed_inputs'],['manifest'])

    def test_missing_binary_is_blocked_zero_success(self):
        r=check.check_all(Path('/no/such/export'),Path('/no/such/format'))
        self.assertEqual(r['status'],'ORACLE_BLOCKED');self.assertEqual(r['counts']['failed_or_blocked'],86)
        self.assertEqual(r['counts']['generated_supported'],0)

    def test_changed_or_deleted_input_fails_gate(self):
        good={'status':'PASS','kind':'generated_positive','outcome':'supported_generated'}
        for changed in ('new',None):
            with patch.object(check,'fingerprint',side_effect=[{'x':'old'},{'x':changed}]),patch.object(check.os,'access',return_value=True),patch.object(check,'check_case',return_value=good):
                r=check.check_all(Path('ignored'),Path('ignored'))
            self.assertEqual(r['status'],'FAIL');self.assertEqual(r['changed_inputs'],['x'])

    def test_failure_retention_records_source_expected_and_identity(self):
        with tempfile.TemporaryDirectory(dir=check.ROOT/'artifacts') as temp,patch.object(check,'FAILURES',Path(temp)):
            record={'id':'testcase','status':'FAIL','_failure_bundle':{'source':'a=1;','expected_contract':{'x':1}}}
            check.retain_failure(record,{'checker':'hash'},0)
            saved=json.loads((check.ROOT/record['retention']['path']).read_text())
            self.assertEqual(saved['source'],'a=1;');self.assertEqual(saved['input_sha256'],{'checker':'hash'})
            self.assertEqual(saved['minimization']['status'],'NOT_RUN')

    def test_failure_retention_limit_is_explicit(self):
        record={'id':'testcase','_failure_bundle':{'source':'a=1;'}}
        check.retain_failure(record,{},20)
        self.assertEqual(record['retention']['status'],'LIMIT_REACHED');self.assertNotIn('_failure_bundle',record)


if __name__=='__main__':unittest.main()
