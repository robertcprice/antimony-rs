"""Arithmetic gate failure controls; live reference name preservation regression."""
import copy
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch
import expression_check as c
import oracle
from cli_check import validate_report
from test_cli_check import report


class ArithmeticGateTests(unittest.TestCase):
    def row(self, native='accepted'):
        return next(x for x in c.load_manifest()['fixtures'] if x['native'] == native)

    def test_manifest_is_hash_pinned_and_nonempty(self):
        rows = c.load_manifest()['fixtures']
        self.assertEqual(len(rows), 54)
        self.assertEqual(sum(x['native'] == 'accepted' for x in rows), 47)

    def test_duplicate_ids_rejected(self):
        value = c.load_manifest(); value['fixtures'].append(value['fixtures'][0])
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / 'manifest.json'; path.write_text(json.dumps(value))
            with self.assertRaisesRegex(ValueError, 'Duplicate'):
                c.load_manifest(path)

    def test_source_hash_drift_rejected(self):
        row = self.row(); row['sha256'] = '0' * 64
        with self.assertRaisesRegex(ValueError, 'Hash mismatch'):
            c.check_case(row, Path('/unused'))

    def test_expectation_hash_drift_rejected(self):
        row = self.row(); row['expectation_sha256'] = '0' * 64
        with self.assertRaisesRegex(ValueError, 'hash changed'):
            c.expectation(row)

    def test_expectation_path_escape_rejected(self):
        row = self.row(); row['expectation'] = 'fixtures/manifests/original-v1.json'
        with self.assertRaisesRegex(ValueError, 'escapes'):
            c.expectation(row)

    def test_reference_blocked_never_passes(self):
        with patch.object(c, 'run', return_value={'status':'ORACLE_BLOCKED'}), patch.object(c, 'cli_process') as native:
            result = c.check_case(self.row(), Path('/unused'))
        self.assertEqual(result['status'], 'ORACLE_BLOCKED'); native.assert_not_called()

    def test_reference_drift_never_executes_native(self):
        row = self.row(); ref = c.expectation(row); ref['oracle'] = {}
        with patch.object(c, 'run', return_value=ref), patch.object(c, 'snapshot', return_value={}), patch.object(c, 'cli_process') as native:
            self.assertEqual(c.check_case(row, Path('/unused'))['status'], 'FAIL')
        native.assert_not_called()

    def run_with_native(self, row, result):
        ref = c.expectation(row); ref['oracle'] = {}
        with patch.object(c, 'run', return_value=ref), patch.object(c, 'snapshot', return_value=c.expectation(row)), patch.object(c, 'cli_process', return_value=result):
            return c.check_case(row, Path('/unused'))

    def test_negative_unexpected_acceptance_fails(self):
        self.assertEqual(self.run_with_native(self.row('rejected'), {'status':'SUCCESS','report':{'diagnostics':[]}})['status'], 'FAIL')

    def test_negative_wrong_diagnostic_fails(self):
        result = {'status':'REJECTED','report':{'diagnostics':[{'severity':'error','code':'other'}]}}
        self.assertEqual(self.run_with_native(self.row('rejected'), result)['status'], 'FAIL')

    def test_negative_exact_diagnostic_passes(self):
        row = self.row('rejected')
        result = {'status':'REJECTED','report':{'diagnostics':[{'severity':'error','code':row['diagnostic_code']}]}}
        self.assertEqual(self.run_with_native(row, result)['status'], 'PASS')

    def test_positive_rejection_fails(self):
        self.assertEqual(self.run_with_native(self.row(), {'status':'REJECTED'})['status'], 'FAIL')

    def test_missing_fingerprint_is_explicit(self):
        self.assertEqual(c.fingerprint([Path('/no/arithmetic-gate-file')]), {'/no/arithmetic-gate-file':None})

    def test_exact_comparator_keeps_ast_categories_and_order(self):
        self.assertTrue(c.differences({'type':257,'real':1.0}, {'type':258,'real':1.0}))
        self.assertTrue(c.differences({'children':[1,2]}, {'children':[2,1]}))
        self.assertTrue(c.differences({'real':1e-5}, {'real':1.000000000000001e-5}))

    def test_previous_implementation_profile_is_not_current(self):
        old = report(); old['profile_id'] = 'flat-declarations-v0.1-slice'
        with self.assertRaisesRegex(ValueError, 'profile'):
            validate_report(old, 'check', 0, 'a=1;')

    def test_reference_user_function_call_identity_is_preserved(self):
        # Direct AST test does not widen the closed XML projection vocabulary.
        program = '''import json,sys,libsbml
sys.path.insert(0, 'tools/oracle')
import oracle
print(json.dumps([oracle.ast(libsbml.parseL3Formula(x)) for x in ('f(x)','g(x)','sin(x)')]))'''
        child = subprocess.run([str(oracle.ROOT / '.venv-oracle/bin/python'), '-c', program], cwd=oracle.ROOT, capture_output=True, text=True, check=True)
        f,g,sine = json.loads(child.stdout)
        self.assertEqual(f['name'], 'f'); self.assertEqual(g['name'], 'g')
        self.assertNotEqual(f,g); self.assertNotIn('name',sine)
        self.assertTrue(oracle.projection_limits(f'<math xmlns="{oracle.MATH}"><lambda/></math>'))


if __name__ == '__main__':
    unittest.main()
