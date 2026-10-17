"""Current symbol corpus contracts and historical-overlay failure controls."""
import copy
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import symbols_check as c
import profile_overlay as o
from oracle import ROOT, run


class SymbolGateTests(unittest.TestCase):
    def test_inventory(self):
        rows = c.load_manifest()['fixtures']
        self.assertEqual(len(rows),66)
        self.assertEqual(len({r['id'] for r in rows}),66)

    def test_duplicate_ids_fail(self):
        m=c.load_manifest();m['fixtures'].append(m['fixtures'][0])
        with tempfile.TemporaryDirectory() as d:
            p=Path(d)/'manifest';p.write_text(json.dumps(m))
            with self.assertRaisesRegex(ValueError,'Duplicate'):c.load_manifest(p)

    def test_wrong_profile_fails(self):
        m=c.load_manifest();m['implemented_profile']='arithmetic-v0.1-slice'
        with tempfile.TemporaryDirectory() as d:
            p=Path(d)/'manifest';p.write_text(json.dumps(m))
            with self.assertRaisesRegex(ValueError,'profile'):c.load_manifest(p)

    def test_source_drift_fails(self):
        m=c.load_manifest()['fixtures'][0];m['sha256']='0'*64
        with self.assertRaisesRegex(ValueError,'Hash mismatch'):c.check_case(m,Path('/unused'))

    def test_expectation_drift_fails(self):
        row=c.load_manifest()['fixtures'][0];row['expectation_sha256']='0'*64
        with self.assertRaisesRegex(ValueError,'hash changed'):c.expectation(row)

    def test_expectation_escape_fails(self):
        row=c.load_manifest()['fixtures'][0];row['expectation']='fixtures/manifests/original-v1.json'
        with self.assertRaisesRegex(ValueError,'escapes'):c.expectation(row)

    def test_reference_blocked_does_not_run_cli(self):
        import expression_check
        with patch.object(expression_check,'run',return_value={'status':'ORACLE_BLOCKED'}),patch.object(expression_check,'cli_process') as native:
            r=c.check_case(c.load_manifest()['fixtures'][0],Path('/unused'))
        self.assertEqual(r['status'],'ORACLE_BLOCKED');native.assert_not_called()

    def test_unset_attributes_and_warnings_preserved(self):
        r=run('convert','compartment c;species S in c;formula k;')
        self.assertEqual(r['status'],'PASS',r)
        p=r['projection'];self.assertIsNone(p['compartments'][0]['size'])
        self.assertIsNone(p['species'][0]['initial_concentration'])
        self.assertIsNone(p['species'][0]['initial_amount'])
        self.assertIsNone(p['parameters'][0]['value'])
        self.assertTrue(r['messages']);self.assertTrue(all(m['severity']=='Warning' for m in r['messages']))

    def test_symbolic_unset_keeps_initial_assignment(self):
        r=run('convert','a=z*2;')
        self.assertEqual(r['status'],'PASS',r)
        self.assertEqual({p['id']:p['value'] for p in r['projection']['parameters']},{'a':None,'z':None})
        self.assertEqual(r['projection']['initial_assignments'][0]['symbol'],'a')

    def test_first_occurrence_reaction_identifier_allocation(self):
        r=run('convert','_J0=>S;_J0;')
        self.assertEqual(r['status'],'PASS',r)
        self.assertEqual(r['projection']['reactions'][0]['id'],'_J1')
        self.assertIn('_J0',[x['id'] for x in r['projection']['species']])

    def test_partial_invalid_constant_is_reference_valid_but_strict_native_negative(self):
        m=c.load_manifest()
        rows=[r for r in m['fixtures'] if r['id'].startswith('partial_zero') or r['id']=='partial_domain_sibling']
        self.assertEqual(len(rows),4)
        for row in rows:
            self.assertEqual(row['reference_status'],'PASS');self.assertEqual(row['native'],'rejected')
            self.assertEqual(row['diagnostic_code'],'declaration_conflict')


