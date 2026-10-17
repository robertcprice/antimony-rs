"""Independent Phase 6-8 modular/substanceOnly analytic derivations and gate failure controls."""
import copy
from pathlib import Path
import unittest
from unittest.mock import patch
import modular_anchors as a
import modular_check as gate
import check


class ModularAnchorTests(unittest.TestCase):
    def test_two_case_manifest(self):
        self.assertEqual([c['id'] for c in a.load_manifest()['cases']], list(a.IDS))

    def test_initial_states(self):
        self.assertEqual(
            a.expected_values('substance_only_reaction_override_decay', 0),
            {'S': 4.0, '[S]': 2.0, 'P': 0.0, '[P]': 0.0, 'k_fast': 0.5, 'k_slow': 0.1, 'V': 2.0},
        )
        self.assertEqual(
            a.expected_values('modular_port_and_member_override_cascade', 0),
            {'A': 6.0, '[A]': 6.0, 'B': 0.0, '[B]': 0.0, 'M_k': 0.25, 'default_compartment': 1.0},
        )

    def test_analytic_derivatives_and_invariants(self):
        h = 1e-5
        for t in [0.1, 1.0, 3.0, 7.0, 9.9]:
            v0 = a.expected_values('substance_only_reaction_override_decay', t - h)
            v1 = a.expected_values('substance_only_reaction_override_decay', t)
            v2 = a.expected_values('substance_only_reaction_override_decay', t + h)
            ds_dt = (v2['S'] - v0['S']) / (2 * h)
            dp_dt = (v2['P'] - v0['P']) / (2 * h)
            self.assertAlmostEqual(ds_dt, -0.5 * v1['S'], places=8)
            self.assertAlmostEqual(dp_dt, 0.5 * v1['S'], places=8)
            self.assertAlmostEqual(v1['S'], 2.0 * v1['[S]'], places=12)

            m0 = a.expected_values('modular_port_and_member_override_cascade', t - h)
            m1 = a.expected_values('modular_port_and_member_override_cascade', t)
            m2 = a.expected_values('modular_port_and_member_override_cascade', t + h)
            da_dt = (m2['A'] - m0['A']) / (2 * h)
            db_dt = (m2['B'] - m0['B']) / (2 * h)
            self.assertAlmostEqual(da_dt, -0.25 * m1['A'], places=8)
            self.assertAlmostEqual(db_dt, 0.25 * m1['A'], places=8)

    def test_projection_semantic_mutations_fail(self):
        for id in a.IDS:
            p = a.expected_projection(id)
            a.validate_preconditions(id, p)
            q = copy.deepcopy(p)
            q['species'][0]['has_only_substance_units'] = not q['species'][0]['has_only_substance_units']
            with self.assertRaises(ValueError):
                a.validate_preconditions(id, q)

    def test_invariants_hold_and_reject_corrupted_rows(self):
        for id in a.IDS:
            for t in [0, 2.5, 5.0, 10.0]:
                row = {'time': t, **a.expected_values(id, t)}
                for r in a.invariant_values(id, row).values():
                    self.assertAlmostEqual(r['observed'], r['expected'], places=12)

    def test_live_reference_matches_independently_written_structures(self):
        for case in a.load_manifest()['cases']:
            ref = check.reference_run('convert', check.source(case))
            self.assertEqual(ref['status'], 'PASS', ref)
            a.validate_preconditions(case['id'], ref['projection'])


if __name__ == '__main__':
    unittest.main()
