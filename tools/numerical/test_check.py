"""Adversarial checker controls, not additional numerical model successes."""
import copy
import json
from pathlib import Path
import subprocess
import unittest
from unittest.mock import patch

import check
import anchors


class CheckerTests(unittest.TestCase):
    def setUp(self):
        self.manifest = anchors.load_manifest()
        self.case = self.manifest['cases'][2]  # nonunit volume distinguishes units
        self.projection = anchors.expected_projection(self.case['id'])
        self.xml = '<sbml>test protocol identity only</sbml>'
        self.response = self.good_response()

    def good_response(self):
        case = self.case
        rows = [[i/10, *[anchors.expected_values(case['id'], i/10)[x] for x in case['selections'][1:]]] for i in range(101)]
        floating = [s['id'] for s in self.projection['species'] if not s['boundary_condition']]
        return {'schema': check.WORKER_SCHEMA, 'status': 'PASS',
                'metadata': {'packages': {'libroadrunner': '2.10.0', 'numpy': '2.2.6'}, 'conserved_moiety_analysis': False, 'roadrunner_version':'2.10.0; test'},
                'solver_readback': copy.deepcopy(check.SOLVER), 'selections': case['selections'][:],
                'column_names': case['selections'][:], 'timeseries': rows,
                'sbml_sha256': check.digest(self.xml.encode()), 'floating_species_ids': floating,
                'effective_absolute_tolerance_before': check.expected_tolerances(case, self.projection, floating),
                'effective_absolute_tolerance_after': check.expected_tolerances(case, self.projection, floating, dict(zip(case['selections'], rows[-1])))}

    def validate(self, response=None):
        return check.validate_simulation(response or self.response, self.case, self.xml, self.projection)

    def test_exact_analytic_protocol_passes(self):
        rows = self.validate()
        outcome = check.compare(rows, self.case['selections'], [anchors.expected_values(self.case['id'], i/10) for i in range(101)])
        self.assertEqual(outcome['status'], 'PASS')
        self.assertEqual(outcome['max_scaled_error'], 0)

    def test_wrong_or_duplicate_columns_rejected(self):
        for key in ('selections', 'column_names'):
            response = copy.deepcopy(self.response)
            response[key][1:3] = response[key][1:3][::-1]
            with self.assertRaises(ValueError): self.validate(response)

    def test_truncated_or_extra_rows_rejected(self):
        for rows in ([], self.response['timeseries'][:-1], self.response['timeseries'] + [self.response['timeseries'][-1]]):
            response = copy.deepcopy(self.response); response['timeseries'] = rows
            with self.assertRaises(ValueError): self.validate(response)

    def test_wrong_time_and_duplicate_time_rejected(self):
        for t in (0, .101, 100):
            response = copy.deepcopy(self.response); response['timeseries'][1][0] = t
            with self.assertRaises(ValueError): self.validate(response)

    def test_nonfinite_and_boolean_values_rejected(self):
        for value in (float('nan'), float('inf'), -float('inf'), True, '1'):
            response = copy.deepcopy(self.response); response['timeseries'][1][1] = value
            with self.assertRaises(ValueError): self.validate(response)

    def test_missing_column_rejected(self):
        self.response['timeseries'][50].pop()
        with self.assertRaises(ValueError): self.validate()

    def test_wrong_sbml_identity_rejected(self):
        self.response['sbml_sha256'] = '0'*64
        with self.assertRaises(ValueError): self.validate()

    def test_changed_solver_settings_rejected(self):
        self.response['solver_readback']['relative_tolerance'] = 1e-3
        with self.assertRaises(ValueError): self.validate()

    def test_wrong_versions_and_reduction_rejected(self):
        self.response['metadata']['packages']['libroadrunner'] = '0'
        with self.assertRaises(ValueError): self.validate()
        self.response = self.good_response()
        self.response['metadata']['conserved_moiety_analysis'] = True
        with self.assertRaises(ValueError): self.validate()

    def test_actual_library_version_mismatch_rejected(self):
        self.response['metadata']['roadrunner_version'] = '0.0.0; mock'
        with self.assertRaises(ValueError): self.validate()

    def test_duplicate_comparator_selection_rejected(self):
        with self.assertRaises(ValueError): check.compare([[0, 1, 1]], ['time', 'S', 'S'], [{'S': 1}])

    def test_wrong_effective_tolerance_rejected(self):
        for key in ('effective_absolute_tolerance_before', 'effective_absolute_tolerance_after'):
            response = copy.deepcopy(self.response); response[key][0] *= 10
            with self.assertRaises(ValueError): self.validate(response)

    def test_effective_tolerance_after_is_current_not_initial(self):
        self.assertNotEqual(self.response['effective_absolute_tolerance_before'], self.response['effective_absolute_tolerance_after'])
        self.validate()

    def test_wrong_floating_species_rejected(self):
        self.response['floating_species_ids'] = ['S', 'S']
        with self.assertRaises(ValueError): self.validate()

    def test_both_paths_wrong_cannot_pass_analytic_anchor(self):
        rows = copy.deepcopy(self.response['timeseries'])
        for row in rows: row[1] *= 3  # common amount/concentration conflation
        self.assertEqual(check.compare(rows, self.case['selections'], [dict(zip(self.case['selections'], row)) for row in rows])['status'], 'PASS')
        self.assertEqual(check.compare(rows, self.case['selections'], [anchors.expected_values(self.case['id'], i/10) for i in range(101)])['status'], 'FAIL')

    def test_truncated_comparison_cannot_zip_to_success(self):
        with self.assertRaises(ValueError): check.compare([[0, 1]], ['time','S'], [])

    def test_comparator_direct_nonfinite_shape_and_huge_integer_rejected(self):
        for row in ([0, float('nan')], [0, float('inf')], [0, 10**1000], [0], [0, True]):
            with self.assertRaises(ValueError): check.compare([row], ['time', 'S'], [{'S': 0}])

    def test_absolute_tolerance_near_zero_boundary(self):
        self.assertEqual(check.compare([[0, 1e-8]], ['time','S'], [{'S': 0}])['status'], 'PASS')
        self.assertEqual(check.compare([[0, 1.01e-8]], ['time','S'], [{'S': 0}])['status'], 'FAIL')

    def test_duplicate_json_keys_and_nonfinite_json_rejected(self):
        for text in ('{"a":1,"a":2}', '{"a":NaN}', '{"a":Infinity}'):
            with self.assertRaises(ValueError): check.load_json(text)

    def test_manifest_threshold_and_inventory_drift_rejected(self):
        self.manifest['comparison']['absolute'] = 1
        with self.assertRaises(ValueError): check.validate_manifest(self.manifest)
        self.manifest = anchors.load_manifest(); self.manifest['cases'] = self.manifest['cases'][:-1]
        with self.assertRaises(ValueError): check.validate_manifest(self.manifest)

    def test_source_hash_and_path_escape_rejected(self):
        case = copy.deepcopy(self.case); case['sha256'] = '0'*64
        with self.assertRaises(ValueError): check.source(case)
        case['path'] = '../anything'
        with self.assertRaises(ValueError): check.source(case)

    def test_missing_worker_interpreter_is_blocked(self):
        self.assertEqual(check.simulate(self.xml, self.case['selections'], '/no/such/python')['status'], 'ORACLE_BLOCKED')

    def test_timeout_is_oracle_error(self):
        with patch.object(check.subprocess, 'run', side_effect=subprocess.TimeoutExpired('worker', 60)):
            self.assertEqual(check.simulate(self.xml, self.case['selections'])['status'], 'ORACLE_ERROR')

    def test_malformed_worker_report_is_oracle_error(self):
        with patch.object(check.subprocess, 'run', return_value=subprocess.CompletedProcess([], 0, 'not json', '')):
            self.assertEqual(check.simulate(self.xml, self.case['selections'])['status'], 'ORACLE_ERROR')

    def test_worker_blocked_nonzero_exit_preserves_blocked(self):
        response = {'schema': check.WORKER_SCHEMA, 'status': 'ORACLE_BLOCKED', 'reason': 'missing'}
        with patch.object(check.subprocess, 'run', return_value=subprocess.CompletedProcess([], 1, json.dumps(response), '')):
            self.assertEqual(check.simulate(self.xml, self.case['selections'])['status'], 'ORACLE_BLOCKED')

    def test_missing_native_has_zero_pass_credit(self):
        report = check.check_all(Path('/no/such/native'))
        self.assertEqual(report['status'], 'ORACLE_BLOCKED')
        self.assertEqual(report['counts'], {'passed': 0, 'blocked': 8, 'failed': 0})

    def test_missing_reference_prevents_numerical_pass(self):
        with patch.object(check, 'reference_run', return_value={'status': 'ORACLE_BLOCKED'}):
            report = check.check_case(self.case, Path('/no/such/native'))
        self.assertEqual(report['status'], 'ORACLE_BLOCKED')
        self.assertEqual(report['stages']['native_simulation'], 'NOT_RUN')

    def test_input_mutation_fails_even_when_all_cases_pass(self):
        with patch.object(check, 'fingerprints', side_effect=[{'native':'a'}, {'native':'b'}]), patch.object(check, 'check_case', return_value={'status': 'PASS'}):
            report = check.check_all(Path('unused'))
        self.assertEqual(report['status'], 'FAIL')
        self.assertEqual(report['changed_inputs'], ['native'])

    def test_native_rejection_never_numerical_success(self):
        with patch.object(check, 'reference_run', return_value={'status':'PASS'}), patch.object(check, 'native_export', return_value={'status':'REJECTED'}):
            report = check.check_case(self.case, Path('unused'))
        self.assertEqual(report['status'], 'FAIL')
        self.assertEqual(report['stages']['native_simulation'], 'NOT_RUN')


if __name__ == '__main__':
    unittest.main()
