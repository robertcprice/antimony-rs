"""Independent mathematics/contract tests. No RoadRunner, libSBML or downloads."""
import copy
import math
import unittest
import anchors


class AnalyticAnchorTests(unittest.TestCase):
    def test_exact_nonvacuous_frozen_manifest(self):
        m = anchors.load_manifest()
        self.assertEqual(len(m['cases']), 8)
        self.assertEqual(m['grid'], {'start':0,'end':10,'steps':100})
        self.assertEqual(m['comparison'], {'absolute':1e-8,'relative':1e-7})
        for case in m['cases']:
            self.assertEqual(case['selections'], ['time',*anchors.expected_values(case['id'],0)])
            self.assertEqual(len(set(case['selections'])), len(case['selections']))

    def test_initial_conditions_independent_numbers(self):
        expected = {'units_decay':{'S':3,'[S]':3},
                    'reversible':{'A':3,'B':1},
                    'nonunit_no_factor':{'S':6,'[S]':2,'P':0},
                    'boundary':{'B':2,'P':1},'source_drain':{'S':0},
                    'forward_initial':{'S':3,'held':3,'k':.4,'base':2,'delta':1,'p':.8},
                    'stoichiometry':{'A':4,'B':0},
                    'unequal_transfer':{'A':6,'[A]':3,'B':5,'[B]':1}}
        for name, values in expected.items():
            for column,value in values.items():
                self.assertEqual(anchors.expected_values(name,0)[column],value)

    def test_hand_derived_rhs_matches_analytic_derivatives(self):
        # Independently state concentration equations directly from stoichiometry/volumes.
        rhs = {
            'units_decay': lambda x:{'[S]':-.4*x['[S]']},
            'reversible':lambda x:{'[A]':-.3*x['[A]']+.1*x['[B]'],
                                   '[B]':.3*x['[A]']-.1*x['[B]']},
            'nonunit_no_factor':lambda x:{'[S]':-.6*x['[S]']/3,'[P]':.6*x['[S]']/3},
            'boundary':lambda x:{'[B]':0,'[P]':.3*x['[B]']},
            'source_drain':lambda x:{'[S]':.6-.2*x['[S]']},
            'forward_initial':lambda x:{'[S]':-.4*x['[S]'],'held':0},
            'stoichiometry':lambda x:{'[A]':-2*.2*x['[A]'],'[B]':.2*x['[A]']},
            'unequal_transfer':lambda x:{'[A]':-.4*x['[A]'], '[B]':.4*x['[A]']*2/5}}
        h=1e-5
        for name in anchors.IDS:
            for t in [.1,.5,1,3,9.9]:
                low=anchors.expected_values(name,t-h);high=anchors.expected_values(name,t+h)
                for column,rate in rhs[name](anchors.expected_values(name,t)).items():
                    self.assertAlmostEqual((high[column]-low[column])/(2*h),rate,delta=2e-9)

    def test_all_grid_values_finite_and_physical(self):
        for name in anchors.IDS:
            for i in range(101):
                row=anchors.expected_values(name,i/10)
                self.assertTrue(all(math.isfinite(v) and v>=0 for v in row.values()))
                for pair in anchors.invariant_values(name,row).values():
                    self.assertAlmostEqual(pair['observed'],pair['expected'],delta=1e-13)

    def test_no_volume_factor_is_not_inserted(self):
        x=anchors.expected_values('nonunit_no_factor',5)
        self.assertAlmostEqual(x['[S]'],2/math.e)
        self.assertGreater(abs(x['[S]']-2*math.exp(-.6*5)),.6)
        self.assertEqual(x['S'],3*x['[S]'])

    def test_cross_compartment_concentrations_are_not_conserved(self):
        x=anchors.expected_values('unequal_transfer',5)
        self.assertAlmostEqual(x['A']+x['B'],11)
        self.assertGreater(abs(x['[A]']+x['[B]']-4),1)

    def test_boundary_reservoir_not_depleted(self):
        x=anchors.expected_values('boundary',10)
        self.assertEqual((x['B'],x['P']),(2,7))
        self.assertNotIn('amount_balance',anchors.invariant_values('boundary',x))

    def test_initial_assignment_is_not_ongoing_rule(self):
        x=anchors.expected_values('forward_initial',10)
        self.assertEqual(x['held'],3)
        self.assertGreater(x['held']-x['S'],2.9)
        self.assertEqual(anchors.expected_projection('forward_initial')['rules'],[])

    def test_stoichiometry_changes_decay_rate_not_reaction_order(self):
        x=anchors.expected_values('stoichiometry',5)
        self.assertAlmostEqual(x['A'],4*math.exp(-2))
        self.assertAlmostEqual(x['A']+2*x['B'],4)
        self.assertGreater(abs(x['A']-4*math.exp(-1)),.9)

    def test_bad_times_and_unknown_case_reject(self):
        for t in [-1,10.1,math.nan,math.inf,-math.inf,True,'1',None,10**1000]:
            with self.assertRaises(ValueError):anchors.expected_values('units_decay',t)
        with self.assertRaises(ValueError):anchors.expected_values('missing',0)

    def test_invariants_reject_nonfinite_missing_extra_rows(self):
        base=anchors.expected_values('nonunit_no_factor',1)
        for value in [math.nan,math.inf,True,'3']:
            row=dict(base,S=value)
            with self.assertRaises(ValueError):anchors.invariant_values('nonunit_no_factor',row)
        for row in [{},dict(base,extra=0),dict(base,time=math.nan)]:
            with self.assertRaises(ValueError):anchors.invariant_values('nonunit_no_factor',row)
        row=dict(base);del row['S']
        with self.assertRaises(ValueError):anchors.invariant_values('nonunit_no_factor',row)

    def test_mutated_volume_and_stoichiometry_break_invariants(self):
        x=anchors.expected_values('nonunit_no_factor',2);x['S']=x['[S]']
        self.assertGreater(abs(anchors.invariant_values('nonunit_no_factor',x)['amount_concentration_S']['observed']),1)
        x=anchors.expected_values('stoichiometry',2);x['B']*=2
        balance=anchors.invariant_values('stoichiometry',x)['stoichiometric_balance']
        self.assertGreater(abs(balance['observed']-balance['expected']),1)

    def test_hand_written_projection_contracts_accept_themselves(self):
        for name in anchors.IDS:anchors.validate_preconditions(name,anchors.expected_projection(name))

    def test_projection_mutations_fail_for_all_semantic_roles(self):
        p=anchors.expected_projection('units_decay')
        mutations=[('compartments',0,'size',2),('compartments',0,'constant',False),
                   ('species',0,'initial_concentration',4),('species',0,'initial_amount',3),
                   ('species',0,'has_only_substance_units',True),('species',0,'boundary_condition',True),
                   ('species',0,'compartment','other'),('parameters',0,'value',.5),
                   ('parameters',0,'units',None),('reactions',0,'reversible',True)]
        for collection,index,key,value in mutations:
            q=copy.deepcopy(p);q[collection][index][key]=value
            with self.assertRaises(ValueError):anchors.validate_preconditions('units_decay',q)
        for key,value in [('packages',['comp']),('rules',[{}]),('events',[{}]),('level',2),('extra',0)]:
            q=copy.deepcopy(p);q[key]=value
            with self.assertRaises(ValueError):anchors.validate_preconditions('units_decay',q)

    def test_projection_math_unit_and_initialassignment_drift_reject(self):
        q=anchors.expected_projection('stoichiometry');q['reactions'][0]['reactants'][0]['stoichiometry']=1
        with self.assertRaises(ValueError):anchors.validate_preconditions('stoichiometry',q)
        q=anchors.expected_projection('nonunit_no_factor');q['reactions'][0]['math']['children'].append({'type':260,'name':'cell'})
        with self.assertRaises(ValueError):anchors.validate_preconditions('nonunit_no_factor',q)
        q=anchors.expected_projection('units_decay');q['unit_definitions'][0]['units'][0]['multiplier']=60
        with self.assertRaises(ValueError):anchors.validate_preconditions('units_decay',q)
        q=anchors.expected_projection('forward_initial');q['initial_assignments'][-1]['math']={'type':256,'integer':3}
        with self.assertRaises(ValueError):anchors.validate_preconditions('forward_initial',q)

    def test_projection_type_confusion_and_nan_fail(self):
        for value in [True,math.nan,math.inf,'1',10**1000]:
            q=anchors.expected_projection('units_decay');q['compartments'][0]['size']=value
            with self.assertRaises(ValueError):anchors.validate_preconditions('units_decay',q)

    def test_manifest_tolerance_grid_solver_drift_reject(self):
        base=anchors.load_manifest()
        for section,key,value in [('grid','steps',0),('grid','steps',True),('grid','end',0),
                                  ('grid','end',math.nan),('comparison','absolute',1),
                                  ('comparison','relative',math.inf),('solver','relative_tolerance',.1)]:
            q=copy.deepcopy(base);q[section][key]=value
            with self.assertRaises(ValueError):anchors.validate_manifest(q)

    def test_manifest_case_hash_selection_and_constant_drift_reject(self):
        base=anchors.load_manifest()
        for key,value in [('sha256','0'*64),('path','../outside.ant'),
                          ('selections',['time','S']),('constants',{'k':1})]:
            q=copy.deepcopy(base);q['cases'][0][key]=value
            with self.assertRaises(ValueError):anchors.validate_manifest(q)
        for cases in [[],base['cases'][:-1],base['cases'][::-1],[None]]:
            q=copy.deepcopy(base);q['cases']=cases
            with self.assertRaises(ValueError):anchors.validate_manifest(q)
        with self.assertRaises(ValueError):anchors.validate_manifest([])

    def test_contracts_return_fresh_objects(self):
        a=anchors.expected_projection('units_decay');a['species'][0]['initial_concentration']=99
        self.assertEqual(anchors.expected_projection('units_decay')['species'][0]['initial_concentration'],3)
        a=anchors.expected_values('units_decay',0);a['S']=99
        self.assertEqual(anchors.expected_values('units_decay',0)['S'],3)

if __name__=='__main__':unittest.main()
