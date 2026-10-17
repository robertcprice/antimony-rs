"""Explicit current native-outcome overlay; immutable historical reference contracts."""
import copy
import json
import os
from oracle import ROOT, digest

KNOWN_PROFILES = {
    'modular-core-v0.8-slice': ('fixtures/manifests/modular-core-v8-overlay.json', 'antimony-native-overlay-v8'),
    'functions-core-v0.5-slice': ('fixtures/manifests/functions-core-v5-overlay.json', 'antimony-native-overlay-v7'),
    'rules-core-v0.4-slice': ('fixtures/manifests/rules-core-v4-overlay.json', 'antimony-native-overlay-v6'),
    'conditional-core-v0.3-slice': ('fixtures/manifests/conditional-core-v3-overlay.json', 'antimony-native-overlay-v5'),
    'math-core-v0.2-slice': ('fixtures/manifests/math-core-v2-overlay.json', 'antimony-native-overlay-v3'),
    'math-core-v0.2.1-slice': ('fixtures/manifests/math-core-v2_1-overlay.json', 'antimony-native-overlay-v4'),
}

def configuration(value):
    if value not in KNOWN_PROFILES:
        raise ValueError('Unknown implementation profile configuration')
    path, schema = KNOWN_PROFILES[value]
    return ROOT / path, schema

PROFILE = os.environ.get('ANTIMONY_IMPLEMENTATION_PROFILE', 'modular-core-v0.8-slice')
PATH, OVERLAY_SCHEMA = configuration(PROFILE)


