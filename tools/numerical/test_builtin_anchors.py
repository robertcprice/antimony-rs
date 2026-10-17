"""Independent builtin analytic derivations and gate failure controls."""
import copy
import math
from pathlib import Path
import unittest
from unittest.mock import patch
import builtin_anchors as a
import builtin_check as gate
import check


class BuiltinAnchorTests(unittest.TestCase):
    def test_two_case_manifest(self):
        self.assertEqual([c['id'] for c in a.load_manifest()['cases']],list(a.IDS))

    def test_initial_state_zero_and_volume_one(self):
        for id in a.IDS:self.assertEqual(a.expected_values(id,0),{'S':0.0,'[S]':0.0,'default_compartment':1.0})

    def test_analytic_derivatives_match_handwritten_input(self):
        for id in a.IDS:
            for t in [0.1,1.,3.,7.,9.9]:
                h=1e-5;derivative=(a.expected_values(id,t+h)['S']-a.expected_values(id,t-h)['S'])/(2*h)
                expected=math.exp(-t) if id=='exponential_input' else 1+math.sin(t)
                self.assertAlmostEqual(derivative,expected,places=8)

    def test_nonfinite_out_of_range_and_unknown_reject(self):
        for t in [True,-1,11,float('nan'),float('inf')]:
            with self.assertRaises(ValueError):a.expected_values(a.IDS[0],t)
        with self.assertRaises(ValueError):a.expected_values('unknown',0)

    def test_projection_semantic_mutations_fail(self):
        for id in a.IDS:
            p=a.expected_projection(id);a.validate_preconditions(id,p)
            changes=[lambda q:q['species'][0].update(initial_concentration=1),lambda q:q['compartments'][0].update(size=2),lambda q:q['reactions'][0]['math'].update(type=999),lambda q:q['reactions'][0].update(reversible=True),lambda q:q['rules'].append({'unreviewed':True})]
            for change in changes:
                q=copy.deepcopy(p);change(q)
                with self.assertRaises(ValueError):a.validate_preconditions(id,q)

    def test_amount_and_volume_invariants(self):
        for id in a.IDS:
            row={'time':1,**a.expected_values(id,1)}
            for r in a.invariant_values(id,row).values():self.assertEqual(r['observed'],r['expected'])
            del row['S']
            with self.assertRaises(ValueError):a.invariant_values(id,row)

    def test_worker_tolerance_contract_uses_selected_anchor(self):
        case=a.load_manifest()['cases'][0]
        self.assertEqual(check.expected_tolerances(case,a.expected_projection(case['id']),['S'],anchor_contract=a),[1e-12])

    def test_missing_runtime_is_blocked(self):
        with patch.object(gate,'fingerprint',side_effect=FileNotFoundError('absent')):
            r=gate.check_all(Path('/unused'))
        self.assertEqual(r['status'],'ORACLE_BLOCKED');self.assertEqual(r['counts']['passed'],0)

    def test_changed_runtime_fails_aggregate(self):
        with patch.object(gate,'fingerprint',side_effect=[{'native':'before'},{'native':'after'}]),patch.object(check,'check_case',return_value={'status':'PASS'}):
            r=gate.check_all(Path('/unused'))
        self.assertEqual(r['status'],'FAIL');self.assertEqual(r['changed_inputs'],['native'])

    def test_blocked_case_never_counts_as_pass(self):
        with patch.object(gate,'fingerprint',return_value={'native':'same'}),patch.object(check,'check_case',return_value={'status':'ORACLE_BLOCKED'}):
            r=gate.check_all(Path('/unused'))
        self.assertEqual(r['status'],'FAIL');self.assertEqual(r['counts'],{'passed':0,'blocked':2,'failed':0})

    def test_live_reference_matches_independently_written_structures(self):
        for case in a.load_manifest()['cases']:
            ref=check.reference_run('convert',check.source(case))
            self.assertEqual(ref['status'],'PASS',ref)
            a.validate_preconditions(case['id'],ref['projection'])

if __name__=='__main__':unittest.main()
