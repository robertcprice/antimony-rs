"""Exact functions reference contracts and fail-closed current identity checks."""
import copy
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import profile_overlay
import functions_check as c


class FunctionsContractTests(unittest.TestCase):
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
        self.assertEqual(len(r), 164)
        self.assertEqual(sum(x['native'] == 'accepted' for x in r), 68)
        self.assertEqual(sum(x['native'] == 'rejected' for x in r), 96)

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
        self.mutated(lambda m: m['fixtures'][0].update(expectation='fixtures/functions/manifest.json'), 'escapes')

    def test_wrong_selected_profile_rejected(self):
        for value in ('math-core-v0.2-slice', 'math-core-v0.2.1-slice', 'conditional-core-v0.3-slice', 'rules-core-v0.4-slice'):
            with patch.object(profile_overlay, 'PROFILE', value), self.assertRaisesRegex(ValueError, 'requires'):
                c.load_manifest()

    def test_functions_projection_guards(self):
        base = c.expectation(self.rows()['fn_two_arg_sum'])
        c.require_valid_functions_projection(base)

        dup = copy.deepcopy(base)
        dup['projection']['function_definitions'].append(dup['projection']['function_definitions'][0])
        with self.assertRaisesRegex(ValueError, 'Duplicate'):
            c.require_valid_functions_projection(dup)

        colliding = copy.deepcopy(base)
        colliding['projection']['function_definitions'][0]['id'] = 'a'
        with self.assertRaisesRegex(ValueError, 'collides'):
            c.require_valid_functions_projection(colliding)

        bad_root = copy.deepcopy(base)
        bad_root['projection']['function_definitions'][0]['math']['type'] = 43
        with self.assertRaisesRegex(ValueError, 'AST_LAMBDA'):
            c.require_valid_functions_projection(bad_root)

        dup_param = copy.deepcopy(base)
        dup_param['projection']['function_definitions'][0]['math']['children'][1]['name'] = 'x'
        with self.assertRaisesRegex(ValueError, 'Duplicate FunctionDefinition parameter'):
            c.require_valid_functions_projection(dup_param)

        null_math = copy.deepcopy(base)
        null_math['projection']['function_definitions'][0]['math'] = None
        with self.assertRaisesRegex(ValueError, 'Null'):
            c.require_valid_functions_projection(null_math)

    def test_outside_projection_and_nonfinite_controls(self):
        rows = self.rows()
        self.assertEqual(rows['reject_empty_function_body']['reference_status'], 'PROJECTION_UNSUPPORTED')
        self.assertEqual(rows['reject_semicolon_only_function_body']['reference_status'], 'PROJECTION_UNSUPPORTED')
        self.assertEqual(rows['reject_body_rational_zero_denom']['reference_status'], 'REFERENCE_NONFINITE')


if __name__ == '__main__':
    unittest.main()