def load():
    value = json.loads(PATH.read_text())
    expected_keys = {'schema', 'implemented_profile', 'sources', 'original_overrides', 'workload_overrides', 'arithmetic_overrides'}
    if PROFILE == 'modular-core-v0.8-slice':
        expected_keys |= {
            'symbols_overrides',
            'conditionals_overrides',
            'rules_overrides',
            'functions_overrides',
            'collaboration_overrides',
        }
    if set(value) != expected_keys or value['schema'] != OVERLAY_SCHEMA or value['implemented_profile'] != PROFILE:
        raise ValueError('Unknown current-profile overlay contract')
    source_inventory = {'fixtures/manifests/original-v1.json', 'fixtures/manifests/arithmetic-v1.json', 'tools/workloads/generated-v1.json', 'tools/workloads/generated_contracts.py', 'fixtures/manifests/flat-model-v2-overlay.json'}
    if PROFILE in ('math-core-v0.2.1-slice', 'conditional-core-v0.3-slice', 'rules-core-v0.4-slice', 'functions-core-v0.5-slice', 'modular-core-v0.8-slice'):
        source_inventory.add('fixtures/manifests/math-core-v2-overlay.json')
    if PROFILE in ('conditional-core-v0.3-slice', 'rules-core-v0.4-slice', 'functions-core-v0.5-slice', 'modular-core-v0.8-slice'):
        source_inventory |= {'fixtures/manifests/math-core-v2_1-overlay.json', 'fixtures/manifests/builtins-v2.json', 'fixtures/manifests/symbols-v2.json', 'fixtures/rational-calls/corrective-v1.json'}
    if PROFILE in ('rules-core-v0.4-slice', 'functions-core-v0.5-slice', 'modular-core-v0.8-slice'):
        source_inventory |= {'fixtures/manifests/conditional-core-v3-overlay.json', 'fixtures/conditionals/manifest.json', 'fixtures/conditionals/aliases-v1.json'}
    if PROFILE in ('functions-core-v0.5-slice', 'modular-core-v0.8-slice'):
        source_inventory |= {'fixtures/manifests/rules-core-v4-overlay.json', 'fixtures/rules/manifest.json'}
    if PROFILE == 'modular-core-v0.8-slice':
        source_inventory |= {'fixtures/manifests/functions-core-v5-overlay.json', 'fixtures/functions/manifest.json', 'fixtures/collaboration/manifest.json'}
    if set(value['sources']) != source_inventory:
        raise ValueError('Historical overlay source inventory changed')
    expected_original = {'absent_values', 'init_unknown', 'formula_uninitialized', 'formula_duplicate', 'formula_species_promotion', 'formula_var_qualifier', 'formula_dollar_qualifier', 'function_call'}
    if PROFILE in ('rules-core-v0.4-slice', 'functions-core-v0.5-slice', 'modular-core-v0.8-slice'):
        expected_original |= {'assignment_rule', 'formula_assignment_rule'}
    if PROFILE == 'modular-core-v0.8-slice':
        expected_original |= {'event', 'init_compartment_expression'}
    if set(value['original_overrides']) != expected_original:
        raise ValueError('Unexpected original override inventory')
    for path, sha in value['sources'].items():
        source = (ROOT / path).resolve()
        if not source.is_relative_to(ROOT) or digest(source.read_bytes()) != sha:
            raise ValueError('Historical overlay source hash changed')
    parent_map = {
        'math-core-v0.2-slice': 'fixtures/manifests/math-core-v2-overlay.json',
        'math-core-v0.2.1-slice': 'fixtures/manifests/math-core-v2-overlay.json',
        'conditional-core-v0.3-slice': 'fixtures/manifests/math-core-v2_1-overlay.json',
        'rules-core-v0.4-slice': 'fixtures/manifests/conditional-core-v3-overlay.json',
        'functions-core-v0.5-slice': 'fixtures/manifests/rules-core-v4-overlay.json',
        'modular-core-v0.8-slice': 'fixtures/manifests/functions-core-v5-overlay.json',
    }
    parent = json.loads((ROOT / parent_map[PROFILE]).read_text())
    for key in ('workload_overrides', 'arithmetic_overrides'):
        if value[key] != parent[key]:
            raise ValueError(f'Current {key} must preserve parent corpus outcomes')
    if PROFILE in ('math-core-v0.2.1-slice', 'conditional-core-v0.3-slice', 'functions-core-v0.5-slice') and value['original_overrides'] != parent['original_overrides']:
        raise ValueError('Current original_overrides must preserve parent corpus outcomes')
    if PROFILE in ('rules-core-v0.4-slice', 'modular-core-v0.8-slice'):
        inherited = {k: v for k, v in value['original_overrides'].items() if k in parent['original_overrides']}
        if inherited != parent['original_overrides']:
            raise ValueError('Current original_overrides must preserve parent corpus outcomes')
    original = json.loads((ROOT / 'fixtures/manifests/original-v1.json').read_text())
    rows = {r['id']: r for r in original['fixtures']}
    for id, change in value['original_overrides'].items():
        if id not in rows or set(change) != {'from', 'to', 'reason'} or change['from'] != rows[id]['native_profile_expectation'] or change['to'] != 'target_supported' or rows[id]['reference_status'] != 'PASS' or not change['reason']:
            raise ValueError('Invalid original outcome override')
    if value['workload_overrides'] != {f'grid_unset_{i:02}': {'from':'REJECTED','to':'EXPORTED'} for i in range(4)}:
        raise ValueError('Unexpected workload outcome overrides')
    changes = value['arithmetic_overrides']
    if set(changes) != {'deferred_builtin'}:
        raise ValueError('Unexpected arithmetic override inventory')
    change = changes['deferred_builtin']
    if set(change) != {'from','diagnostic_code_from','to','reason'} or change['from'] != 'rejected' or change['diagnostic_code_from'] != 'unsupported_feature' or change['to'] != 'accepted' or not change['reason']:
        raise ValueError('Invalid arithmetic override')
    if PROFILE == 'modular-core-v0.8-slice':
        _validate_rejection_to_acceptance_overrides(
            value['symbols_overrides'],
            'fixtures/manifests/symbols-v2.json',
            {'deferred_compartment_expression'},
        )
        _validate_rejection_to_acceptance_overrides(
            value['conditionals_overrides'],
            'fixtures/conditionals/manifest.json',
            {'conditional_compartment_deferred'},
        )
        _validate_rejection_to_acceptance_overrides(
            value['rules_overrides'],
            'fixtures/rules/manifest.json',
            {
                'assignment_compartment',
                'assignment_compartment_after',
                'assignment_compartment_reaction',
                'assignment_reaction_target_before',
                'assignment_reaction_target_after',
                'rate_compartment',
                'rate_compartment_after',
                'rate_compartment_reaction',
            },
        )
        _validate_rejection_to_acceptance_overrides(
            value['functions_overrides'],
            'fixtures/functions/manifest.json',
            {
                'reject_compartment_init_via_fn_call',
                'reject_compartment_rule_via_fn_call',
                'reject_reaction_assign_via_fn_call',
            },
        )
        _validate_collaboration_overrides(
            value['collaboration_overrides'],
            'fixtures/collaboration/manifest.json',
            {
                'neg_deferred_event_trigger',
                'neg_deferred_dynamic_compartment_assignment_rule',
                'neg_deferred_dynamic_compartment_rate_rule',
                'neg_deferred_reaction_id_assignment_rule',
            },
        )
    return value