class OverlayTests(unittest.TestCase):
    def test_original_manifest_is_not_mutated_and_eight_outcomes_change(self):
        original=json.loads((ROOT/'fixtures/manifests/original-v1.json').read_text());saved=copy.deepcopy(original)
        current=o.original(original)
        self.assertEqual(original,saved)
        expected_changes = 12 if o.PROFILE == 'modular-core-v0.8-slice' else 10
        expected_supported = 60 if o.PROFILE == 'modular-core-v0.8-slice' else 58
        self.assertEqual(sum(a!=b for a,b in zip(original['fixtures'],current['fixtures'])),expected_changes)
        self.assertEqual(sum(r['native_profile_expectation']=='target_supported' for r in current['fixtures']),expected_supported)

    def bad_overlay(self,mutate,pattern):
        value=o.load();mutate(value)
        with tempfile.TemporaryDirectory() as d:
            p=Path(d)/'overlay.json';p.write_text(json.dumps(value))
            with patch.object(o,'PATH',p),self.assertRaisesRegex(ValueError,pattern):o.load()

    def test_wrong_source_hash(self):
        self.bad_overlay(lambda v:v['sources'].__setitem__('fixtures/manifests/original-v1.json','0'*64),'hash changed')

    def test_wrong_old_outcome(self):
        self.bad_overlay(lambda v:v['original_overrides']['absent_values'].__setitem__('from','malformed'),'override')

    def test_unknown_override(self):
        self.bad_overlay(lambda v:v['original_overrides'].__setitem__('unknown',{'from':'malformed','to':'target_supported','reason':'x'}),'override')

    def test_workload_override_inventory_closed(self):
        self.bad_overlay(lambda v:v['workload_overrides'].pop('grid_unset_00'),'workload')

    def test_original_source_manifest_modification_fails(self):
        value=json.loads((ROOT/'fixtures/manifests/original-v1.json').read_text());value['fixtures'].pop()
        with self.assertRaisesRegex(ValueError,'exact historical'):o.original(value)

    def test_unset_projection_untouched_and_contract_only_changes(self):
        case={'id':'grid_unset_00','recipe':{'family':'unset','index':0}}
        contract={'reference_status':'PASS','native_status':'REJECTED','native_stage':'resolve','native_code':'unsupported_feature','projection':{'value':None}}
        updated=o.workload(case,contract)
        self.assertEqual(updated,{'reference_status':'PASS','native_status':'EXPORTED','projection':{'value':None}})
        self.assertEqual(contract['native_status'],'REJECTED')

    def test_workload_precondition_must_match(self):
        with self.assertRaisesRegex(ValueError,'precondition'):
            o.workload({'id':'grid_unset_00','recipe':{'family':'unset'}},{'reference_status':'PASS','native_status':'EXPORTED'})

class OverlayAggregateTests(unittest.TestCase):
    def test_missing_overlay_keeps_historical_rejection_and_fails_new_acceptance(self):
        import native_check as n
        from corpus import snapshot
        original=json.loads((ROOT/'fixtures/manifests/original-v1.json').read_text())
        row=next(r for r in original['fixtures'] if r['id']=='absent_values')
        ref=run('convert',(ROOT/row['path']).read_text())
        with patch.object(n,'run',return_value=ref),patch.object(n,'native_export',return_value={'status':'EXPORTED','xml':'unused'}):
            result=n.check_case(row,Path('/unused'))
        self.assertEqual(result['status'],'FAIL')
        self.assertEqual(result['outcome'],'unexpected_acceptance')

    def test_overlay_mutation_during_native_gate_fails(self):
        import native_check as n
        import sys
        original=json.loads((ROOT/'fixtures/manifests/original-v1.json').read_text())
        fixture=original['fixtures'][0]
        for delete in (False,True):
            with self.subTest(delete=delete),tempfile.TemporaryDirectory() as d:
                binary=Path(d)/'native';binary.write_text('trusted test stand-in');binary.chmod(0o700)
                overlay=Path(d)/'overlay';overlay.write_bytes(o.PATH.read_bytes())
                def check(*args):
                    if delete:overlay.unlink()
                    else:overlay.write_bytes(overlay.read_bytes()+b' ')
                    return {'schema':'antimony-rs-compatibility-v1','profile_id':n.PROFILE_ID,
                            'fixture':fixture['id'],'feature_ids':fixture['feature_ids'],'source_sha256':fixture['sha256'],
                            'status':'PASS','reference_status':'PASS','outcome':'supported_export',
                            'native_stages':dict.fromkeys(n.STAGES,'NOT_RUN')}
                with patch.object(o,'PATH',overlay),patch.object(n,'check_case',side_effect=check),patch.object(n,'emit') as output,patch.object(sys,'argv',[
                    'native_check.py','--native',str(binary),'--trusted-native','--current-profile','--fixture',fixture['id']]):
                    self.assertEqual(n.main(),2)
                self.assertTrue(output.call_args.args[0]['overlay_changed'])
                self.assertEqual(output.call_args.args[0]['status'],'FAIL')

if __name__=='__main__':unittest.main()
