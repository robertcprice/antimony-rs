"""Pinned-reference contract for bounded flat declaration syntax; no native-derived goldens."""
import unittest
import oracle


class FlatDeclarationReferenceTests(unittest.TestCase):
    def convert(self, name, folder='accepted'):
        text = (oracle.ROOT / 'fixtures' / folder / (name + '.ant')).read_text()
        response = oracle.run('convert', text)
        self.assertEqual(response['status'], 'PASS', response)
        return response['projection']

    def test_optional_header_parentheses_preserve_named_model(self):
        for fixture, name in [('header_optional', 'optional_header'),
                              ('header_optional_semicolon', 'header_semicolon')]:
            with self.subTest(fixture=fixture):
                p = self.convert(fixture)
                self.assertEqual(p['model']['id'], name)
                self.assertEqual(p['parameters'][0]['value'], 2)

    def test_formula_list_retains_scalar_versus_initial_assignment(self):
        p = self.convert('formula_list_literals')
        values = {v['id']: v['value'] for v in p['parameters']}
        self.assertEqual(values, {'a': 2, 'b': -3, 'c': None, 'd': None})
        self.assertEqual(p['initial_assignments'], [
            {'symbol': 'c', 'math': {'type': 256, 'integer': 4}},
            {'symbol': 'd', 'math': {'type': 256, 'integer': 5}}])

    def test_forward_formula_list_keeps_first_occurrence_assignment_order(self):
        p = self.convert('formula_list_forward')
        self.assertEqual([x['symbol'] for x in p['initial_assignments']], ['z', 'b'])
        self.assertEqual(p['initial_assignments'][0]['math']['children'][0]['name'], 'b')
        self.assertEqual(p['initial_assignments'][1]['math']['children'][0]['name'], 'a')
        self.assertEqual(next(x for x in p['parameters'] if x['id'] == 'a')['value'], 2)

    def test_mixed_formula_declaration_and_later_initialization(self):
        p = self.convert('formula_list_mixed')
        self.assertEqual({x['id']: x['value'] for x in p['parameters']}, {'a': 1, 'b': None})
        self.assertEqual([x['symbol'] for x in p['initial_assignments']], ['b'])

    def test_species_dependency_constness_propagates_through_list(self):
        for fixture, boundary in [('formula_list_species_dependency', False),
                                  ('formula_list_boundary_dependency', True)]:
            with self.subTest(fixture=fixture):
                p = self.convert(fixture)
                flags = {x['id']: x['constant'] for x in p['parameters']}
                self.assertEqual(flags, {'b': boundary, 'k': True, 'z': boundary})
                self.assertEqual([x['symbol'] for x in p['initial_assignments']], ['z', 'b'])
                self.assertEqual(p['species'][0]['boundary_condition'], boundary)
                self.assertFalse(p['species'][0]['constant'])

    def test_formula_list_accepts_eof_without_semicolon(self):
        p = self.convert('formula_list_eof')
        self.assertEqual({x['id']: x['value'] for x in p['parameters']}, {'a': 1, 'b': 2})

    def test_malformed_bare_and_formula_list_delimiters(self):
        for fixture in ['bare_statement', 'formula_trailing_comma', 'formula_newline_continuation']:
            with self.subTest(fixture=fixture):
                text = (oracle.ROOT / 'fixtures/rejected' / (fixture + '.ant')).read_text()
                r = oracle.run('convert', text)
                self.assertEqual(r['status'], 'REFERENCE_REJECTED', r)
                self.assertEqual(r['load_result'], -1)
                self.assertIn('syntax error', r['diagnostic'])

    def test_parameter_is_not_a_declaration_keyword(self):
        for text in ['parameter a=1,b=2;', 'parameter a,b;']:
            with self.subTest(text=text):
                self.assertEqual(oracle.run('convert', text)['status'], 'REFERENCE_REJECTED')

    def test_missing_duplicate_and_type_promotion_are_strict_native_restrictions(self):
        missing = self.convert('formula_uninitialized', 'rejected')
        self.assertIsNone(next(x for x in missing['parameters'] if x['id'] == 'a')['value'])
        duplicate = self.convert('formula_duplicate', 'rejected')
        self.assertEqual(duplicate['parameters'][0]['value'], 2)
        promoted = self.convert('formula_species_promotion', 'rejected')
        self.assertEqual(promoted['parameters'], [])
        self.assertEqual({x['id']: x['initial_concentration'] for x in promoted['species']}, {'a': 2, 'b': 3})

    def test_deferred_qualifiers_and_assignment_rule_have_real_upstream_meaning(self):
        variable = self.convert('formula_var_qualifier', 'rejected')
        self.assertTrue(all(not x['constant'] for x in variable['parameters']))
        dollar = self.convert('formula_dollar_qualifier', 'rejected')
        self.assertTrue(all(x['constant'] for x in dollar['parameters']))
        ruled = self.convert('formula_assignment_rule', 'rejected')
        self.assertEqual(ruled['rules'][0]['variable'], 'a')
        self.assertEqual(ruled['rules'][0]['kind'], 'assignmentRule')
        self.assertFalse(next(x for x in ruled['parameters'] if x['id'] == 'a')['constant'])


if __name__ == '__main__':
    unittest.main()
