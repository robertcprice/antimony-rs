"""Independent rules analytic derivations and gate failure controls."""
import copy
import math
from pathlib import Path
import unittest
from unittest.mock import patch
import rules_anchors as a
import rule_check as gate
import check


class RulesAnchorTests(unittest.TestCase):
    def test_two_case_manifest(self):
        self.assertEqual([c['id'] for c in a.load_manifest()['cases']], list(a.IDS))

    def test_initial_states_and_volume_one(self):
        self.assertEqual(
            a.expected_values('coupled_rate_and_assignment_rule', 0),
            {'S': 0.0, '[S]': 0.0, 'x': 4.0, 'y': 8.0, 'k': 0.5, 'default_compartment': 1.0},
        )
        self.assertEqual(
            a.expected_values('boundary_species_assignment_and_rate_rule', 0),
            {'S': 0.0, '[S]': 0.0, 'B': 1.0, '[B]': 1.0, 'u': 0.0, 'default_compartment': 1.0},
        )

    def test_analytic_derivatives_and_algebraic_relations(self):
        h = 1e-5
        for t in [0.1, 1.0, 3.0, 7.0, 9.9]:
            v0 = a.expected_values('coupled_rate_and_assignment_rule', t - h)
            v1 = a.expected_values('coupled_rate_and_assignment_rule', t)
            v2 = a.expected_values('coupled_rate_and_assignment_rule', t + h)
            dx_dt = (v2['x'] - v0['x']) / (2 * h)
            ds_dt = (v2['S'] - v0['S']) / (2 * h)
            self.assertAlmostEqual(dx_dt, -0.5 * v1['x'], places=8)
            self.assertAlmostEqual(v1['y'], 2.0 * v1['x'], places=12)
            self.assertAlmostEqual(ds_dt, v1['y'], places=8)

            b0 = a.expected_values('boundary_species_assignment_and_rate_rule', t - h)
            b1 = a.expected_values('boundary_species_assignment_and_rate_rule', t)
            b2 = a.expected_values('boundary_species_assignment_and_rate_rule', t + h)
            du_dt = (b2['u'] - b0['u']) / (2 * h)
            dbs_dt = (b2['S'] - b0['S']) / (2 * h)
            self.assertAlmostEqual(du_dt, 1.0, places=8)
            self.assertAlmostEqual(b1['B'], 1.0 + 0.2 * b1['u'], places=12)
            self.assertAlmostEqual(dbs_dt, b1['B'], places=8)

    def test_nonfinite_out_of_range_and_unknown_reject(self):
        for t in [True, -1, 11, float('nan'), float('inf')]:
            with self.assertRaises(ValueError):
                a.expected_values(a.IDS[0], t)
        with self.assertRaises(ValueError):
            a.expected_values('unknown', 0)

    def test_projection_semantic_mutations_fail(self):
        for id in a.IDS:
            p = a.expected_projection(id)
            a.validate_preconditions(id, p)
            changes = [
                lambda q: q['species'][0].update(boundary_condition=not q['species'][0]['boundary_condition']),
                lambda q: q['compartments'][0].update(size=2),
                lambda q: q['rules'][0].update(kind='algebraicRule'),
                lambda q: q['rules'].reverse(),
                lambda q: q['initial_assignments'].append({'symbol': 'x', 'math': {'type': 256, 'integer': 4}}),
            ]
            for change in changes:
                q = copy.deepcopy(p)
                change(q)
                with self.assertRaises(ValueError):
                    a.validate_preconditions(id, q)

    def test_invariants_hold_and_reject_corrupted_rows(self):
        for id in a.IDS:
            for t in [0, 2.5, 5.0, 10.0]:
                row = {'time': t, **a.expected_values(id, t)}
                for r in a.invariant_values(id, row).values():
                    self.assertAlmostEqual(r['observed'], r['expected'], places=12)
            bad = {'time': 1.0, **a.expected_values(id, 1.0)}
            del bad['S']
            with self.assertRaises(ValueError):
                a.invariant_values(id, bad)

    def test_worker_tolerance_contract_uses_selected_anchor(self):
        cases = a.load_manifest()['cases']
        self.assertEqual(
            check.expected_tolerances(cases[0], a.expected_projection(cases[0]['id']), ['S'], anchor_contract=a),
            [1e-12, 4e-12],
        )
        self.assertEqual(
            check.expected_tolerances(cases[1], a.expected_projection(cases[1]['id']), ['S'], anchor_contract=a),
            [1e-12, 1e-12],
        )

    def test_missing_runtime_is_blocked(self):
        with patch.object(gate, 'fingerprint', side_effect=FileNotFoundError('absent')):
            r = gate.check_all(Path('/unused'))
        self.assertEqual(r['status'], 'ORACLE_BLOCKED')
        self.assertEqual(r['counts']['passed'], 0)

    def test_changed_runtime_fails_aggregate(self):
        with patch.object(gate, 'fingerprint', side_effect=[{'native': 'before'}, {'native': 'after'}]), patch.object(check, 'check_case', return_value={'status': 'PASS'}):
            r = gate.check_all(Path('/unused'))
        self.assertEqual(r['status'], 'FAIL')
        self.assertEqual(r['changed_inputs'], ['native'])

    def test_blocked_case_never_counts_as_pass(self):
        with patch.object(gate, 'fingerprint', return_value={'native': 'same'}), patch.object(check, 'check_case', return_value={'status': 'ORACLE_BLOCKED'}):
            r = gate.check_all(Path('/unused'))
        self.assertEqual(r['status'], 'FAIL')
        self.assertEqual(r['counts'], {'passed': 0, 'blocked': 2, 'failed': 0})

    def test_live_reference_matches_independently_written_structures(self):
        for case in a.load_manifest()['cases']:
            ref = check.reference_run('convert', check.source(case))
            self.assertEqual(ref['status'], 'PASS', ref)
            a.validate_preconditions(case['id'], ref['projection'])


if __name__ == '__main__':
    unittest.main()
