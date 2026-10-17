"""Stdlib validation for the explicitly versioned Phase 1 corpus/record contract."""
import json
import re
from pathlib import Path
import profile_overlay

PROFILE_ID = 'kinetic-core-v0.1'
FROZEN_FEATURES = {f'F{i:02}' for i in range(1, 14)} | {f'X{i:02}' for i in range(1, 14)}
REFERENCE_STATUSES = {'PASS', 'REFERENCE_REJECTED', 'SBML_INVALID'}
NATIVE_EXPECTATIONS = {'target_supported', 'strict_profile_rejected', 'malformed', 'unsupported_profile', 'semantic_rejected'}
STAGES = ('syntax', 'resolve', 'export', 'numerical')
STATUSES = {'PASS', 'FAIL', 'UNSUPPORTED_PROFILE', 'ORACLE_BLOCKED', 'NOT_RUN', 'ORACLE_ERROR', 'REFERENCE_REJECTED', 'SBML_INVALID', 'PROJECTION_UNSUPPORTED', 'INPUT_REJECTED'}

def validate_manifest(manifest, profile):
    if manifest.get('schema') != 'antimony-rs-corpus-v1' or manifest.get('profile_id') != PROFILE_ID:
        raise ValueError('Unsupported corpus schema/profile')
    if profile.get('profile_id') != PROFILE_ID or {f['id'] for f in profile['features']} != FROZEN_FEATURES:
        raise ValueError('Frozen profile feature IDs changed; review/version the oracle contract')
    fixtures = manifest.get('fixtures')
    if not isinstance(fixtures, list) or not fixtures:
        raise ValueError('Corpus requires nonempty fixture list')
    seen = set()
    for fixture in fixtures:
        required = {'id', 'path', 'sha256', 'expectation', 'feature_ids', 'semantic_tags', 'native_profile_expectation', 'reference_status', 'provenance', 'license', 'numerical_applicability'}
        if set(fixture) != required:
            raise ValueError('Fixture fields do not match corpus-v1 schema')
        if not isinstance(fixture['id'], str) or not re.fullmatch(r'[a-z][a-z0-9_]*', fixture['id']) or fixture['id'] in seen:
            raise ValueError('Invalid/duplicate fixture ID')
        seen.add(fixture['id'])
        for key in required - {'feature_ids', 'semantic_tags'}:
            if not isinstance(fixture[key], str) or not fixture[key]:
                raise ValueError('Fixture field must be a nonempty string: ' + key)
        if not re.fullmatch('[0-9a-f]{64}', fixture['sha256']):
            raise ValueError('Invalid fixture SHA-256')
        for key in ('feature_ids', 'semantic_tags'):
            if not isinstance(fixture[key], list) or not fixture[key] or not all(isinstance(x, str) and x for x in fixture[key]):
                raise ValueError('Invalid fixture tag list: ' + key)
            if len(set(fixture[key])) != len(fixture[key]):
                raise ValueError('Duplicate feature/tag')
        if not set(fixture['feature_ids']) <= FROZEN_FEATURES:
            raise ValueError('Unknown frozen profile feature ID')
        if fixture['reference_status'] not in REFERENCE_STATUSES or fixture['native_profile_expectation'] not in NATIVE_EXPECTATIONS:
            raise ValueError('Invalid expected reference/native status')
        if fixture['reference_status'] == 'SBML_INVALID' and fixture['native_profile_expectation'] != 'strict_profile_rejected':
            raise ValueError('Invalid upstream SBML can only require strict native rejection')

def validate_record(record):
    required = {'schema', 'profile_id', 'fixture', 'feature_ids', 'source_sha256', 'status', 'reference_status', 'native_stages'}
    if not required <= set(record):
        raise ValueError('Missing compatibility fields')
    if not isinstance(record['fixture'], str) or not re.fullmatch(r'[a-z][a-z0-9_]*', record['fixture']):
        raise ValueError('Invalid compatibility fixture ID')
    if not isinstance(record['source_sha256'], str) or not re.fullmatch('[0-9a-f]{64}', record['source_sha256']):
        raise ValueError('Invalid compatibility SHA-256')
    if not isinstance(record['feature_ids'], list) or not all(isinstance(x, str) for x in record['feature_ids']) or len(set(record['feature_ids'])) != len(record['feature_ids']):
        raise ValueError('Invalid compatibility features')
    if record.get('schema') != 'antimony-rs-compatibility-v1' or record.get('profile_id') != PROFILE_ID:
        raise ValueError('Unsupported compatibility schema/profile')
    if 'implemented_profile' in record:
        if record['implemented_profile'] != profile_overlay.PROFILE or record.get('corpus_profile_id') != PROFILE_ID or not record.get('profile_scope'):
            raise ValueError('Expanded implementation metadata must distinguish historical corpus scope')
    if record.get('reference_status') not in STATUSES or record.get('status') not in STATUSES:
        raise ValueError('Unknown compatibility status')
    if set(record.get('native_stages', {})) != set(STAGES):
        raise ValueError('Compatibility record requires each native stage')
    if any(x not in STATUSES for x in record['native_stages'].values()):
        raise ValueError('Unknown native stage status')
    if not record.get('feature_ids') or not set(record['feature_ids']) <= FROZEN_FEATURES:
        raise ValueError('Unknown compatibility feature ID')
