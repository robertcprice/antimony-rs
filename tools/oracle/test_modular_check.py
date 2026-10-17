"""Unit tests for the Phase 6-8 modular & events oracle gate."""
import copy
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import profile_overlay
import modular_check as c


class ModularContractTests(unittest.TestCase):
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
        self.assertEqual(len(r), 48)
        self.assertEqual(sum(x['readiness'] == 'complete' for x in r), 31)
        self.assertEqual(sum(x['readiness'] == 'incomplete' for x in r), 2)
        self.assertEqual(sum(x['readiness'] == 'rejected' for x in r), 15)
        self.assertEqual(sum(x['native'] == 'accepted' for x in r), 33)
        self.assertEqual(sum(x['native'] == 'rejected' for x in r), 15)

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
        self.mutated(lambda m: m['fixtures'][0].update(expectation='fixtures/modular/manifest.json'), 'escapes')

    def test_wrong_selected_profile_rejected(self):
        for value in ('math-core-v0.2-slice', 'conditional-core-v0.3-slice', 'rules-core-v0.4-slice', 'functions-core-v0.5-slice'):
            with patch.object(profile_overlay, 'PROFILE', value), self.assertRaisesRegex(ValueError, 'requires'):
                c.load_manifest()

    def test_modular_projection_guards(self):
        rows = self.rows()
        ev_snap = c.expectation(rows['event_named_multiple_assignments'])
        c.require_valid_modular_projection(ev_snap)

        dup_ev = copy.deepcopy(ev_snap)
        dup_ev['projection']['events'].append(dup_ev['projection']['events'][0])
        with self.assertRaisesRegex(ValueError, 'Duplicate event id'):
            c.require_valid_modular_projection(dup_ev)

        const_target = copy.deepcopy(ev_snap)
        const_target['projection']['parameters'][0]['constant'] = True
        with self.assertRaisesRegex(ValueError, 'non-constant'):
            c.require_valid_modular_projection(const_target)

        sub_snap = c.expectation(rows['substance_only_basic_scalar'])
        c.require_valid_modular_projection(sub_snap)
        bad_sub = copy.deepcopy(sub_snap)
        bad_sub['projection']['species'][0]['initial_concentration'] = 10.0
        with self.assertRaisesRegex(ValueError, 'initial_concentration'):
            c.require_valid_modular_projection(bad_sub)


if __name__ == '__main__':
    unittest.main()
