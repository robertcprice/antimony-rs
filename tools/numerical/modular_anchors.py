"""Two independent analytic trajectories for Phase 6-8 constructs (substanceOnly species, reaction overrides, submodel port/member flattening).

Coefficients and closed-form solutions are hand-derived, never fitted from simulator output.
"""
import hashlib
import json
import math
from pathlib import Path
import anchors as base

ROOT = Path(__file__).resolve().parents[2]
MANIFEST = ROOT / 'fixtures/numerical/modular/manifest.json'
IDS = ('substance_only_reaction_override_decay', 'modular_port_and_member_override_cascade')


def _substance_only_species(name, compartment, amount, boundary=False):
    return dict(
        base._identity(name),
        compartment=compartment,
        initial_amount=float(amount),
        initial_concentration=None,
        has_only_substance_units=True,
        boundary_condition=boundary,
        constant=False,
        substance_units=None,
        conversion_factor=None,
    )


def expected_values(id, t):
    if id not in IDS or type(t) not in (int, float) or not math.isfinite(t) or not 0 <= t <= 10:
        raise ValueError('Unknown modular anchor or invalid time')
    if id == 'substance_only_reaction_override_decay':
        # Compartment V = 2; substanceOnly species S(0) = 4, P(0) = 0.
        # J := k_fast * S overrides k_slow * S with k_fast = 0.5.
        # Because S hasOnlySubstanceUnits="true", bare S in MathML evaluates to amount S(t):
        # dS/dt = -0.5 * S, S(0) = 4 => S(t) = 4 * exp(-0.5 * t), [S](t) = S(t) / 2 = 2 * exp(-0.5 * t)
        # dP/dt = +0.5 * S, P(0) = 0 => P(t) = -4 * expm1(-0.5 * t), [P](t) = P(t) / 2 = -2 * expm1(-0.5 * t)
        s = 4.0 * math.exp(-0.5 * t)
        p = -4.0 * math.expm1(-0.5 * t)
        return {
            'S': s,
            '[S]': 0.5 * s,
            'P': p,
            '[P]': 0.5 * p,
            'k_fast': 0.5,
            'k_slow': 0.1,
            'V': 2.0,
        }
    # Submodel decay_stage(X, Y) instantiated as M: decay_stage(A, B) with M.k = 0.25.
    # Flattens to M_J: A => B with rate M_k * A, A(0) = 6, B(0) = 0, M_k = 0.25, default_compartment = 1.
    a = 6.0 * math.exp(-0.25 * t)
    b = -6.0 * math.expm1(-0.25 * t)
    return {
        'A': a,
        '[A]': a,
        'B': b,
        '[B]': b,
        'M_k': 0.25,
        'default_compartment': 1.0,
    }


def expected_projection(id):
    if id not in IDS:
        raise ValueError('Unknown modular anchor')
    common = {
        'level': 3,
        'version': 2,
        'model_units': dict.fromkeys(
            ['AreaUnits', 'ConversionFactor', 'ExtentUnits', 'LengthUnits', 'SubstanceUnits', 'TimeUnits', 'VolumeUnits']
        ),
        'function_definitions': [],
        'initial_assignments': [],
        'rules': [],
        'events': [],
        'unit_definitions': [],
        'constraints': [],
        'packages': [],
    }
    if id == 'substance_only_reaction_override_decay':
        return {
            **common,
            'model': base._identity('__main'),
            'compartments': [base._compartment('V', 2.0)],
            'species': [
                _substance_only_species('P', 'V', 0.0),
                _substance_only_species('S', 'V', 4.0),
            ],
            'parameters': [
                base._parameter('k_fast', 0.5, True),
                base._parameter('k_slow', 0.1, True),
            ],
            'reactions': [
                base._reaction(
                    'J',
                    [('S', 1)],
                    [('P', 1)],
                    {'type': 42, 'children': [base._name('k_fast'), base._name('S')]},
                )
            ],
        }
    return {
        **common,
        'model': base._identity('main'),
        'compartments': [base._compartment('default_compartment', 1.0)],
        'species': [
            base._species('A', 'default_compartment', 6.0),
            base._species('B', 'default_compartment', 0.0),
        ],
        'parameters': [base._parameter('M_k', 0.25, True)],
        'reactions': [
            base._reaction(
                'M_J',
                [('A', 1)],
                [('B', 1)],
                {'type': 42, 'children': [base._name('M_k'), base._name('A')]},
            )
        ],
    }


def validate_preconditions(id, projection):
    base._same(expected_projection(id), projection)


def invariant_values(id, row):
    expected = expected_values(id, 0)
    if set(row) != {'time', *expected} or any(type(v) not in (int, float) or not math.isfinite(v) for v in row.values()):
        raise ValueError('Invalid modular invariant row')
    if id == 'substance_only_reaction_override_decay':
        return {
            'volume': {'observed': row['V'], 'expected': 2.0},
            'fixed_k_fast': {'observed': row['k_fast'], 'expected': 0.5},
            'fixed_k_slow': {'observed': row['k_slow'], 'expected': 0.1},
            'amount_equals_two_conc_S': {'observed': row['S'] - 2.0 * row['[S]'], 'expected': 0.0},
            'amount_equals_two_conc_P': {'observed': row['P'] - 2.0 * row['[P]'], 'expected': 0.0},
            'mass_conservation': {'observed': row['S'] + row['P'], 'expected': 4.0},
        }
    return {
        'volume': {'observed': row['default_compartment'], 'expected': 1.0},
        'fixed_M_k': {'observed': row['M_k'], 'expected': 0.25},
        'amount_equals_concentration_A': {'observed': row['A'] - row['[A]'], 'expected': 0.0},
        'amount_equals_concentration_B': {'observed': row['B'] - row['[B]'], 'expected': 0.0},
        'mass_conservation': {'observed': row['A'] + row['B'], 'expected': 6.0},
    }


def load_manifest():
    m = json.loads(MANIFEST.read_text())
    if (
        set(m) != {'schema', 'implemented_profile', 'grid', 'comparison', 'solver', 'cases'}
        or m['schema'] != 'antimony-modular-numerical-v1'
        or m['implemented_profile'] != 'modular-core-v0.8-slice'
    ):
        raise ValueError('Unknown modular numerical manifest')
    for k in ('grid', 'comparison', 'solver'):
        base._same(getattr(base, k.upper()), m[k])
    if [r['id'] for r in m['cases']] != list(IDS):
        raise ValueError('Exactly two ordered modular anchors required')
    for r in m['cases']:
        base._same('fixtures/numerical/modular/' + r['id'] + '.ant', r['path'])
        base._same(['time', *expected_values(r['id'], 0)], r['selections'])
        if hashlib.sha256((ROOT / r['path']).read_bytes()).hexdigest() != r['sha256']:
            raise ValueError('Modular numerical source hash drift')
    return m