def _validate_rejection_to_acceptance_overrides(overrides, manifest_relpath, expected_ids):
    if set(overrides) != expected_ids:
        raise ValueError(f'Unexpected override inventory for {manifest_relpath}')
    manifest = json.loads((ROOT / manifest_relpath).read_text())
    rows = {r['id']: r for r in manifest['fixtures']}
    for id, change in overrides.items():
        if (
            id not in rows
            or set(change) != {'from', 'diagnostic_code_from', 'to', 'reason'}
            or change['from'] != 'rejected'
            or change['diagnostic_code_from'] != 'unsupported_feature'
            or change['to'] != 'accepted'
            or rows[id]['native'] != 'rejected'
            or rows[id]['diagnostic_code'] != 'unsupported_feature'
            or rows[id]['reference_status'] != 'PASS'
            or not change['reason']
        ):
            raise ValueError(f'Invalid override for {manifest_relpath}: {id}')


def _validate_collaboration_overrides(overrides, manifest_relpath, expected_ids):
    if set(overrides) != expected_ids:
        raise ValueError(f'Unexpected collaboration override inventory for {manifest_relpath}')
    manifest = json.loads((ROOT / manifest_relpath).read_text())
    rows = {r['id']: r for r in manifest['fixtures']}
    for id, change in overrides.items():
        if (
            id not in rows
            or set(change) != {'from', 'readiness_from', 'diagnostic_code_from', 'to', 'readiness_to', 'reason'}
            or change['from'] != 'rejected'
            or change['readiness_from'] != 'rejected'
            or change['diagnostic_code_from'] != 'unsupported_feature'
            or change['to'] != 'accepted'
            or change['readiness_to'] != 'complete'
            or rows[id]['native'] != 'rejected'
            or rows[id]['readiness'] != 'rejected'
            or rows[id]['diagnostic_code'] != 'unsupported_feature'
            or rows[id]['reference_status'] != 'PASS'
            or not change['reason']
        ):
            raise ValueError(f'Invalid collaboration override: {id}')


def original(manifest):
    overlay = load()
    result = copy.deepcopy(manifest)
    expected = json.loads((ROOT / 'fixtures/manifests/original-v1.json').read_text())
    if result != expected:
        raise ValueError('Overlay requires exact historical original manifest')
    for row in result['fixtures']:
        change = overlay['original_overrides'].get(row['id'])
        if change:
            row['native_profile_expectation'] = change['to']
    return result


def workload(case, contract):
    overlay = load()
    result = copy.deepcopy(contract)
    change = overlay['workload_overrides'].get(case['id'])
    if change:
        if result.get('reference_status') != 'PASS' or result.get('native_status') != change['from'] or case['recipe']['family'] != 'unset':
            raise ValueError('Workload override precondition changed')
        result['native_status'] = change['to']
        del result['native_stage'], result['native_code']
    return result


