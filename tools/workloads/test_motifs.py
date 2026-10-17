"""Independent structural/mathematical tests; no simulation or reference dependency."""
import copy
from fractions import Fraction
import math
import unittest
import motifs


def by_id(items, name):
    return next(x for x in items if x['id']==name)


def evaluate(tree, values):
    # Small test-only evaluator of a manually specified tree, not an ODE solver.
    if 'name' in tree:return values[tree['name']]
    if 'integer' in tree:return Fraction(tree['integer'])
    a,b=[evaluate(t,values) for t in tree['children']]
    return {43:lambda:a+b,45:lambda:a-b,42:lambda:a*b,47:lambda:a/b,296:lambda:a**b}[tree['type']]()


class MotifTests(unittest.TestCase):
    def test_six_hashed_original_cases_no_numerical_claim(self):
        cases=motifs.get_cases();self.assertEqual(len(cases),6)
        self.assertEqual([c['id'] for c in cases],list(motifs.IDS))
        for c in cases:
            self.assertEqual(c['kind'],'motif');self.assertEqual(c['numerical'],'NOT_RUN')
            self.assertEqual(c['expectation_id'],c['id']);self.assertIn('Original',c['provenance'])

    def test_hand_authored_entity_counts(self):
        counts=[(1,4,3,2),(1,4,5,4),(2,5,4,3),(1,4,5,4),(2,3,4,3),(1,3,2,2)]
        for name,expected in zip(motifs.IDS,counts):
            p=motifs.expected_projection(name)
            self.assertEqual(tuple(len(p[k]) for k in ['compartments','species','parameters','reactions']),expected)
            motifs.validate_preconditions(name,p)

    def test_concentration_initialization_and_explicit_membership(self):
        for name in motifs.IDS:
            p=motifs.expected_projection(name);volumes={c['id'] for c in p['compartments']}
            self.assertNotIn('default_compartment',volumes)
            for s in p['species']:
                self.assertIn(s['compartment'],volumes);self.assertIsNone(s['initial_amount'])
                self.assertIsNotNone(s['initial_concentration']);self.assertIs(s['has_only_substance_units'],False)
                self.assertIs(s['constant'],False)

    def test_unit_metadata_and_no_numerical_rescaling(self):
        p=motifs.expected_projection('unit_forward_sequence')
        self.assertEqual(p['model_units']['TimeUnits'],'second')
        self.assertEqual(p['model_units']['SubstanceUnits'],'mole')
        self.assertEqual(p['model_units']['ExtentUnits'],'mole')
        self.assertEqual(p['model_units']['VolumeUnits'],'litre')
        self.assertEqual(p['unit_definitions'][0]['units'],[{'kind':28,'exponent':-1.,'scale':0,'multiplier':1.}])
        self.assertEqual(by_id(p['parameters'],'kslow')['value'],.1)
        self.assertEqual(by_id(p['compartments'],'cell')['size'],4)
        self.assertTrue(all(x['units']=='per' for x in p['parameters']))

    def test_forward_assignments_are_trees_not_scalar_substitution(self):
        p=motifs.expected_projection('branched_pathway')
        self.assertIsNone(by_id(p['parameters'],'k1')['value'])
        self.assertEqual(p['initial_assignments'],[{'symbol':'k1','math':{'type':47,'children':[{'type':260,'name':'q'},{'type':256,'integer':2}]}}])
        p=motifs.expected_projection('unit_forward_sequence')
        self.assertIsNone(by_id(p['parameters'],'kfast')['value'])
        self.assertEqual(p['initial_assignments'][0]['symbol'],'kfast')
        self.assertEqual(p['initial_assignments'][0]['math']['type'],42)
        self.assertEqual(p['rules'],[])

    def test_feedback_has_one_true_modifier_not_reactant(self):
        p=motifs.expected_projection('rational_feedback');r=by_id(p['reactions'],'Input')
        self.assertEqual(r['modifiers'],['Z'])
        # Canonical MathML <power/> is libSBML AST_FUNCTION_POWER (296), not caret94.
        self.assertEqual(r['math']['children'][0]['children'][1]['children'][1]['type'],296)
        self.assertEqual([x['species'] for x in r['reactants']],['Feed'])
        self.assertEqual([x['species'] for x in r['products']],['X'])
        for name in motifs.IDS:
            for reaction in motifs.expected_projection(name)['reactions']:
                if name!='rational_feedback' or reaction['id']!='Input':self.assertEqual(reaction['modifiers'],[])

    def test_boundary_clamp_not_immutable_species(self):
        p=motifs.expected_projection('rational_feedback');feed=by_id(p['species'],'Feed')
        self.assertIs(feed['boundary_condition'],True);self.assertIs(feed['constant'],False)
        for name in motifs.IDS:
            for s in motifs.expected_projection(name)['species']:
                if name!='rational_feedback' or s['id']!='Feed':self.assertIs(s['boundary_condition'],False)

    def test_only_authored_reactions_are_reversible(self):
        expected={'enzyme_binding_catalysis':['Bind'],'coupled_transport_catalysis':['Bind'],
                  'reversible_shuttle':['Exchange']}
        for name in motifs.IDS:
            self.assertEqual([r['id'] for r in motifs.expected_projection(name)['reactions'] if r['reversible']],expected.get(name,[]))

    def test_eight_amount_balance_laws_annihilate_effective_stoichiometry(self):
        count=0
        for name in motifs.IDS:
            p=motifs.expected_projection(name);ss={s['id']:s for s in p['species']}
            volumes={c['id']:Fraction(str(c['size'])) for c in p['compartments']}
            for weights,total in motifs.balance_laws(name):
                count+=1
                initial=sum(Fraction(w)*Fraction(str(ss[s]['initial_concentration']))*volumes[ss[s]['compartment']] for s,w in weights.items())
                self.assertEqual(initial,Fraction(str(total)))
                for r in p['reactions']:
                    balance=0
                    for sign,key in [(-1,'reactants'),(1,'products')]:
                        for term in r[key]:
                            if not ss[term['species']]['boundary_condition']:
                                balance+=sign*weights.get(term['species'],0)*Fraction(str(term['stoichiometry']))
                    self.assertEqual(balance,0,(name,r['id'],weights))
        self.assertEqual(count,8)

    def test_open_feedback_has_source_minus_drain_not_closed_mass(self):
        p=motifs.expected_projection('rational_feedback')
        mobile={'X','Y','Z'}
        expected={'Input':1,'XY':0,'YZ':0,'Drain':-1}
        for r in p['reactions']:
            delta=sum(1 for x in r['products'] if x['species'] in mobile)-sum(1 for x in r['reactants'] if x['species'] in mobile)
            self.assertEqual(delta,expected[r['id']])
        self.assertEqual(motifs.balance_laws('rational_feedback'),[({'Feed':1},4)])

    def test_initial_rates_match_independent_exact_arithmetic(self):
        expected={
            'enzyme_binding_catalysis':{'Bind':'4.8','Cat':'0'},
            'branched_pathway':{'AB':'3.6','AC':'1.8','BD':'.9','CD':'1.5'},
            'coupled_transport_catalysis':{'Transport':'1.6','Bind':'3','Cat':'0'},
            'rational_feedback':{'Input':'128/85','XY':'.6','YZ':'0','Drain':'.1'},
            'reversible_shuttle':{'Exchange':'.3','Convert':'1.5','Return':'.375'},
            'unit_forward_sequence':{'First':'2.4','Second':'0'}}
        for name in motifs.IDS:
            p=motifs.expected_projection(name)
            vals={x['id']:Fraction(str(x['size'])) for x in p['compartments']}
            vals.update({x['id']:Fraction(str(x['initial_concentration'])) for x in p['species']})
            vals.update({x['id']:Fraction(str(x['value'])) for x in p['parameters'] if x['value'] is not None})
            # Both reviewed IA cases depend only on an already scalar initialized parameter.
            for a in p['initial_assignments']:vals[a['symbol']]=evaluate(a['math'],vals)
            for r in p['reactions']:self.assertEqual(evaluate(r['math'],vals),Fraction(expected[name][r['id']]))

    def test_no_unmodeled_constructs_or_hidden_conversion_factors(self):
        for name in motifs.IDS:
            p=motifs.expected_projection(name)
            for key in ['rules','events','constraints','function_definitions','packages']:self.assertEqual(p[key],[])
            self.assertIsNone(p['model_units']['ConversionFactor'])
            self.assertTrue(all(s['conversion_factor'] is None for s in p['species']))
            self.assertTrue(all(r['local_parameters']==[] for r in p['reactions']))

    def test_semantic_mutations_reject(self):
        for name in motifs.IDS:
            base=motifs.expected_projection(name)
            for collection,key,value in [('species','initial_concentration',99),('species','initial_amount',99),
                                         ('species','boundary_condition',True),('compartments','size',99),
                                         ('parameters','value',99),('reactions','modifiers',['ghost'])]:
                q=copy.deepcopy(base);q[collection][0][key]=value
                # A rational Feed boundary flag might already be true: choose inverse instead.
                if key=='boundary_condition':q[collection][0][key]=not base[collection][0][key]
                with self.assertRaises(ValueError):motifs.validate_preconditions(name,q)

    def test_math_stoichiometry_units_and_missing_entities_reject(self):
        for name in motifs.IDS:
            q=motifs.expected_projection(name);q['reactions'][0]['math']={'type':256,'integer':0}
            with self.assertRaises(ValueError):motifs.validate_preconditions(name,q)
            q=motifs.expected_projection(name);q['species'].pop()
            with self.assertRaises(ValueError):motifs.validate_preconditions(name,q)
        q=motifs.expected_projection('enzyme_binding_catalysis');q['reactions'][0]['reactants'][0]['stoichiometry']=2
        with self.assertRaises(ValueError):motifs.validate_preconditions('enzyme_binding_catalysis',q)
        q=motifs.expected_projection('unit_forward_sequence');q['unit_definitions'][0]['units'][0]['scale']=-3
        with self.assertRaises(ValueError):motifs.validate_preconditions('unit_forward_sequence',q)

    def test_nonfinite_and_boolean_numeric_confusion_reject(self):
        for value in [True,math.nan,math.inf,-math.inf,'2',10**1000]:
            p=motifs.expected_projection('enzyme_binding_catalysis');p['compartments'][0]['size']=value
            with self.assertRaises(ValueError):motifs.validate_preconditions('enzyme_binding_catalysis',p)

    def test_manifest_source_hash_status_and_identity_drift_reject(self):
        base=motifs.load_manifest()
        for key,value in [('path','../elsewhere'),('sha256','0'*64),('numerical','PASS'),
                          ('kind','published'),('expectation_id','other'),('provenance','')]:
            m=copy.deepcopy(base);m['cases'][0][key]=value
            with self.assertRaises(ValueError):motifs.validate_manifest(m)
        for cases in [[],base['cases'][:-1],base['cases'][::-1],[None]]:
            m=copy.deepcopy(base);m['cases']=cases
            with self.assertRaises(ValueError):motifs.validate_manifest(m)
        with self.assertRaises(ValueError):motifs.validate_manifest([])
        with self.assertRaises(ValueError):motifs._unique([('a',1),('a',2)])

    def test_fresh_return_values_prevent_shared_expected_state_mutation(self):
        p=motifs.expected_projection('enzyme_binding_catalysis');p['species'][0]['initial_concentration']=99
        self.assertEqual(by_id(motifs.expected_projection('enzyme_binding_catalysis')['species'],'C')['initial_concentration'],0)
        law=motifs.balance_laws('enzyme_binding_catalysis');law[0][0]['E']=99
        self.assertEqual(motifs.balance_laws('enzyme_binding_catalysis')[0][0]['E'],1)

    def test_unknown_motif_rejects(self):
        for fn in [motifs.expected_projection,motifs.balance_laws]:
            with self.assertRaises(ValueError):fn('unreviewed')

if __name__=='__main__':unittest.main()
