"""Live release-specific species/default mapping; not full SI certification."""
import unittest
import oracle

class SpeciesUnitsReferenceTests(unittest.TestCase):
    def convert(self,name,rejected=False,status='PASS'):
        p=oracle.ROOT/'fixtures'/('rejected' if rejected else 'accepted')/('species_units_'+name+'.ant')
        r=oracle.run('convert',p.read_text());self.assertEqual(r['status'],status,r)
        return r['projection']
    def test_direct_cancellation_yields_builtin_without_rescaling(self):
        p=self.convert('implicit_concentration')
        self.assertEqual(p['species'][0]['substance_units'],'mole')
        self.assertEqual(p['species'][0]['initial_concentration'],2)
    def test_scaled_concentration_cancels_only_exact_volume_factors(self):
        p=self.convert('scaled_concentration');u=p['unit_definitions'][-1]['units']
        self.assertEqual([(x['kind'],x['scale'],x['exponent']) for x in u],[(23,-3,1)])
        p=self.convert('scaled_volume');u=p['unit_definitions'][-1]['units']
        self.assertEqual([(x['kind'],x['scale'],x['exponent']) for x in u],[(23,0,1),(18,0,-1),(18,-3,1)])
    def test_alias_expansion_is_after_raw_reduction(self):
        p=self.convert('alias_concentration')
        self.assertEqual([x['exponent'] for x in p['unit_definitions'][-1]['units']],[1,-1,1])
    def test_two_component_inverse_pair_is_not_simplified(self):
        p=self.convert('inverse_pair')
        self.assertEqual([x['exponent'] for x in p['unit_definitions'][-1]['units']],[-1,1])
    def test_dimensionless_generated_name_keeps_release_singularization(self):
        p=self.convert('dimensionless_compartment')
        self.assertEqual(p['species'][0]['substance_units'],'mole_dimensionles')
    def test_user_definition_reuse_preserves_original_order(self):
        p=self.convert('dedup_reverse')
        self.assertEqual(p['species'][0]['substance_units'],'custom')
        self.assertEqual([u['kind'] for u in p['unit_definitions'][0]['units']],[18,23])
    def test_generated_order_is_first_species_not_sorted_id(self):
        p=self.convert('generated_order')
        self.assertEqual([u['id'] for u in p['unit_definitions']],['zed','mole_litre','dimensionless_litre'])
    def test_all_six_default_attributes_and_builtin_elision(self):
        p=self.convert('defaults_builtin')
        self.assertEqual(p['model_units'],dict(AreaUnits='area',ConversionFactor=None,ExtentUnits='mole',LengthUnits='metre',SubstanceUnits='mole',TimeUnits='second',VolumeUnits='litre'))
        self.assertEqual([u['id'] for u in p['unit_definitions']],['area'])
    def test_all_scaled_defaults_preserve_declaration_order(self):
        p=self.convert('defaults_scaled')
        self.assertEqual([u['id'] for u in p['unit_definitions']],['time_unit','substance','volume','area','length','extent'])
        self.assertEqual(p['species'][0]['initial_concentration'],2)
        self.assertIsNone(p['species'][0]['substance_units'])
    def test_default_alias_redirects_entity_but_not_generated_name(self):
        p=self.convert('defaults_alias_entities')
        self.assertEqual(p['model_units']['SubstanceUnits'],'small')
        self.assertEqual(p['parameters'][0]['units'],'small')
        self.assertEqual(p['species'][0]['substance_units'],'substance_litre')
    def test_default_redirect_uses_components_not_lexical_alias(self):
        p=self.convert('defaults_forward_match')
        self.assertEqual(p['model_units']['SubstanceUnits'],'small')
        self.assertEqual([u['id'] for u in p['unit_definitions']],['small'])
    def test_implicit_volume_conversion_ignores_model_override(self):
        for name in ('implicit_volume_override','defaults_dimensionless'):
            p=self.convert(name)
            self.assertEqual(p['species'][0]['substance_units'],'mole_litre')
            self.assertEqual(p['unit_definitions'][-1]['units'][-1]['scale'],0)
    def test_explicit_unknown_volume_can_produce_invalid_reference(self):
        p=self.convert('invalid_explicit',True,'SBML_INVALID')
        self.assertEqual(p['species'][0]['substance_units'],'mole_volume')
        self.assertEqual(p['unit_definitions'],[])
    def test_generated_name_collision_drops_parameter_upstream(self):
        p=self.convert('collision_parameter',True)
        self.assertEqual(p['parameters'],[])
        self.assertEqual(p['unit_definitions'][0]['id'],'mole_litre')
    def test_six_special_names_cannot_be_plain_numeric_parameters(self):
        for name in ('substance','volume','area','length','extent','time_unit'):
            with self.subTest(name=name):
                r=oracle.run('convert',name+'=2;')
                self.assertEqual(r['status'],'REFERENCE_REJECTED',r)
                self.assertIn('unit definition',r['diagnostic'])
