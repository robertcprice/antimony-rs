"""Exact conditional reference contracts and fail-closed current identity checks."""
import copy,json,tempfile,unittest
from pathlib import Path
from unittest.mock import patch
import conditional_check as c
import oracle,profile_overlay
from native_check import differences

class ConditionalContractTests(unittest.TestCase):
    def rows(self):return {r['id']:r for r in c.load_manifest()['fixtures']}
    def ast(self,id):return c.expectation(self.rows()[id])['projection']['initial_assignments'][0]['math']
    def mutated(self,fn,pattern):
        m=c.load_manifest();fn(m)
        with tempfile.TemporaryDirectory() as d:
            p=Path(d)/'m';p.write_text(json.dumps(m))
            with self.assertRaisesRegex(ValueError,pattern):c.load_manifest(p)
    def test_inventory(self):
        r=list(self.rows().values());self.assertEqual(len(r),248);self.assertEqual(sum(x['native']=='accepted' for x in r),155)
    def test_case_insensitive_safe_ids(self):
        ids=list(self.rows());self.assertEqual(len(ids),len({x.lower() for x in ids}))
    def test_duplicate(self):self.mutated(lambda m:m['fixtures'].append(m['fixtures'][0]),'Duplicate')
    def test_extra_field(self):self.mutated(lambda m:m.update(unreviewed=True),'fields')
    def test_bad_profile(self):self.mutated(lambda m:m.update(implemented_profile='future'),'profile')
    def test_bad_source_hash(self):self.mutated(lambda m:m['fixtures'][0].update(sha256='0'*64),'Hash mismatch')
    def test_bad_snapshot_hash(self):self.mutated(lambda m:m['fixtures'][0].update(expectation_sha256='0'*64),'hash changed')
    def test_source_escape(self):self.mutated(lambda m:m['fixtures'][0].update(path='fixtures/builtins/sin_rate.ant'),'escapes')
    def test_snapshot_escape(self):self.mutated(lambda m:m['fixtures'][0].update(expectation='fixtures/conditionals/manifest.json'),'escapes')
    def test_unknown_implementation_rejected(self):
        with self.assertRaisesRegex(ValueError,'Unknown'):profile_overlay.configuration('conditional-core-v0.4-slice')
    def test_wrong_selected_profile_rejected(self):
        for value in ('math-core-v0.2-slice','math-core-v0.2.1-slice'):
            with patch.object(profile_overlay,'PROFILE',value),self.assertRaisesRegex(ValueError,'requires'):c.load_manifest()
    def test_historical_configurations_preserved(self):
        for value in ('math-core-v0.2-slice','math-core-v0.2.1-slice','conditional-core-v0.3-slice','rules-core-v0.4-slice','functions-core-v0.5-slice','modular-core-v0.8-slice'):
            path,schema=profile_overlay.configuration(value)
            with patch.multiple(profile_overlay,PROFILE=value,PATH=path,OVERLAY_SCHEMA=schema):self.assertEqual(profile_overlay.load()['implemented_profile'],value)
    def test_current_parent_outcomes_unchanged(self):
        new=profile_overlay.load();old=json.loads((oracle.ROOT/'fixtures/manifests/conditional-core-v3-overlay.json').read_text())
        for key in ('workload_overrides','arithmetic_overrides'):self.assertEqual(old[key],new[key])
        for k,v in old['original_overrides'].items():self.assertEqual(new['original_overrides'][k],v)
        expected_added = {'assignment_rule','formula_assignment_rule'}
        if profile_overlay.PROFILE == 'modular-core-v0.8-slice':
            expected_added |= {'event','init_compartment_expression'}
        self.assertEqual(set(new['original_overrides'])-set(old['original_overrides']),expected_added)
    def test_six_boolean_aliases_exact(self):
        for value,type_id in [('true',266),('false',264)]:
            for case in ('lower','title','upper'):self.assertEqual(self.ast('constant_'+value+'_'+case),{'type':type_id})
    def test_outer_function_infix_provenance(self):
        flat=self.ast('left_function_and_append');nested=self.ast('functional_nested_and')
        self.assertEqual(len(flat['children']),3);self.assertEqual(len(nested['children']),2)
        self.assertEqual(nested['children'][0]['type'],304);self.assertTrue(differences(flat,nested))
        self.assertEqual(len(self.ast('empty_left_and')['children']),1)
    def test_right_logical_child_not_flattened(self):
        node=self.ast('right_function_and');self.assertEqual(len(node['children']),2);self.assertEqual(node['children'][1]['type'],304)
    def test_comparison_chain_predicate_neighbors(self):
        r=self.rows()
        for name in ('functional_left_relation_chain','relational_and_chain','plus_wrapped_chain','double_minus_wrapped_chain'):
            self.assertEqual((r[name]['reference_status'],r[name]['diagnostic_code']),('PASS','unsupported_feature'))
        for name in ('single_relation_and_not_chain','mixed_boolean_and_not_chain','relational_or_not_chain','functional_left_nested_relation','not_wrapped_relation'):
            self.assertEqual(r[name]['native'],'accepted')
    def test_missing_otherwise_is_not_zero(self):
        n=self.ast('piecewise_pair_true');m=copy.deepcopy(n);m['children'].append({'type':256,'integer':0});self.assertTrue(differences(n,m))
    def test_branch_order_is_exact(self):
        n=self.ast('piecewise_ordered_second');m=copy.deepcopy(n);m['children'][:4]=m['children'][2:4]+m['children'][:2];self.assertTrue(differences(n,m))
    def test_bool_is_not_numeric_one(self):self.assertTrue(differences(self.ast('constant_true_lower'),{'type':256,'integer':1}))
    def test_double_not_retained(self):
        n=self.ast('double_not_retained');self.assertEqual(n['type'],305);self.assertEqual(n['children'][0]['type'],305)
    def test_unknown_and_eager_domain_contracts(self):
        r=self.rows()
        for name in ('and_unknown_first','or_unknown_first','piecewise_unknown_then_invalid_condition'):self.assertEqual(r[name]['native'],'accepted')
        for name in ('comparison_unknown_then_invalid','nary_comparison_unknown_middle','and_invalid_before_unknown'):self.assertEqual(r[name]['diagnostic_code'],'declaration_conflict')
    def test_static_checks_survive_lazy_branches(self):
        r=self.rows()
        self.assertEqual(r['inactive_cycle_hidden']['diagnostic_code'],'initialization_cycle')
        self.assertEqual(r['literal_rational_zero_inactive']['diagnostic_code'],'unsupported_feature')
        self.assertEqual(r['inactive_branch_units_conflict']['diagnostic_code'],'units_inconsistency')
    def test_null_math_never_supported(self):
        for key in ('initial_assignments','reactions','rules'):
            with self.assertRaisesRegex(ValueError,'Null'):c.require_represented_math({'projection':{key:[{'math':None}]}})
    def test_nested_null_math_never_supported(self):
        with self.assertRaisesRegex(ValueError,'Null'):c.require_represented_math({'projection':{'initial_assignments':[{'math':{'type':295,'children':[None]}}]}})
    def test_only_verified_vocabulary_added(self):
        for tag in ('true','false','and','or','not','eq','geq','gt','leq','lt','neq','piecewise','piece','otherwise'):
            self.assertEqual(oracle.projection_limits(f'<math xmlns="{oracle.MATH}"><{tag}/></math>'),[])
        for tag in ('xor','implies','lambda','bvar','infinity','notanumber','delay','selector'):
            self.assertTrue(oracle.projection_limits(f'<math xmlns="{oracle.MATH}"><{tag}/></math>'))
    def test_unrepresented_attributes_and_symbols_stay_closed(self):
        self.assertTrue(oracle.projection_limits(f'<math xmlns="{oracle.MATH}"><piecewise unverified="1"/></math>'))
        self.assertTrue(oracle.projection_limits(f'<math xmlns="{oracle.MATH}"><csymbol definitionURL="unknown">x</csymbol></math>'))
    def test_outside_projection_never_positive(self):
        def edit(m):next(r for r in m['fixtures'] if r['reference_status']=='PROJECTION_UNSUPPORTED').update(native='accepted',diagnostic_code=None)
        self.mutated(edit,'reference support')
    def test_outside_projection_no_native_when_blocked(self):
        row=next(r for r in self.rows().values() if r['reference_status']=='PROJECTION_UNSUPPORTED')
        with patch.object(c,'run',return_value={'status':'ORACLE_BLOCKED'}),patch.object(c,'cli_process') as execute:self.assertEqual(c.check_case(row,Path('/unused'))['status'],'FAIL')
        execute.assert_not_called()
    def test_outside_projection_no_artifact_credit(self):
        row=next(r for r in self.rows().values() if r['reference_status']=='PROJECTION_UNSUPPORTED');snap=c.expectation(row)
        response=dict(snap,reasons=snap['projection_reasons'],oracle={'packages':snap['oracle_versions'],'antimony_library_version':snap['antimony_library_version'],'antimony_bundled_libsbml_reported_by_xml':snap['antimony_bundled_libsbml']})
        value={'status':'REJECTED','artifact':b'forbidden','report':{'diagnostics':[{'severity':'error','code':'unsupported_feature'}]}}
        with patch.object(c,'run',return_value=response),patch.object(c,'cli_process',return_value=value):self.assertEqual(c.check_case(row,Path('/unused'))['status'],'FAIL')
    def test_nonfinite_never_finite_projection(self):
        for row in self.rows().values():
            if row['reference_status']=='REFERENCE_NONFINITE':
                value=c.expectation(row);self.assertEqual(value['exact_projection'],'NOT_REPRESENTED');self.assertNotIn('projection',value)

