"""Two independent analytic trajectories for coupled assignment (:=) and rate ('=) rules.

Coefficients and closed-form solutions are hand-derived, never fitted from simulator output.
"""
import hashlib
import json
import math
from pathlib import Path
import anchors as base

ROOT = Path(__file__).resolve().parents[2]
MANIFEST = ROOT / 'fixtures/numerical/rules/manifest.json'
IDS = ('coupled_rate_and_assignment_rule', 'boundary_species_assignment_and_rate_rule')


def expected_values(id, t):
    if id not in IDS or type(t) not in (int, float) or not math.isfinite(t) or not 0 <= t <= 10:
        raise ValueError('Unknown rule anchor or invalid time')
    if id == 'coupled_rate_and_assignment_rule':
        # dx/dt = -0.5 * x, x(0) = 4 => x(t) = 4 * exp(-0.5 * t)
        # y := 2 * x => y(t) = 8 * exp(-0.5 * t)
        # dS/dt = y, S(0) = 0 => S(t) = 16 * (1 - exp(-0.5 * t)) = -16 * expm1(-0.5 * t)
        x = 4.0 * math.exp(-0.5 * t)
        y = 8.0 * math.exp(-0.5 * t)
        s = -16.0 * math.expm1(-0.5 * t)
        return {'S': s, '[S]': s, 'x': x, 'y': y, 'k': 0.5, 'default_compartment': 1.0}
    # du/dt = 1, u(0) = 0 => u(t) = t
    # B := 1 + 0.2 * u => B(t) = 1 + 0.2 * t
    # dS/dt = B, S(0) = 0 => S(t) = t + 0.1 * t^2
    u = float(t)
    b = 1.0 + 0.2 * t
    s = t + 0.1 * t * t
    return {'S': s, '[S]': s, 'B': b, '[B]': b, 'u': u, 'default_compartment': 1.0}


def expected_projection(id):
    if id not in IDS:
        raise ValueError('Unknown rule anchor')
    common = {
        'level': 3,
        'version': 2,
        'model': base._identity('__main'),
        'model_units': dict.fromkeys(['AreaUnits', 'ConversionFactor', 'ExtentUnits', 'LengthUnits', 'SubstanceUnits', 'TimeUnits', 'VolumeUnits']),
        'compartments': [base._compartment('default_compartment', 1)],
        'initial_assignments': [],
        'events': [],
        'function_definitions': [],
        'unit_definitions': [],
        'constraints': [],
        'packages': [],
    }
    if id == 'coupled_rate_and_assignment_rule':
        return {
            **common,
            'species': [base._species('S', 'default_compartment', 0)],
            'parameters': [
                base._parameter('k', 0.5, True),
                base._parameter('x', 4.0, False),
                base._parameter('y', None, False),
            ],
            'reactions': [base._reaction('J', [], [('S', 1)], base._name('y'))],
            'rules': [
                {
                    'kind': 'rateRule',
                    'variable': 'x',
                    'math': {'type': 42, 'children': [{'type': 45, 'children': [base._name('k')]}, base._name('x')]},
                },
                {
                    'kind': 'assignmentRule',
                    'variable': 'y',
                    'math': {'type': 42, 'children': [{'type': 256, 'integer': 2}, base._name('x')]},
                },
            ],
        }
    return {
        **common,
        'species': [
            base._species('B', 'default_compartment', None, True),
            base._species('S', 'default_compartment', 0, False),
        ],
        'parameters': [base._parameter('u', 0.0, False)],
        'reactions': [base._reaction('J', [('B', 1)], [('S', 1)], base._name('B'))],
        'rules': [
            {
                'kind': 'assignmentRule',
                'variable': 'B',
                'math': {
                    'type': 43,
                    'children': [
                        {'type': 256, 'integer': 1},
                        {'type': 42, 'children': [{'type': 257, 'real': 0.2}, base._name('u')]},
                    ],
                },
            },
            {
                'kind': 'rateRule',
                'variable': 'u',
                'math': {'type': 256, 'integer': 1},
            },
        ],
    }


def validate_preconditions(id, projection):
    base._same(expected_projection(id), projection)


def invariant_values(id, row):
    expected = expected_values(id, 0)
    if set(row) != {'time', *expected} or any(type(v) not in (int, float) or not math.isfinite(v) for v in row.values()):
        raise ValueError('Invalid rules invariant row')
    if id == 'coupled_rate_and_assignment_rule':
        return {
            'volume': {'observed': row['default_compartment'], 'expected': 1.0},
            'fixed_k': {'observed': row['k'], 'expected': 0.5},
            'amount_equals_concentration': {'observed': row['S'] - row['[S]'], 'expected': 0.0},
            'assignment_relation': {'observed': row['y'] - 2.0 * row['x'], 'expected': 0.0},
            'coupled_balance': {'observed': row['S'] + 4.0 * row['x'], 'expected': 16.0},
        }
    return {
        'volume': {'observed': row['default_compartment'], 'expected': 1.0},
        'amount_equals_concentration_S': {'observed': row['S'] - row['[S]'], 'expected': 0.0},
        'amount_equals_concentration_B': {'observed': row['B'] - row['[B]'], 'expected': 0.0},
        'boundary_assignment': {'observed': row['B'] - (1.0 + 0.2 * row['u']), 'expected': 0.0},
        'quadratic_integral': {'observed': row['S'] - (row['u'] + 0.1 * row['u'] * row['u']), 'expected': 0.0},
    }


def load_manifest():
    m = json.loads(MANIFEST.read_text())
    if set(m) != {'schema', 'implemented_profile', 'grid', 'comparison', 'solver', 'cases'} or m['schema'] != 'antimony-rules-numerical-v1' or m['implemented_profile'] != 'rules-core-v0.4-slice':
        raise ValueError('Unknown rules numerical manifest')
    for k in ('grid', 'comparison', 'solver'):
        base._same(getattr(base, k.upper()), m[k])
    if [r['id'] for r in m['cases']] != list(IDS):
        raise ValueError('Exactly two ordered rules anchors required')
    for r in m['cases']:
        base._same('fixtures/numerical/rules/' + r['id'] + '.ant', r['path'])
        base._same(['time', *expected_values(r['id'], 0)], r['selections'])
        if hashlib.sha256((ROOT / r['path']).read_bytes()).hexdigest() != r['sha256']:
            raise ValueError('Rules numerical source hash drift')
    return m
