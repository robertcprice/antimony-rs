"""Exact rules reference contracts and fail-closed current identity checks."""
import copy
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import oracle
import profile_overlay
import rules_check as c
from native_check import differences


class RulesContractTests(unittest.TestCase):
    def rows(self):
        return {r['id']: r for r in c.load_manifest()['fixtures']}

    def mutated(self, fn, pattern):
        m = c.load_manifest()
        fn(m)
        with tempfile.TemporaryDirectory() as d:
            p = Path(d) / 'm'
            p.write_text(json.dumps(m))
            with self.assertRaisesRegex(ValueError, pattern):
                c.load_manifest(p)

    def test_inventory(self):
        r = list(self.rows().values())
        self.assertEqual(len(r), 200)
        self.assertEqual(sum(x['native'] == 'accepted' for x in r), 114)
        self.assertEqual(sum(x['native'] == 'rejected' for x in r), 86)

    def test_case_insensitive_safe_ids(self):
        ids = list(self.rows())
        self.assertEqual(len(ids), len({x.lower() for x in ids}))

    def test_duplicate(self):
        self.mutated(lambda m: m['fixtures'].append(m['fixtures'][0]), 'Duplicate')

    def test_extra_field(self):
        self.mutated(lambda m: m.update(unreviewed=True), 'fields')

    def test_bad_profile(self):
        self.mutated(lambda m: m.update(implemented_profile='future'), 'profile')

    def test_bad_source_hash(self):
        self.mutated(lambda m: m['fixtures'][0].update(sha256='0' * 64), 'Hash mismatch')

    def test_bad_snapshot_hash(self):
        self.mutated(lambda m: m['fixtures'][0].update(expectation_sha256='0' * 64), 'hash changed')

    def test_source_escape(self):
        self.mutated(lambda m: m['fixtures'][0].update(path='fixtures/builtins/sin_rate.ant'), 'escapes')

    def test_snapshot_escape(self):
        self.mutated(lambda m: m['fixtures'][0].update(expectation='fixtures/rules/manifest.json'), 'escapes')

    def test_wrong_selected_profile_rejected(self):
        for value in ('math-core-v0.2-slice', 'math-core-v0.2.1-slice', 'conditional-core-v0.3-slice'):
            with patch.object(profile_overlay, 'PROFILE', value), self.assertRaisesRegex(ValueError, 'requires'):
                c.load_manifest()

    def test_rules_projection_guards(self):
        base = c.expectation(self.rows()['scalar_decimal'])
        c.require_valid_rules_projection(base)

        dup = copy.deepcopy(base)
        dup['projection']['rules'].append(dup['projection']['rules'][0])
        with self.assertRaisesRegex(ValueError, 'Duplicate'):
            c.require_valid_rules_projection(dup)

        const_target = copy.deepcopy(base)
        const_target['projection']['parameters'][0]['constant'] = True
        with self.assertRaisesRegex(ValueError, 'non-constant'):
            c.require_valid_rules_projection(const_target)

        missing_target = copy.deepcopy(base)
        missing_target['projection']['rules'][0]['variable'] = 'missing'
        with self.assertRaisesRegex(ValueError, 'non-constant'):
            c.require_valid_rules_projection(missing_target)

        dual = copy.deepcopy(base)
        dual['projection']['initial_assignments'] = [{'symbol': 'a', 'math': {'type': 256, 'integer': 1}}]
        with self.assertRaisesRegex(ValueError, 'InitialAssignment'):
            c.require_valid_rules_projection(dual)

        null_math = copy.deepcopy(base)
        null_math['projection']['rules'][0]['math'] = None
        with self.assertRaisesRegex(ValueError, 'Null'):
            c.require_valid_rules_projection(null_math)

    def test_lexical_scalar_export_contracts(self):
        rows = self.rows()
        lit = c.expectation(rows['scalar_decimal'])['projection']['parameters'][0]
        neg = c.expectation(rows['scalar_negative'])['projection']['parameters'][0]
        grouped = c.expectation(rows['scalar_group'])['projection']['parameters'][0]
        plus = c.expectation(rows['scalar_plus'])['projection']['parameters'][0]
        rat = c.expectation(rows['scalar_rational'])['projection']['parameters'][0]
        self.assertEqual(lit['value'], 2.5)
        self.assertEqual(neg['value'], -2.0)
        self.assertIsNone(grouped['value'])
        self.assertIsNone(plus['value'])
        self.assertIsNone(rat['value'])

    def test_first_seen_and_replacement_order(self):
        rows = self.rows()
        order = c.expectation(rows['rule_order'])['projection']['rules']
        self.assertEqual([r['variable'] for r in order], ['z', 'a', 'm'])
        rep = c.expectation(rows['assign_overwrite_infers'])['projection']
        self.assertEqual([p['id'] for p in rep['parameters']], ['a', 'u'])
        self.assertEqual(len(rep['rules']), 1)

    def test_outside_projection_and_nonfinite_controls(self):
        rows = self.rows()
        self.assertEqual(rows['algebraic_rule_zero_eq']['reference_status'], 'PROJECTION_UNSUPPORTED')
        self.assertEqual(rows['assign_rational_zero_denom_inactive']['reference_status'], 'REFERENCE_NONFINITE')


if __name__ == '__main__':
    unittest.main()