def arithmetic(manifest):
    overlay = load()
    expected = json.loads((ROOT / 'fixtures/manifests/arithmetic-v1.json').read_text())
    if manifest != expected:
        raise ValueError('Arithmetic overlay requires exact historical manifest')
    result = copy.deepcopy(manifest)
    for row in result['fixtures']:
        change = overlay['arithmetic_overrides'].get(row['id'])
        if change:
            if row['native'] != change['from'] or row['diagnostic_code'] != change['diagnostic_code_from'] or row['reference_status'] != 'PASS':
                raise ValueError('Arithmetic override precondition changed')
            row.update(native=change['to'], diagnostic_code=None)
    return result


def symbols(manifest):
    overlay = load()
    expected = json.loads((ROOT / 'fixtures/manifests/symbols-v2.json').read_text())
    if manifest != expected:
        raise ValueError('Symbols overlay requires exact historical manifest')
    result = copy.deepcopy(manifest)
    for row in result['fixtures']:
        change = overlay.get('symbols_overrides', {}).get(row['id'])
        if change:
            if row['native'] != change['from'] or row['diagnostic_code'] != change['diagnostic_code_from'] or row['reference_status'] != 'PASS':
                raise ValueError('Symbols override precondition changed')
            row.update(native=change['to'], diagnostic_code=None)
    return result


def conditionals(manifest):
    overlay = load()
    result = copy.deepcopy(manifest)
    for row in result['fixtures']:
        change = overlay.get('conditionals_overrides', {}).get(row['id'])
        if change:
            if row['native'] != change['from'] or row['diagnostic_code'] != change['diagnostic_code_from'] or row['reference_status'] != 'PASS':
                raise ValueError('Conditionals override precondition changed')
            row.update(native=change['to'], diagnostic_code=None)
    return result


def rules(manifest):
    overlay = load()
    expected = json.loads((ROOT / 'fixtures/rules/manifest.json').read_text())
    if manifest != expected:
        raise ValueError('Rules overlay requires exact historical manifest')
    result = copy.deepcopy(manifest)
    for row in result['fixtures']:
        change = overlay.get('rules_overrides', {}).get(row['id'])
        if change:
            if row['native'] != change['from'] or row['diagnostic_code'] != change['diagnostic_code_from'] or row['reference_status'] != 'PASS':
                raise ValueError('Rules override precondition changed')
            row.update(native=change['to'], diagnostic_code=None)
    return result


def functions(manifest):
    overlay = load()
    expected = json.loads((ROOT / 'fixtures/functions/manifest.json').read_text())
    if manifest != expected:
        raise ValueError('Functions overlay requires exact historical manifest')
    result = copy.deepcopy(manifest)
    for row in result['fixtures']:
        change = overlay.get('functions_overrides', {}).get(row['id'])
        if change:
            if row['native'] != change['from'] or row['diagnostic_code'] != change['diagnostic_code_from'] or row['reference_status'] != 'PASS':
                raise ValueError('Functions override precondition changed')
            row.update(native=change['to'], diagnostic_code=None)
    return result


def collaboration(manifest):
    overlay = load()
    expected = json.loads((ROOT / 'fixtures/collaboration/manifest.json').read_text())
    if manifest != expected:
        raise ValueError('Collaboration overlay requires exact historical manifest')
    result = copy.deepcopy(manifest)
    for row in result['fixtures']:
        change = overlay.get('collaboration_overrides', {}).get(row['id'])
        if change:
            if (
                row['native'] != change['from']
                or row['readiness'] != change['readiness_from']
                or row['diagnostic_code'] != change['diagnostic_code_from']
                or row['reference_status'] != 'PASS'
            ):
                raise ValueError('Collaboration override precondition changed')
            row.update(native=change['to'], readiness=change['readiness_to'], diagnostic_code=None)
    return result


def boundary_path():
    return ROOT / {
        'math-core-v0.2-slice': 'profiles/math-core-v0.2.json',
        'math-core-v0.2.1-slice': 'profiles/math-core-v0.2.1.json',
        'conditional-core-v0.3-slice': 'profiles/conditional-core-v0.3.json',
        'rules-core-v0.4-slice': 'profiles/rules-core-v0.4.json',
        'functions-core-v0.5-slice': 'profiles/functions-core-v0.5.json',
        'modular-core-v0.8-slice': 'profiles/modular-core-v0.8.json',
    }[PROFILE]
