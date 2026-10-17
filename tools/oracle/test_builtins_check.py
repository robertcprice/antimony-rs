"""Numeric builtin vocabulary, exact identity and current gate failure controls."""
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import builtins_check as c
import oracle
import profile_overlay


class BuiltinGateTests(unittest.TestCase):
    def test_inventory(self):
        rows=c.load_manifest()['fixtures']
        self.assertEqual(len(rows),109);self.assertEqual(sum(r['native']=='accepted' for r in rows),66)

    def test_duplicate_ids_fail(self):
        m=c.load_manifest();m['fixtures'].append(m['fixtures'][0])
        with tempfile.TemporaryDirectory() as d:
            p=Path(d)/'manifest';p.write_text(json.dumps(m))
            with self.assertRaisesRegex(ValueError,'Duplicate'):c.load_manifest(p)

    def test_old_profile_is_not_current_corpus(self):
        m=c.load_manifest();m['implemented_profile']='flat-model-v0.2-slice'
        with tempfile.TemporaryDirectory() as d:
            p=Path(d)/'manifest';p.write_text(json.dumps(m))
            with self.assertRaisesRegex(ValueError,'profile'):c.load_manifest(p)

    def test_unknown_manifest_field_fails(self):
        m=c.load_manifest();m['unreviewed']=True
        with tempfile.TemporaryDirectory() as d:
            p=Path(d)/'manifest';p.write_text(json.dumps(m))
            with self.assertRaisesRegex(ValueError,'fields'):c.load_manifest(p)

    def test_source_hash_drift_fails(self):
        r=c.load_manifest()['fixtures'][0];r['sha256']='0'*64
        with self.assertRaisesRegex(ValueError,'Hash mismatch'):c.check_case(r,Path('/unused'))

    def test_expectation_hash_drift_fails(self):
        r=c.load_manifest()['fixtures'][0];r['expectation_sha256']='0'*64
        with self.assertRaisesRegex(ValueError,'hash changed'):c.expectation(r)

    def test_expectation_escape_fails(self):
        r=c.load_manifest()['fixtures'][0];r['expectation']='fixtures/manifests/symbols-v2.json'
        with self.assertRaisesRegex(ValueError,'escapes'):c.expectation(r)

    def test_oracle_blocked_no_native_execution(self):
        import expression_check
        with patch.object(expression_check,'run',return_value={'status':'ORACLE_BLOCKED'}),patch.object(expression_check,'cli_process') as execute:
            r=c.check_case(c.load_manifest()['fixtures'][0],Path('/unused'))
        self.assertEqual(r['status'],'ORACLE_BLOCKED');execute.assert_not_called()

    def test_only_sin_historical_outcome_newly_promoted(self):
        old=json.loads((oracle.ROOT/'fixtures/manifests/flat-model-v2-overlay.json').read_text())
        math_v2=json.loads((oracle.ROOT/'fixtures/manifests/math-core-v2-overlay.json').read_text())
        self.assertEqual(set(math_v2['original_overrides'])-set(old['original_overrides']),{'function_call'})
        new=profile_overlay.load()
        expected={'function_call','assignment_rule','formula_assignment_rule'}
        if profile_overlay.PROFILE == 'modular-core-v0.8-slice':
            expected |= {'event','init_compartment_expression'}
        self.assertEqual(set(new['original_overrides'])-set(old['original_overrides']),expected)
        self.assertEqual(new['workload_overrides'],old['workload_overrides'])
        for k,v in old['original_overrides'].items():self.assertEqual(new['original_overrides'][k],v)

    def test_expanded_records_distinguish_implementation_and_historical_target(self):
        from contracts import validate_record, STAGES
        row={'schema':'antimony-rs-compatibility-v1','profile_id':'kinetic-core-v0.1',
             'fixture':'function_call','feature_ids':['X01'],'source_sha256':'0'*64,
             'status':'PASS','reference_status':'PASS','native_stages':dict.fromkeys(STAGES,'NOT_RUN'),
             'implemented_profile':profile_overlay.PROFILE,'corpus_profile_id':'kinetic-core-v0.1',
             'profile_scope':'Historical corpus, expanded implementation, not frozen target conformance'}
        validate_record(row)
        schema=json.loads((oracle.ROOT/'tools/oracle/compatibility.schema.json').read_text())
        self.assertIn(row['implemented_profile'],schema['properties']['implemented_profile']['enum'])
        self.assertEqual(schema['dependentRequired']['implemented_profile'],['corpus_profile_id','profile_scope'])
        row['implemented_profile']='kinetic-core-v0.1'
        with self.assertRaisesRegex(ValueError,'historical corpus'):validate_record(row)

    def test_current_extension_profile_does_not_claim_frozen_target_conformance(self):
        p=json.loads((oracle.ROOT/'profiles/math-core-v0.2.json').read_text())
        self.assertEqual(p['implementation_id'],'math-core-v0.2-slice')
        self.assertFalse(p['claims']['frozen_target_conformance'])
        self.assertIn('excluded',p['relationship_to_historical_target'])

    def test_arithmetic_overlay_changes_only_historical_sin_outcome(self):
        from expression_check import load_manifest
        old=load_manifest();before=json.dumps(old,sort_keys=True)
        new=profile_overlay.arithmetic(old)
        self.assertEqual(json.dumps(old,sort_keys=True),before)
        changes=[(a,b) for a,b in zip(old['fixtures'],new['fixtures']) if a!=b]
        self.assertEqual(len(changes),1)
        a,b=changes[0];self.assertEqual(a['id'],'deferred_builtin')
        self.assertEqual(b['native'],'accepted');self.assertIsNone(b['diagnostic_code'])
        for k in ('path','sha256','expectation','expectation_sha256','reference_status'):
            self.assertEqual(a[k],b[k])
        self.assertEqual(sum(r['native']=='accepted' for r in new['fixtures']),48)

    def test_retained_scope_negatives_have_positive_feature_companions(self):
        rows={r['id']:r for r in c.load_manifest()['fixtures']}
        pairs={'time_rate':'time_rate_source','time_call_rate':'time_call_rate_source',
               'runtime_domain_unknown':'runtime_unknown_source','runtime_domain_negative':'runtime_negative_parameter',
               'builtin_after_unit_declaration':'builtin_after_custom_unit'}
        for negative,positive in pairs.items():
            self.assertEqual(rows[negative]['native'],'rejected')
            self.assertEqual(rows[positive]['native'],'accepted')
            self.assertEqual(rows[positive]['reference_status'],'PASS')
        self.assertEqual(rows['runtime_domain_negative']['diagnostic_code'],'declaration_conflict')
        self.assertEqual(sum(r['native']=='rejected' for r in rows.values()),43)

    def test_arithmetic_overlay_rejects_changed_historical_manifest(self):
        from expression_check import load_manifest
        m=load_manifest();m['fixtures'].pop()
        with self.assertRaisesRegex(ValueError,'exact historical'):profile_overlay.arithmetic(m)


