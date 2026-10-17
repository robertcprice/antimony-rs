"""Unit tests for the independent collaboration & pathway verification gate."""
import copy
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import profile_overlay
import collaboration_check as c


class CollaborationContractTests(unittest.TestCase):
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

    def test_inventory_and_readiness_counts(self):
        r = list(self.rows().values())
        self.assertEqual(len(r), 36)
        self.assertEqual(sum(x['readiness'] == 'complete' for x in r), 18)
        self.assertEqual(sum(x['readiness'] == 'incomplete' for x in r), 4)
        self.assertEqual(sum(x['readiness'] == 'rejected' for x in r), 14)
        self.assertEqual(sum(x['native'] == 'accepted' for x in r), 22)
        self.assertEqual(sum(x['native'] == 'rejected' for x in r), 14)

    def test_case_insensitive_safe_ids(self):
        ids = list(self.rows())
        self.assertEqual(len(ids), len({x.lower() for x in ids}))

    def test_duplicate_id_rejected(self):
        self.mutated(lambda m: m['fixtures'].append(m['fixtures'][0]), 'Duplicate')

    def test_extra_top_field_rejected(self):
        self.mutated(lambda m: m.update(unreviewed=True), 'fields')

    def test_bad_profile_rejected(self):
        self.mutated(lambda m: m.update(implemented_profile='future'), 'profile')

    def test_bad_source_hash_rejected(self):
        self.mutated(lambda m: m['fixtures'][0].update(sha256='0' * 64), 'Hash mismatch')

    def test_bad_snapshot_hash_rejected(self):
        self.mutated(lambda m: m['fixtures'][0].update(expectation_sha256='0' * 64), 'hash changed')

    def test_source_escape_rejected(self):
        self.mutated(lambda m: m['fixtures'][0].update(path='fixtures/functions/cases/fn_const_zero_arg.ant'), 'escapes')

    def test_snapshot_escape_rejected(self):
        self.mutated(lambda m: m['fixtures'][0].update(expectation='fixtures/collaboration/manifest.json'), 'escapes')

    def test_wrong_selected_profile_rejected(self):
        for value in ('math-core-v0.2-slice', 'conditional-core-v0.3-slice', 'rules-core-v0.4-slice'):
            with patch.object(profile_overlay, 'PROFILE', value), self.assertRaisesRegex(ValueError, 'requires'):
                c.load_manifest()

    def test_readiness_mismatch_rejected(self):
        # Flipping a complete fixture to incomplete or vice-versa must fail validation
        self.mutated(lambda m: m['fixtures'][0].update(readiness='incomplete'), 'marked incomplete')
        self.mutated(
            lambda m: next(x for x in m['fixtures'] if x['id'] == 'incomplete_michaelis_menten_draft').update(readiness='complete'),
            'marked complete',
        )

    def test_simulation_ready_projection_helper(self):
        rows = self.rows()
        complete_snap = c.expectation(rows['filled_in_michaelis_menten_draft'])
        incomplete_snap = c.expectation(rows['incomplete_michaelis_menten_draft'])
        self.assertTrue(c.is_simulation_ready_projection(complete_snap['projection']))
        self.assertFalse(c.is_simulation_ready_projection(incomplete_snap['projection']))

        # Clearing a parameter value without an initialAssignment or assignmentRule makes it incomplete
        degraded = copy.deepcopy(complete_snap['projection'])
        degraded['parameters'][0]['value'] = None
        self.assertFalse(c.is_simulation_ready_projection(degraded))


if __name__ == '__main__':
    unittest.main()
