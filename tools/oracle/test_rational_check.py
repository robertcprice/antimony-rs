"""Fail-closed supplemental rational reference and profile controls."""
import copy
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import rational_check as c
import rational_reference
import profile_overlay
from native_check import differences


class RationalContractTests(unittest.TestCase):
    def mutated(self, edit, error):
        m=c.load_manifest();edit(m)
        with tempfile.TemporaryDirectory() as d:
            p=Path(d)/'m.json';p.write_text(json.dumps(m))
            with self.assertRaisesRegex(ValueError,error):c.load_manifest(p)

    def test_inventory(self):
        r=c.load_manifest()['fixtures'];self.assertEqual(len(r),124)
        self.assertEqual(sum(x['native']=='accepted' for x in r),73)
        self.assertEqual(sum(x['reference_status']=='REFERENCE_NONFINITE' for x in r),6)

    def test_duplicate(self):self.mutated(lambda m:m['fixtures'].append(m['fixtures'][0]),'Duplicate')
    def test_unknown_field(self):self.mutated(lambda m:m.update(ignored=True),'fields')
    def test_profile(self):self.mutated(lambda m:m.update(implemented_profile='future'),'profile')
    def test_bad_source(self):self.mutated(lambda m:m['fixtures'][0].update(sha256='0'*64),'Hash mismatch')
    def test_bad_snapshot(self):self.mutated(lambda m:m['fixtures'][0].update(expectation_sha256='0'*64),'hash changed')
    def test_source_escape(self):self.mutated(lambda m:m['fixtures'][0].update(path='fixtures/builtins/sin_rate.ant'),'escapes')
    def test_snapshot_escape(self):self.mutated(lambda m:m['fixtures'][0].update(expectation='fixtures/manifests/builtins-v2/sin_rate.json'),'escapes')
    def test_ancestor_hash(self):self.mutated(lambda m:m['reference_investigation'].update(sha256='0'*64),'investigation hash')
    def test_nonfinite_cannot_be_positive(self):
        def edit(m):
            r=next(r for r in m['fixtures'] if r['reference_status']=='REFERENCE_NONFINITE');r.update(native='accepted',diagnostic_code=None)
        self.mutated(edit,'reference support')
    def test_nonfinite_cannot_be_syntax(self):
        def edit(m):next(r for r in m['fixtures'] if r['reference_status']=='REFERENCE_NONFINITE')['diagnostic_code']='syntax'
        self.mutated(edit,'explicit unsupported')
    def test_wrong_current_configuration_rejected(self):
        with patch.object(profile_overlay,'PROFILE','math-core-v0.2-slice'):
            with self.assertRaisesRegex(ValueError,'requires'):c.load_manifest()
    def test_unknown_configuration_rejected(self):
        for value in ['', 'future', 'math-core-v0.2.2-slice']:
            with self.subTest(value=value), self.assertRaisesRegex(ValueError,'Unknown'):profile_overlay.configuration(value)
    def test_historical_overlay_available(self):
        path,schema=profile_overlay.configuration('math-core-v0.2-slice')
        with patch.multiple(profile_overlay,PROFILE='math-core-v0.2-slice',PATH=path,OVERLAY_SCHEMA=schema):
            self.assertEqual(profile_overlay.load()['implemented_profile'],'math-core-v0.2-slice')
    def test_wrong_report_profile_rejected(self):
        import cli_check
        from test_cli_check import report
        # Actual report validation is exercised with both a known-but-wrong and unknown identity.
        for value in ['math-core-v0.2-slice','unknown']:
            data=report('check');data['profile_id']=value
            with self.assertRaises(ValueError):cli_check.validate_report(data,'check',0,'a=1;')
    def test_rational_is_not_division(self):
        r={'type':259,'numerator':1,'denominator':2};d={'type':47,'children':[{'type':256,'integer':1},{'type':256,'integer':2}]}
        self.assertTrue(differences(r,d));self.assertTrue(differences(r,dict(r,numerator=2,denominator=4)))
    def test_final_negation_contract(self):
        rows={r['id']:r for r in c.load_manifest()['fixtures']}
        self.assertEqual(rows['nested_negation_returns_min']['native'],'accepted')
        self.assertEqual(rows['outer_negation_rescues_positive']['native'],'accepted')
        self.assertEqual(rows['rational_limit_008']['diagnostic_code'],'syntax')
    def test_division_zero_rate_not_rational_zero(self):
        rows={r['id']:r for r in c.load_manifest()['fixtures']}
        self.assertEqual(rows['ordinary_divide_zero_rate']['native'],'accepted')
        self.assertEqual(rows['zero_denominator_initial']['diagnostic_code'],'unsupported_feature')
    def test_nonfinite_unavailable_never_runs_native(self):
        row=next(r for r in c.load_manifest()['fixtures'] if r['reference_status']=='REFERENCE_NONFINITE')
        with patch.object(rational_reference,'run',return_value={'status':'ORACLE_BLOCKED'}),patch.object(c,'cli_process') as execute:
            result=c.check_case(row,Path('/unused'))
        self.assertEqual(result['status'],'FAIL');execute.assert_not_called()
    def test_nonfinite_wrong_rejection_or_artifact_never_passes(self):
        row=next(r for r in c.load_manifest()['fixtures'] if r['reference_status']=='REFERENCE_NONFINITE')
        snap=c.expectation(row);response=dict(snap,oracle={'packages':snap['oracle_versions'],'antimony_library_version':snap['antimony_library_version']})
        for code,artifact in [('syntax',b''),('unsupported_feature',b'not allowed')]:
            value={'status':'REJECTED','artifact':artifact,'report':{'diagnostics':[{'severity':'error','code':code}]}}
            with self.subTest(code=code,artifact=artifact),patch.object(rational_reference,'run',return_value=response),patch.object(c,'cli_process',return_value=value):
                self.assertEqual(c.check_case(row,Path('/unused'))['status'],'FAIL')

    def test_ordinary_reference_blocked_does_not_run_native(self):
        import expression_check
        row=c.load_manifest()['fixtures'][0]
        with patch.object(expression_check,'run',return_value={'status':'ORACLE_BLOCKED'}),patch.object(expression_check,'cli_process') as execute:
            self.assertEqual(c.check_case(row,Path('/unused'))['status'],'ORACLE_BLOCKED')
        execute.assert_not_called()

    def test_nonfinite_cannot_gain_projection_credit(self):
        for row in c.load_manifest()['fixtures']:
            if row['reference_status']=='REFERENCE_NONFINITE':
                value=c.expectation(row);self.assertEqual(value['exact_projection'],'NOT_REPRESENTED')
                self.assertNotIn('projection',value);self.assertTrue(value['nonfinite_math'])

if __name__=='__main__':unittest.main()