class BuiltinReferenceTests(unittest.TestCase):
    def expression(self,expr):
        r=oracle.run('convert','a='+expr+';')
        self.assertEqual(r['status'],'PASS',r)
        return r['projection']['initial_assignments'][0]['math']

    def test_log_alias_and_order_are_preserved(self):
        self.assertEqual(self.expression('log(8)'),self.expression('ln(8)'))
        binary=self.expression('log(2,8)')
        self.assertEqual(binary['type'],294);self.assertEqual([x['integer'] for x in binary['children']],[2,8])
        self.assertEqual(self.expression('log10(8)')['children'][0],{'type':256,'integer':10})
        self.assertNotEqual(binary,self.expression('log(8,2)'))

    def test_sqrt_is_root_with_explicit_degree(self):
        root=self.expression('sqrt(4)')
        self.assertEqual(root,{'type':297,'children':[{'type':256,'integer':2},{'type':256,'integer':4}]})

    def test_aliases_keep_correct_operator_identity(self):
        self.assertEqual(self.expression('ceil(2.1)'),self.expression('ceiling(2.1)'))
        self.assertEqual(self.expression('pow(2,3)'),self.expression('power(2,3)'))
        self.assertNotEqual(self.expression('floor(2.1)'),self.expression('ceil(2.1)'))
        self.assertNotEqual(self.expression('min(1,2)'),self.expression('max(1,2)'))

    def test_variadic_arity_order_is_preserved(self):
        value=self.expression('max(1,3,2,4)')
        self.assertEqual(value['type'],320);self.assertEqual([x['integer'] for x in value['children']],[1,3,2,4])

    def test_constants_are_not_parameter_names(self):
        self.assertEqual(self.expression('pi'),{'type':265})
        self.assertEqual(self.expression('exponentiale'),{'type':263})
        self.assertEqual(self.expression('e'),{'type':260,'name':'e'})

    def test_time_call_and_bare_time_match(self):
        self.assertEqual(self.expression('time()'),{'type':262,'name':'time'})
        self.assertEqual(self.expression('time'),self.expression('time()'))

    def test_time_nonzero_arity_is_strict_rejection_not_false_invalid_sbml_claim(self):
        row=next(r for r in c.load_manifest()['fixtures'] if r['id']=='time_arity_one')
        ref=oracle.run('convert',(oracle.ROOT/row['path']).read_text())
        self.assertEqual(ref['status'],'PASS');self.assertIsNone(ref['projection']['initial_assignments'][0]['math'])
        self.assertEqual(row['native'],'rejected');self.assertEqual(row['diagnostic_code'],'syntax')

    def test_builtin_references_preserve_modifiers(self):
        r=oracle.run('convert','species S=2,P=0,M=3;J:S=>P;sin(M)*S;')
        self.assertEqual(r['status'],'PASS');self.assertEqual(r['projection']['reactions'][0]['modifiers'],['M'])

    def test_missing_builtin_argument_value_is_not_zero(self):
        r=oracle.run('convert','a=sin(z);')
        self.assertEqual(r['status'],'PASS');self.assertEqual({x['id']:x['value'] for x in r['projection']['parameters']},{'a':None,'z':None})

    def test_new_numeric_tags_are_admitted_but_semantics_still_checked(self):
        for tag in ('cos','tan','exp','ln','log','logbase','root','degree','abs','floor','ceiling','min','max','pi','exponentiale'):
            self.assertEqual(oracle.projection_limits(f'<math xmlns="{oracle.MATH}"><{tag}/></math>'),[])
        # This tests vocabulary ONLY, not validity of the deliberately incomplete XML.
        invalid=f'<sbml xmlns="{oracle.CORE}" level="3" version="2"><model id="m"><listOfInitialAssignments><initialAssignment symbol="a"><math xmlns="{oracle.MATH}"><apply><sin/></apply></math></initialAssignment></listOfInitialAssignments></model></sbml>'
        self.assertEqual(oracle.run('inspect',invalid)['status'],'SBML_INVALID')

    def test_unverified_math_remains_closed(self):
        for tag in ('lambda','bvar','xor','implies','infinity','notanumber','delay','factorial','sinh'):
            self.assertTrue(oracle.projection_limits(f'<math xmlns="{oracle.MATH}"><{tag}/></math>'))
        self.assertTrue(oracle.projection_limits(f'<math xmlns="{oracle.MATH}"><sin unverified="x"/></math>'))
        self.assertTrue(oracle.projection_limits(f'<math xmlns="{oracle.MATH}"><csymbol definitionURL="http://www.sbml.org/sbml/symbols/delay">delay</csymbol></math>'))

if __name__=='__main__':unittest.main()