if __name__=='__main__':unittest.main()

class AliasSupplementTests(unittest.TestCase):
    def test_inventory_and_combination(self):
        a=c.load_alias_manifest();self.assertEqual(len(a['fixtures']),14)
        rows=c.load_combined_manifests()['fixtures'];self.assertEqual(len(rows),262);self.assertEqual(sum(r['native']=='accepted' for r in rows),159)
    def test_four_zero_calls_are_boolean_leaves(self):
        for r in c.load_alias_manifest()['fixtures']:
            if r['native']=='accepted':
                node=c.expectation(r)['projection']['initial_assignments'][0]['math']
                self.assertEqual(node,{'type':266 if 'true' in r['id'] else 264})
    def test_nonzero_null_only_negative(self):
        for r in c.load_alias_manifest()['fixtures']:
            if r['id'].endswith(('_one','_two')):
                self.assertEqual((r['native'],r['diagnostic_code'],r['reference_status']),('rejected','syntax','PASS'))
                self.assertIsNone(c.expectation(r)['projection']['initial_assignments'][0]['math'])
                with self.assertRaisesRegex(ValueError,'Null'):c.require_represented_math(c.expectation(r))
    def test_lowercase_calls_reference_reject(self):
        for r in c.load_alias_manifest()['fixtures']:
            if '_lower_' in r['id']:self.assertEqual(r['reference_status'],'REFERENCE_REJECTED')
    def test_parent_hash_binding(self):
        a=c.load_alias_manifest();a['parent_manifest']['sha256']='0'*64
        with tempfile.TemporaryDirectory() as d:
            p=Path(d)/'m';p.write_text(json.dumps(a))
            with self.assertRaisesRegex(ValueError,'parent binding'):c.load_alias_manifest(p)
    def test_duplicate_across_supplements(self):
        base=c.load_manifest();alias=c.load_alias_manifest();alias['fixtures'].append(base['fixtures'][0])
        with patch.object(c,'load_alias_manifest',return_value=alias):
            with self.assertRaisesRegex(ValueError,'Duplicate'):c.load_combined_manifests()
