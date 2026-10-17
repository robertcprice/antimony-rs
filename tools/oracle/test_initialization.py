"""Pinned real-reference contract for arithmetic initialization (no simulation)."""
import unittest
import oracle

class InitializationReferenceTests(unittest.TestCase):
    def convert(self, name, rejected=False, status='PASS'):
        folder = 'rejected' if rejected else 'accepted'
        r = oracle.run('convert', (oracle.ROOT / 'fixtures' / folder / (name + '.ant')).read_text())
        self.assertEqual(r['status'], status, r)
        return r

    def test_arithmetic_is_not_folded_into_scalar_attributes(self):
        p = self.convert('init_numeric_arithmetic')['projection']
        self.assertIsNone(p['parameters'][0]['value'])
        self.assertIsNone(p['species'][0]['initial_concentration'])
        self.assertIsNone(p['species'][0]['initial_amount'])
        self.assertEqual([x['symbol'] for x in p['initial_assignments']], ['S', 'a'])
        self.assertEqual(p['initial_assignments'][1]['math']['type'], 43)
        self.assertEqual(p['rules'], [])

    def test_forward_chain_preserves_source_order_not_topological_order(self):
        p = self.convert('init_forward_chain')['projection']
        self.assertEqual([x['symbol'] for x in p['initial_assignments']], ['z', 'a'])
        self.assertEqual({x['id']:x['value'] for x in p['parameters']}, {'a':None, 'b':3.0, 'z':None})

    def test_normal_species_dependencies_propagate_variable_parameter_flag(self):
        p = self.convert('init_species_dependency')['projection']
        self.assertEqual({x['id']:x['constant'] for x in p['parameters']}, {'a':False, 'b':False})
        self.assertEqual(p['species'][0]['initial_concentration'], 2)
        self.assertEqual(p['compartments'][0]['size'], 3)

    def test_boundary_species_dependencies_propagate_constant_parameter_flag(self):
        p = self.convert('init_boundary_dependency')['projection']
        self.assertEqual({x['id']:x['constant'] for x in p['parameters']}, {'a':True, 'b':True})
        self.assertTrue(p['species'][0]['boundary_condition'])
        self.assertFalse(p['species'][0]['constant'])

    def test_wrapping_and_unary_plus_preserve_initial_assignment_mode(self):
        p = self.convert('init_wrapped_literals')['projection']
        self.assertEqual({x['id']:x['value'] for x in p['parameters']}, {'a':-2.0, 'b':None, 'c':None, 'd':None})
        self.assertEqual([x['symbol'] for x in p['initial_assignments']], ['b', 'c', 'd'])
        self.assertEqual([x['math']['integer'] for x in p['initial_assignments']], [2,3,-2])

    def test_initialization_precedence(self):
        assignments = self.convert('init_precedence')['projection']['initial_assignments']
        self.assertEqual(assignments[0]['math']['type'], 45)
        self.assertEqual(assignments[0]['math']['children'][0]['type'], 296)
        self.assertEqual(assignments[2]['math']['children'][1]['type'], 296)

    def test_reaction_cycle_is_not_initialization_cycle(self):
        p = self.convert('init_reaction_cycle')['projection']
        self.assertEqual(len(p['reactions']), 2)
        self.assertEqual([x['symbol'] for x in p['initial_assignments']], ['k'])
        for name in ('init_cycle','init_self_cycle'):
            r = self.convert(name, True, 'REFERENCE_REJECTED')
            self.assertIn('Loop detected', r['diagnostic'])

    def test_initial_assignment_order_includes_reaction_participants(self):
        p = self.convert('init_first_occurrence_order')['projection']
        self.assertEqual([x['symbol'] for x in p['initial_assignments']], ['S','P','k'])

    def test_upstream_leaves_domain_and_unknown_dependency_checks_to_consumer(self):
        for name in ('init_unknown','init_divzero','init_nonfinite','init_domain'):
            p = self.convert(name, True)['projection']
            self.assertEqual(len(p['initial_assignments']), 1)
            self.assertTrue(all(x['value'] is None for x in p['parameters']))

    def test_compartment_expression_is_valid_upstream_but_deferred_natively(self):
        p = self.convert('init_compartment_expression', True)['projection']
        self.assertIsNone(p['compartments'][0]['size'])
        self.assertEqual(p['initial_assignments'][0]['symbol'], 'cell')
        self.assertEqual(p['species'][0]['initial_concentration'], 2)
