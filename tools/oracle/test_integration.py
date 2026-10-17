"""Real reference tests. No skips: missing reference is a failing gate, not PASS."""
import json
import unittest
import oracle

class ReferenceTests(unittest.TestCase):
    def convert(self, name, folder='accepted'):
        r = oracle.run('convert', (oracle.ROOT / 'fixtures' / folder / (name + '.ant')).read_text())
        self.assertEqual(r['status'], 'PASS', r)
        return r

    def test_first_conversion_and_independent_reinspection(self):
        r = self.convert('first_conversion')
        self.assertEqual(r['projection']['species'][0]['initial_concentration'], 12)
        self.assertEqual(r['projection']['packages'], [])
        other = oracle.run('inspect', r['sbml'])
        self.assertEqual(other['status'], 'PASS', other)
        self.assertEqual(r['projection'], other['projection'])
        self.assertEqual(len(r['messages']), 12)

    def test_arrows(self):
        self.assertTrue(self.convert('arrow_reversible')['projection']['reactions'][0]['reversible'])
        self.assertFalse(self.convert('arrow_irreversible')['projection']['reactions'][0]['reversible'])

    def test_boundaries(self):
        for name in ['boundary_dollar', 'boundary_const']:
            species = self.convert(name)['projection']['species']
            s = next(x for x in species if x['id'] == 'S')
            self.assertTrue(s['boundary_condition'])
            self.assertFalse(s['constant'])

    def test_volume_does_not_change_concentration(self):
        p = self.convert('nonunit_volume')['projection']
        self.assertEqual(p['compartments'][0]['size'], 3)
        self.assertEqual(next(s for s in p['species'] if s['id'] == 'S')['initial_concentration'], 2)

    def test_missing_values_remain_unset(self):
        p = self.convert('absent_values', 'rejected')['projection']
        self.assertTrue(all(s['initial_concentration'] is None and s['initial_amount'] is None for s in p['species']))
        self.assertIsNone(p['parameters'][0]['value'])

    def test_malformed_is_reference_rejected(self):
        r = oracle.run('convert', (oracle.ROOT / 'fixtures/rejected/malformed.ant').read_text())
        self.assertEqual(r['status'], 'REFERENCE_REJECTED', r)
        self.assertEqual(r['load_result'], -1)

    def test_invalid_xml_is_not_oracle_blocked(self):
        self.assertEqual(oracle.run('inspect', '<broken>')['status'], 'SBML_INVALID')

    def test_scientific_notation(self):
        r = self.convert('scientific_notation')
        p = r['projection']
        self.assertEqual(p['parameters'][0]['value'], 0.2)
        literal = p['reactions'][0]['math']['children'][0]['children'][0]
        self.assertEqual(literal['type'], 257)  # libSBML reads e-notation into real value
        self.assertEqual(literal['real'], 0.001)
        # Exercise independent parsing of the alternate legal numeric encoding;
        # preserve its AST type rather than concealing a representation mismatch.
        xml = r['sbml'].replace('<cn> 0.001 </cn>', '<cn type="e-notation">1<sep/>-3</cn>')
        self.assertNotEqual(xml, r['sbml'])
        alternate = oracle.run('inspect', xml)
        self.assertEqual(alternate['status'], 'PASS', alternate)
        alternate_literal = alternate['projection']['reactions'][0]['math']['children'][0]['children'][0]
        self.assertEqual(alternate_literal, {'type':258, 'real':0.001})

    def test_precedence_tree(self):
        p = self.convert('precedence')['projection']['reactions'][0]['math']
        plus = p['children'][0]['children'][0]
        neg, power = plus['children']
        self.assertEqual(neg['type'], 45)  # unary minus
        self.assertEqual(neg['children'][0]['type'], 296)  # power binds tighter
        self.assertEqual(power['type'], 296)
        self.assertEqual(power['children'][1]['type'], 296)  # right-associative

if __name__ == '__main__':
    unittest.main()
