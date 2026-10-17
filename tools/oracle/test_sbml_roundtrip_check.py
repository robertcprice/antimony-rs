"""Unit tests for the SBML -> Antimony -> SBML roundtrip check gate."""
import unittest
import sbml_roundtrip_check as rt


class SbmlRoundtripContractTests(unittest.TestCase):
    def test_accepted_rows_inventory(self):
        rows = rt.load_accepted_rows()
        self.assertEqual(len(rows), 55)
        self.assertEqual(sum(r['readiness'] == 'complete' for r in rows), 49)
        self.assertEqual(sum(r['readiness'] == 'incomplete' for r in rows), 6)


if __name__ == '__main__':
    unittest.main()
