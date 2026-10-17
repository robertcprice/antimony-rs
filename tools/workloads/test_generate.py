"""Finite-grid determinism/coverage; these are not live differential executions."""
import copy
import unittest
import generate
import generated_contracts as contracts


class GenerationTests(unittest.TestCase):
    def test_frozen_receipt_and_denominators(self):
        cases=generate.validate_receipt()
        self.assertEqual(len(cases),80)
        self.assertEqual(sum(c['kind']=='generated_positive' for c in cases),64)
        self.assertEqual(sum(c['kind']=='generated_negative' for c in cases),16)
        self.assertTrue(all(c['numerical']=='NOT_RUN' for c in cases))

    def test_repeated_generation_is_byte_identical(self):
        self.assertEqual(generate.receipt(),generate.receipt())
        self.assertEqual(len({c['source_sha256'] for c in generate.cases()}),80)
        self.assertEqual(len({c['seed_id'] for c in generate.cases()}),80)

    def test_generated_sources_are_small_import_free(self):
        for case in generate.cases():
            text=generate.render(case['recipe'])
            self.assertLess(len(text.encode()),8192)
            self.assertNotIn('import',text.lower())
            self.assertEqual(generate.sha(text.encode()),case['source_sha256'])

    def test_outside_grid_and_boolean_index_rejected(self):
        for family,index in [('unknown',0),('dag',16),('dag',True),('unset',4),('units',-1)]:
            with self.assertRaises(ValueError):generate.recipe(family,index)

    def test_parameter_grid_is_not_just_identifier_renaming(self):
        sizes=set()
        for case in generate.cases():
            if case['recipe']['family']=='compartments':
                p=contracts.expected(case)['projection'];sizes.add(tuple(c['size'] for c in p['compartments']))
        self.assertEqual(len(sizes),16)

    def test_dag_dependency_constant_flags(self):
        cases=generate.cases()
        for i in range(16):
            p=contracts.expected(cases[i])['projection']
            self.assertEqual([x['symbol'] for x in p['initial_assignments']],['z','b'])
            for name in ('z','b'):
                self.assertEqual(next(x['constant'] for x in p['parameters'] if x['id']==name),i%2==0 or i%4==3)

    def test_wrapped_literals_keep_initial_assignment_mode(self):
        for case in generate.cases():
            if case['recipe']['family']=='numeric':
                p=contracts.expected(case)['projection'];index=case['recipe']['index']
                self.assertEqual(bool(p['initial_assignments']),index>=12)
                self.assertEqual(p['parameters'][0]['value'] is None,index>=12)

    def test_reviewed_signed_ia_and_rejected_precision_boundary(self):
        cases=generate.cases()
        self.assertEqual(contracts.expected(cases[62])['projection']['initial_assignments'][0]['math'],{'type':256,'integer':-2})
        for index,value in [(78,1.23456789012346),(79,-1.23456789012346)]:
            c=contracts.expected(cases[index]);self.assertEqual(c['native_status'],'REJECTED')
            self.assertEqual(c['projection']['parameters'][0]['value'],value)

    def test_unit_component_order_is_not_sorted_or_cancelled(self):
        cases=[c for c in generate.cases() if c['recipe']['family']=='units']
        a=contracts.expected(cases[8])['projection']['unit_definitions'][0]['units']
        b=contracts.expected(cases[12])['projection']['unit_definitions'][0]['units']
        self.assertEqual([x['kind'] for x in a],[23,18]);self.assertEqual([x['kind'] for x in b],[18,23])

    def test_explicit_negative_status_and_diagnostic_contracts(self):
        expected={'cycle':('REFERENCE_REJECTED','resolve','initialization_cycle'),
                  'dimensions':('PASS','resolve','units_inconsistency'),
                  'unset':('PASS','resolve','unsupported_feature'),
                  'envelope':('PASS','syntax','unsupported_feature')}
        for case in generate.cases()[64:]:
            c=contracts.expected(case)
            self.assertEqual(c['native_status'],'REJECTED')
            self.assertEqual((c['reference_status'],c['native_stage'],c['native_code']),expected[case['recipe']['family']])

    def test_contract_rejects_boolean_nan_infinity_and_missing_keys(self):
        for a,b in [(1,True),(1,float('nan')),(1,float('inf')),(1,10**1000),({'a':1},{}),([1],[1,2])]:
            with self.assertRaises(ValueError):contracts.same(a,b)

    def test_full_projection_mutations_are_not_normalized(self):
        case=generate.cases()[17];projection=contracts.expected(case)['projection']
        for change in ('volume','boundary','math','rule'):
            p=copy.deepcopy(projection)
            if change=='volume':p['compartments'][0]['size']=999
            elif change=='boundary':p['species'][0]['boundary_condition']=True
            elif change=='math':p['reactions'][0]['math']={'type':256,'integer':1}
            else:p['rules']=[{'kind':'assignmentRule'}]
            with self.assertRaises(ValueError):contracts.same(projection,p)


if __name__=='__main__':unittest.main()
