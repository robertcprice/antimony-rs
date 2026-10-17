"""Executed pinned units contract; does not certify general dimensional validity."""
import unittest
import oracle

class UnitsReferenceTests(unittest.TestCase):
    def convert(self, name, rejected=False):
        folder = 'rejected' if rejected else 'accepted'
        r = oracle.run('convert', (oracle.ROOT/'fixtures'/folder/(name+'.ant')).read_text())
        self.assertEqual(r['status'], 'PASS', r)
        return r['projection']

    def test_builtin_aliases_have_canonical_units_without_definitions(self):
        p = self.convert('units_builtins')
        self.assertEqual(p['unit_definitions'], [])
        self.assertEqual({x['id']:x['units'] for x in p['parameters']}, {'p':'second','q':'mole','r':'dimensionless'})
        self.assertEqual(p['compartments'][0]['units'], 'litre')

    def test_decimal_coefficients_normalize_without_rescaling_values(self):
        p = self.convert('units_scaled')
        components = {x['id']:x['units'][0] for x in p['unit_definitions']}
        expected = {'small':(2.0,-3), 'hour':(3.6,3), 'large':(1.23,2), 'fraction':(1.2,-1), 'precise':(1.2345,0)}
        for name, pair in expected.items():
            self.assertEqual((components[name]['multiplier'],components[name]['scale']), pair)
            self.assertEqual(components[name]['exponent'], 1.0)
        self.assertTrue(all(x['value']==2 for x in p['parameters']))

    def test_component_order_and_quotient_exponents_preserved(self):
        p = self.convert('units_product_order')
        components = {x['id']:x['units'] for x in p['unit_definitions']}
        self.assertEqual([x['kind'] for x in components['product']], [23,28])
        self.assertEqual([x['kind'] for x in components['reverse']], [28,23])
        self.assertEqual([x['exponent'] for x in components['rate']], [1,-1])

    def test_integer_power_and_reciprocal(self):
        p = self.convert('units_powers')
        self.assertEqual([(x['id'],x['units'][0]['exponent']) for x in p['unit_definitions']], [('square',2),('inv',-1)])

    def test_forward_alias_expansion_preserves_definition_order(self):
        p = self.convert('units_forward_alias')
        self.assertEqual([x['id'] for x in p['unit_definitions']], ['rate','small'])
        rate = p['unit_definitions'][0]['units']
        self.assertEqual([(x['kind'],x['scale'],x['exponent']) for x in rate], [(23,-3,1),(28,0,-1)])
        self.assertEqual(p['parameters'][0]['value'], 2)

    def test_compartment_annotation_does_not_rescale_concentration_or_volume(self):
        p = self.convert('units_compartment')
        self.assertEqual(p['compartments'][0]['size'], 3)
        self.assertEqual(p['compartments'][0]['units'], 'cube')
        self.assertEqual(p['species'][0]['initial_concentration'], 2)
        self.assertIsNone(p['species'][0]['substance_units'])

    def test_unknown_and_cyclic_units_are_empty_not_dimensionless(self):
        for name in ('units_unknown','units_cycle'):
            p = self.convert(name, True)
            self.assertTrue(p['unit_definitions'])
            self.assertTrue(all(x['units']==[] for x in p['unit_definitions']))
            self.assertNotEqual(p['parameters'][0]['units'], 'dimensionless')

    def test_fractional_exponent_is_not_truncated_by_integer_getter(self):
        p = self.convert('units_fractional_power', True)
        self.assertEqual(p['unit_definitions'][0]['units'][0]['exponent'], 0.5)

    def test_species_has_means_concentration_and_requires_substance_conversion(self):
        p = self.convert('units_species_annotation', True)
        self.assertEqual(p['species'][0]['substance_units'], 'mole_litre')
        self.assertEqual(p['species'][0]['initial_concentration'], 2)
        self.assertEqual([(u['kind'],u['exponent']) for u in p['unit_definitions'][0]['units']], [(23,1),(18,1)])

    def test_model_defaults_are_behavioral_attributes_not_ordinary_aliases(self):
        p = self.convert('units_model_defaults', True)
        self.assertEqual(p['model_units']['TimeUnits'], 'time_unit')
        self.assertEqual(p['model_units']['SubstanceUnits'], 'substance')
        self.assertEqual(p['model_units']['VolumeUnits'], 'volume')

    def test_upstream_accepts_redefinition_and_dimension_conflict(self):
        p = self.convert('units_redefined', True)
        self.assertEqual(p['unit_definitions'][0]['units'][0]['kind'], 28)
        p = self.convert('units_inconsistent_arithmetic', True)
        self.assertEqual(p['initial_assignments'][0]['math']['type'], 43)
        self.assertEqual(next(x for x in p['parameters'] if x['id']=='p')['units'], 'second')

    def test_scalar_star_and_juxtaposition_have_distinct_power_meaning(self):
        p = self.convert('units_scalar_power', True)
        multiplier = p['unit_definitions'][0]['units'][0]['multiplier']
        self.assertAlmostEqual(multiplier, 2**0.5, places=13)
        r = oracle.run('convert', 'unit square=2 metre^2; p=2;p has square;')
        self.assertEqual(r['status'], 'PASS', r)
        self.assertEqual(r['projection']['unit_definitions'][0]['units'][0]['multiplier'], 2)

    def test_singular_meter_direct_annotation_is_not_a_verified_alias(self):
        r = oracle.run('convert','p=2;p has meter;')
        self.assertEqual(r['status'], 'REFERENCE_REJECTED',r)
        self.assertIn("units 'meter'", r['diagnostic'])
