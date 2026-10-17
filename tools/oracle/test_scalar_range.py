"""Executed numeric-boundary evidence; unsupported reference stays unsupported."""
import unittest
import oracle

class ScalarRangeReferenceTests(unittest.TestCase):
    def roles(self,value):
        return [f'a=-{value};',f'species S;S={value};',
                f'compartment c={value};species S in c;S=2;',
                f'species S,P;S=2;P=0;J:{value} S=>P;S;']
    def check_roles(self,value):
        for index,source in enumerate(self.roles(value)):
            with self.subTest(source=source):
                r=oracle.run('convert',source)
                self.assertEqual(r['status'],'PASS',r)
                p=r['projection']
                observed=(p['parameters'][0]['value'] if index==0 else
                          p['species'][0]['initial_concentration'] if index==1 else
                          p['compartments'][0]['size'] if index==2 else
                          p['reactions'][0]['reactants'][0]['stoichiometry'])
                self.assertEqual(observed,(-1 if index==0 else 1)*float(value))
    def test_verified_lower_normal_scalar_roles(self):
        self.check_roles('1e-307')
    def test_verified_upper_normal_scalar_roles(self):
        self.check_roles('1e308')
    def test_tiny_and_underflow_reference_are_not_projection_passes(self):
        for value in ('1e-308','1e-999'):
            with self.subTest(value=value):
                r=oracle.run('convert','a='+value+';')
                self.assertEqual(r['status'],'PROJECTION_UNSUPPORTED',r)
                self.assertNotIn('projection',r)
    def test_minimum_normal_is_not_safe_reference_export_boundary(self):
        r=oracle.run('convert','a=2.2250738585072014e-308;')
        self.assertEqual(r['status'],'REFERENCE_REJECTED',r)
        self.assertIn('Data type mismatch',r['diagnostic'])
    def test_excess_precision_is_observable_not_normalized_by_checker(self):
        for name,expected in [('scalar_excess_decimal_precision',[1.23456789012346,1.0]),
                              ('scalar_excess_integer_precision',[1000000000000000.0])]:
            with self.subTest(name=name):
                source=(oracle.ROOT/'fixtures/rejected'/(name+'.ant')).read_text()
                r=oracle.run('convert',source)
                self.assertEqual(r['status'],'PASS',r)
                self.assertEqual([p['value'] for p in r['projection']['parameters']],expected)
