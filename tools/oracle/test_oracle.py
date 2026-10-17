"""Stdlib tests; integration tests run separately and fail if oracle unavailable."""
import json
from pathlib import Path
import subprocess
import sys
import unittest
from unittest.mock import patch
import oracle
import corpus

class HarnessTests(unittest.TestCase):
    def test_zero_is_successful_load(self):
        self.assertTrue(oracle.load_succeeded(0))
        self.assertTrue(oracle.load_succeeded(1))
        self.assertFalse(oracle.load_succeeded(-1))

    def test_missing_interpreter_is_blocked(self):
        self.assertEqual(oracle.run('probe', python='/no/such/python')['status'], 'ORACLE_BLOCKED')

    def test_unavailable_dependencies_are_blocked(self):
        with patch('oracle.dependencies', side_effect=ImportError('missing antimony')):
            self.assertEqual(oracle.worker({'mode': 'probe'})['status'], 'ORACLE_BLOCKED')

    def test_child_crash_is_error_not_rejection(self):
        with patch('subprocess.run', return_value=subprocess.CompletedProcess([], -11, '', 'crash')):
            self.assertEqual(oracle.run('probe')['status'], 'ORACLE_ERROR')

    def test_invalid_json_is_error(self):
        with patch('subprocess.run', return_value=subprocess.CompletedProcess([], 0, 'junk', '')):
            self.assertEqual(oracle.run('probe')['status'], 'ORACLE_ERROR')

    def test_timeout_is_error(self):
        with patch('subprocess.run', side_effect=subprocess.TimeoutExpired('oracle', 1)):
            self.assertEqual(oracle.run('probe')['status'], 'ORACLE_ERROR')

    def test_large_input_is_rejected_before_spawn(self):
        self.assertEqual(oracle.run('convert', 'x' * (oracle.MAX_INPUT + 1))['status'], 'INPUT_REJECTED')

    def test_trusted_acknowledgement_required(self):
        out = subprocess.run([sys.executable, str(Path(oracle.__file__)), 'convert', 'fake.ant'], capture_output=True)
        self.assertEqual(out.returncode, 2)
        self.assertIn(b'--trusted', out.stderr)

    def test_fixture_hash_guard(self):
        with self.assertRaisesRegex(ValueError, 'Hash mismatch'):
            corpus.fixture_source({'path': 'fixtures/accepted/first_conversion.ant', 'sha256': 'bad', 'id': 'bad'})

    def test_fixture_path_guard(self):
        with self.assertRaisesRegex(ValueError, 'escapes'):
            corpus.fixture_source({'path': '../outside.ant'})

    def test_package_namespace_fails_closed(self):
        xml = f'<sbml xmlns="{oracle.CORE}" xmlns:fbc="http://www.sbml.org/sbml/level3/version1/fbc/version2" level="3" version="2"><model id="x"/></sbml>'
        self.assertTrue(oracle.projection_limits(xml))

    def test_stoichiometry_math_fails_closed(self):
        self.assertTrue(oracle.projection_limits(f'<sbml xmlns="{oracle.CORE}"><stoichiometryMath/></sbml>'))

    def test_unknown_csymbol_fails_closed(self):
        self.assertTrue(oracle.projection_limits(f'<math xmlns="{oracle.MATH}"><csymbol definitionURL="https://example.invalid">x</csymbol></math>'))

    def test_dtd_fails_closed(self):
        self.assertTrue(oracle.projection_limits('<!DOCTYPE x><x/>'))

    def test_rule_metadata_fails_closed(self):
        self.assertEqual(oracle.projection_limits(f'<sbml xmlns="{oracle.CORE}"><assignmentRule variable="a"/></sbml>'), [])
        self.assertEqual(oracle.projection_limits(f'<sbml xmlns="{oracle.CORE}"><rateRule variable="a"/></sbml>'), [])
        self.assertTrue(oracle.projection_limits(f'<sbml xmlns="{oracle.CORE}"><assignmentRule variable="a" metaid="m"/></sbml>'))
        self.assertTrue(oracle.projection_limits(f'<sbml xmlns="{oracle.CORE}"><rateRule variable="a" sboTerm="SBO:0000001"/></sbml>'))
        self.assertTrue(oracle.projection_limits(f'<sbml xmlns="{oracle.CORE}"><algebraicRule variable="a"/></sbml>'))

if __name__ == '__main__':
    unittest.main()
