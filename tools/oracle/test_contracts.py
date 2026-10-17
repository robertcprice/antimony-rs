import copy
import json
import unittest
from contracts import validate_manifest, validate_record
from oracle import ROOT

class ContractTests(unittest.TestCase):
    def setUp(self):
        self.manifest = json.loads((ROOT / 'fixtures/manifests/original-v1.json').read_text())
        self.profile = json.loads((ROOT / 'profiles/kinetic-core-v0.1.json').read_text())
        self.record = {'schema':'antimony-rs-compatibility-v1','profile_id':'kinetic-core-v0.1', 'fixture':'example', 'feature_ids':['F07'], 'source_sha256':'a'*64, 'status':'PASS','reference_status':'PASS','native_stages':{x:'NOT_RUN' for x in ('syntax','resolve','export','numerical')}}

    def test_current_manifest_and_record(self):
        validate_manifest(self.manifest, self.profile)
        validate_record(self.record)

    def test_unknown_feature(self):
        self.manifest['fixtures'][0]['feature_ids'] = ['reaction.irreversible']
        with self.assertRaises(ValueError): validate_manifest(self.manifest, self.profile)

    def test_duplicate_fixture(self):
        self.manifest['fixtures'].append(copy.deepcopy(self.manifest['fixtures'][0]))
        with self.assertRaises(ValueError): validate_manifest(self.manifest, self.profile)

    def test_bad_hash(self):
        self.manifest['fixtures'][0]['sha256'] = 'bad'
        with self.assertRaises(ValueError): validate_manifest(self.manifest, self.profile)

    def test_changed_frozen_profile(self):
        self.profile['features'].pop()
        with self.assertRaises(ValueError): validate_manifest(self.manifest, self.profile)

    def test_missing_stage(self):
        del self.record['native_stages']['numerical']
        with self.assertRaises(ValueError): validate_record(self.record)

    def test_invalid_status(self):
        self.record['reference_status'] = 'maybe'
        with self.assertRaises(ValueError): validate_record(self.record)

    def test_missing_record_field(self):
        del self.record['source_sha256']
        with self.assertRaises(ValueError): validate_record(self.record)

    def test_invalid_reference_only_allows_strict_rejection(self):
        f = self.manifest['fixtures'][0]
        f['reference_status'] = 'SBML_INVALID'
        with self.assertRaises(ValueError): validate_manifest(self.manifest, self.profile)
        f['native_profile_expectation'] = 'strict_profile_rejected'
        validate_manifest(self.manifest, self.profile)
