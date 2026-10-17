"""Two independent analytic trajectories for user-defined functions (FunctionDefinition / <lambda>).

Coefficients and closed-form solutions are hand-derived, never fitted from simulator output.
"""
import hashlib
import json
import math
from pathlib import Path
import anchors as base

ROOT = Path(__file__).resolve().parents[2]
MANIFEST = ROOT / 'fixtures/numerical/functions/manifest.json'
IDS = ('function_kinetic_law_and_observable', 'nested_function_rate_and_boundary_rule')


def _fn_def(id, params, body_math):
    return {
        **base._identity(id),
        'math': {
            'type': 267,
            'children': [*[base._name(p) for p in params], body_math],
        },
    }


def _user_call(name, children=None):
    node = {'type': 268, 'name': name}
    if children:
        node['children'] = children
    return node


def expected_values(id, t):
    if id not in IDS or type(t) not in (int, float) or not math.isfinite(t) or not 0 <= t <= 10:
        raise ValueError('Unknown function anchor or invalid time')
    if id == 'function_kinetic_law_and_observable':
        # lin_rate(k, x) = k * x, affine_obs(m, x, b) = m * x + b
        # dS/dt = -lin_rate(k, S) = -0.5 * S, S(0) = 4 => S(t) = 4 * exp(-0.5 * t)
        # dP/dt = +lin_rate(k, S) = 0.5 * S, P(0) = 0 => P(t) = -4 * expm1(-0.5 * t)
        # y := affine_obs(2, S, 1) => y(t) = 2 * S(t) + 1 = 8 * exp(-0.5 * t) + 1
        s = 4.0 * math.exp(-0.5 * t)
        p = -4.0 * math.expm1(-0.5 * t)
        y = 8.0 * math.exp(-0.5 * t) + 1.0
        return {'S': s, '[S]': s, 'P': p, '[P]': p, 'y': y, 'k': 0.5, 'default_compartment': 1.0}
    # base_drive() = 1, ramp(t_val, slope) = base_drive() + slope * t_val
    # du/dt = base_drive() = 1, u(0) = 0 => u(t) = t
    # B := ramp(u, 0.2) = 1 + 0.2 * t
    # dS/dt = B, S(0) = 0 => S(t) = t + 0.1 * t^2
    u = float(t)
    b = 1.0 + 0.2 * t
    s = t + 0.1 * t * t
    return {'S': s, '[S]': s, 'B': b, '[B]': b, 'u': u, 'default_compartment': 1.0}


def expected_projection(id):
    if id not in IDS:
        raise ValueError('Unknown function anchor')
    common = {
        'level': 3,
        'version': 2,
        'model': base._identity('__main'),
        'model_units': dict.fromkeys(['AreaUnits', 'ConversionFactor', 'ExtentUnits', 'LengthUnits', 'SubstanceUnits', 'TimeUnits', 'VolumeUnits']),
        'compartments': [base._compartment('default_compartment', 1)],
        'initial_assignments': [],
        'events': [],
        'unit_definitions': [],
        'constraints': [],
        'packages': [],
    }
    if id == 'function_kinetic_law_and_observable':
        return {
            **common,
            'function_definitions': [
                _fn_def('lin_rate', ['k', 'x'], {'type': 42, 'children': [base._name('k'), base._name('x')]}),
                _fn_def(
                    'affine_obs',
                    ['m', 'x', 'b'],
                    {
                        'type': 43,
                        'children': [
                            {'type': 42, 'children': [base._name('m'), base._name('x')]},
                            base._name('b'),
                        ],
                    },
                ),
            ],
            'species': [
                base._species('P', 'default_compartment', 0),
                base._species('S', 'default_compartment', 4),
            ],
            'parameters': [
                base._parameter('k', 0.5, True),
                base._parameter('y', None, False),
            ],
            'reactions': [
                base._reaction(
                    'J',
                    [('S', 1)],
                    [('P', 1)],
                    _user_call('lin_rate', [base._name('k'), base._name('S')]),
                )
            ],
            'rules': [
                {
                    'kind': 'assignmentRule',
                    'variable': 'y',
                    'math': _user_call(
                        'affine_obs',
                        [{'type': 256, 'integer': 2}, base._name('S'), {'type': 256, 'integer': 1}],
                    ),
                },
            ],
        }
    return {
        **common,
        'function_definitions': [
            _fn_def('base_drive', [], {'type': 256, 'integer': 1}),
            _fn_def(
                'ramp',
                ['t_val', 'slope'],
                {
                    'type': 43,
                    'children': [
                        _user_call('base_drive'),
                        {'type': 42, 'children': [base._name('slope'), base._name('t_val')]},
                    ],
                },
            ),
        ],
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
                'math': _user_call('ramp', [base._name('u'), {'type': 257, 'real': 0.2}]),
            },
            {
                'kind': 'rateRule',
                'variable': 'u',
                'math': _user_call('base_drive'),
            },
        ],
    }


def validate_preconditions(id, projection):
    base._same(expected_projection(id), projection)


def invariant_values(id, row):
    expected = expected_values(id, 0)
    if set(row) != {'time', *expected} or any(type(v) not in (int, float) or not math.isfinite(v) for v in row.values()):
        raise ValueError('Invalid functions invariant row')
    if id == 'function_kinetic_law_and_observable':
        return {
            'volume': {'observed': row['default_compartment'], 'expected': 1.0},
            'fixed_k': {'observed': row['k'], 'expected': 0.5},
            'amount_equals_concentration_S': {'observed': row['S'] - row['[S]'], 'expected': 0.0},
            'amount_equals_concentration_P': {'observed': row['P'] - row['[P]'], 'expected': 0.0},
            'affine_observable': {'observed': row['y'] - (2.0 * row['S'] + 1.0), 'expected': 0.0},
            'mass_conservation': {'observed': row['S'] + row['P'], 'expected': 4.0},
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
    if set(m) != {'schema', 'implemented_profile', 'grid', 'comparison', 'solver', 'cases'} or m['schema'] != 'antimony-functions-numerical-v1' or m['implemented_profile'] != 'functions-core-v0.5-slice':
        raise ValueError('Unknown functions numerical manifest')
    for k in ('grid', 'comparison', 'solver'):
        base._same(getattr(base, k.upper()), m[k])
    if [r['id'] for r in m['cases']] != list(IDS):
        raise ValueError('Exactly two ordered functions anchors required')
    for r in m['cases']:
        base._same('fixtures/numerical/functions/' + r['id'] + '.ant', r['path'])
        base._same(['time', *expected_values(r['id'], 0)], r['selections'])
        if hashlib.sha256((ROOT / r['path']).read_bytes()).hexdigest() != r['sha256']:
            raise ValueError('Functions numerical source hash drift')
    return m
