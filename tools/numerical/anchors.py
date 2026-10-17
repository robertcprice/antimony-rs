"""Independent, hand-derived analytic anchors; stdlib only, never a simulator.

Equations and structural preconditions are reviewed together in NUMERICAL_MODELS.md.
No coefficient is inferred from a simulator, SBML, or observed trajectory.
"""
from __future__ import annotations
import hashlib
import json
import math
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
MANIFEST = ROOT / 'fixtures/numerical/manifest.json'
IDS = ('units_decay', 'reversible', 'nonunit_no_factor', 'boundary',
       'source_drain', 'forward_initial', 'stoichiometry', 'unequal_transfer')
GRID = {'start': 0, 'end': 10, 'steps': 100}
COMPARISON = {'absolute': 1e-8, 'relative': 1e-7}
SOLVER = {'name': 'cvode', 'relative_tolerance': 1e-10,
          'absolute_tolerance': 1e-12, 'maximum_num_steps': 100000,
          'variable_step_size': False, 'stiff': True}
CONSTANTS = {
    'units_decay': {'volume': 1, 'S0': 3, 'k': .4},
    'reversible': {'volume': 1, 'A0': 3, 'B0': 1, 'kf': .3, 'kr': .1},
    'nonunit_no_factor': {'volume': 3, 'S0': 2, 'P0': 0, 'k': .6},
    'boundary': {'volume': 1, 'B0': 2, 'P0': 1, 'k': .3},
    'source_drain': {'volume': 1, 'S0': 0, 'q': .6, 'k': .2},
    'forward_initial': {'volume': 1, 'S0': 3, 'base': 2, 'delta': 1,
                        'p': .8, 'k': .4, 'held': 3},
    'stoichiometry': {'volume': 1, 'A0': 4, 'B0': 0, 'k': .2,
                      'reactant_stoichiometry': 2},
    'unequal_transfer': {'source_volume': 2, 'product_volume': 5,
                         'A0': 3, 'B0': 1, 'k': .4},
}


def _finite(value):
    try:
        return type(value) in (int, float) and math.isfinite(value)
    except OverflowError:
        return False


def expected_values(anchor_id, t):
    """All non-time selections at t in [0,10], amount S versus concentration [S]."""
    if anchor_id not in IDS or not _finite(t) or not 0 <= t <= 10:
        raise ValueError('unknown anchor or invalid/out-of-grid time')
    # Explicit coefficients below are mathematics, not projection-derived values.
    if anchor_id == 'units_decay':
        s = 3 * math.exp(-.4 * t)
        return {'S': s, '[S]': s, 'cell': 1., 'k': .4}
    if anchor_id == 'reversible':
        e = math.exp(-.4 * t)
        a, b = 1 + 2 * e, 3 - 2 * e
        return {'A': a, '[A]': a, 'B': b, '[B]': b,
                'default_compartment': 1., 'kf': .3, 'kr': .1}
    if anchor_id == 'nonunit_no_factor':
        s, p = 2 * math.exp(-.2 * t), -2 * math.expm1(-.2 * t)
        return {'S': 3*s, '[S]': s, 'P': 3*p, '[P]': p, 'cell': 3., 'k': .6}
    if anchor_id == 'boundary':
        return {'B': 2., '[B]': 2., 'P': 1 + .6*t, '[P]': 1 + .6*t,
                'default_compartment': 1., 'k': .3}
    if anchor_id == 'source_drain':
        s = -3 * math.expm1(-.2 * t)
        return {'S': s, '[S]': s, 'default_compartment': 1., 'q': .6, 'k': .2}
    if anchor_id == 'forward_initial':
        s = 3 * math.exp(-.4 * t)
        return {'S': s, '[S]': s, 'default_compartment': 1., 'base': 2.,
                'delta': 1., 'p': .8, 'k': .4, 'held': 3.}
    if anchor_id == 'stoichiometry':
        a, b = 4 * math.exp(-.4*t), -2 * math.expm1(-.4*t)
        return {'A': a, '[A]': a, 'B': b, '[B]': b,
                'default_compartment': 1., 'k': .2}
    a, b = 3 * math.exp(-.4*t), 1 - 1.2 * math.expm1(-.4*t)
    return {'A': 2*a, '[A]': a, 'B': 5*b, '[B]': b, 'c1': 2., 'c2': 5., 'k': .4}


def invariant_values(anchor_id, row):
    """Physical amount balances, clamps and fixed quantities; never B+P for reservoir."""
    initial = expected_values(anchor_id, 0)
    if set(row) - {'time'} != set(initial) or any(not _finite(v) for v in row.values()):
        raise ValueError('invariant row has missing/extra/nonfinite columns')
    out = {}
    def add(name, observed, expected):
        out[name] = {'observed': observed, 'expected': expected}
    species = {s[1:-1] for s in initial if s.startswith('[')}
    for name in initial:
        if name not in species and not name.startswith('['):
            add('fixed_' + name, row[name], initial[name])
    if anchor_id == 'reversible':
        add('amount_balance', row['A'] + row['B'], 4.)
    elif anchor_id == 'nonunit_no_factor':
        add('amount_balance', row['S'] + row['P'], 6.)
    elif anchor_id == 'boundary':
        add('boundary_reservoir', row['B'], 2.)
    elif anchor_id == 'stoichiometry':
        add('stoichiometric_balance', row['A'] + 2*row['B'], 4.)
    elif anchor_id == 'unequal_transfer':
        add('amount_balance', row['A'] + row['B'], 11.)
    for name in species:
        volume = (3. if anchor_id == 'nonunit_no_factor' else
                  (2. if name == 'A' else 5.) if anchor_id == 'unequal_transfer' else 1.)
        add('amount_concentration_' + name, row[name] - volume*row['['+name+']'], 0.)
    return out


# Hand-written libSBML projection preconditions, deliberately independent of source parsing.
def _identity(name):
    return {'id': name, 'name': None, 'sbo_term': None, 'annotation': None}

def _name(name):
    return {'type': 260, 'name': name}

def _op(operator, *args):
    return {'type': ord(operator), 'children': [_name(x) if isinstance(x, str) else x for x in args]}

def _compartment(name, size):
    return dict(_identity(name), size=size, spatial_dimensions=3, constant=True,
                units=None, sbo_term=410 if name == 'default_compartment' else None)

def _species(name, compartment, concentration, boundary=False):
    return dict(_identity(name), compartment=compartment, initial_amount=None,
                initial_concentration=concentration, has_only_substance_units=False,
                boundary_condition=boundary, constant=False, substance_units=None,
                conversion_factor=None)

def _parameter(name, value, constant=True, units=None):
    return dict(_identity(name), value=value, constant=constant, units=units)

def _reaction(name, reactants, products, expression, reversible=False):
    def terms(items):
        return [{'species': name, 'stoichiometry': n, 'constant': True} for name, n in items]
    return dict(_identity(name), reactants=terms(reactants), products=terms(products),
                math=expression, reversible=reversible, compartment=None,
                modifiers=[], local_parameters=[])


def expected_projection(anchor_id):
    """Fresh manually authored structural contract; not an oracle-generated golden."""
    if anchor_id not in IDS:
        raise ValueError('unknown anchor')
    p = {'level': 3, 'version': 2, 'model': _identity('__main'),
         'model_units': dict.fromkeys(['AreaUnits','ConversionFactor','ExtentUnits',
                                      'LengthUnits','SubstanceUnits','TimeUnits','VolumeUnits']),
         'packages': [], 'constraints': [], 'events': [], 'rules': [],
         'function_definitions': [], 'initial_assignments': [], 'unit_definitions': []}
    c = 'cell' if anchor_id in ('units_decay', 'nonunit_no_factor') else 'default_compartment'
    p['compartments'] = [_compartment(c, 3 if anchor_id == 'nonunit_no_factor' else 1)]
    if anchor_id == 'units_decay':
        p['species'] = [_species('S', c, 3)]
        p['parameters'] = [_parameter('k', .4, units='per_second')]
        p['reactions'] = [_reaction('J', [('S',1)], [], _op('*', _op('*','k','S'), 'cell'))]
        p['model_units'].update(ExtentUnits='mole',SubstanceUnits='mole',TimeUnits='second',VolumeUnits='litre')
        p['unit_definitions'] = [dict(_identity('per_second'), units=[
            {'exponent': -1., 'kind': 28, 'multiplier': 1., 'scale': 0}])]
    elif anchor_id == 'reversible':
        p['species'] = [_species('A',c,3),_species('B',c,1)]
        p['parameters'] = [_parameter('kf',.3),_parameter('kr',.1)]
        p['reactions'] = [_reaction('J',[('A',1)],[('B',1)],_op('-',_op('*','kf','A'),_op('*','kr','B')),True)]
    elif anchor_id == 'nonunit_no_factor':
        p['species'] = [_species('P',c,0),_species('S',c,2)]
        p['parameters'] = [_parameter('k',.6)]
        p['reactions'] = [_reaction('J',[('S',1)],[('P',1)],_op('*','k','S'))]
    elif anchor_id == 'boundary':
        p['species'] = [_species('B',c,2,True),_species('P',c,1)]
        p['parameters'] = [_parameter('k',.3)]
        p['reactions'] = [_reaction('J',[('B',1)],[('P',1)],_op('*','k','B'))]
    elif anchor_id == 'source_drain':
        p['species'] = [_species('S',c,0)]
        p['parameters'] = [_parameter('k',.2),_parameter('q',.6)]
        p['reactions'] = [_reaction('Jin',[],[('S',1)],_name('q')),
                          _reaction('Jout',[('S',1)],[],_op('*','k','S'))]
    elif anchor_id == 'forward_initial':
        p['species'] = [_species('S',c,None)]
        p['parameters'] = [_parameter('base',2),_parameter('delta',1),
                           _parameter('held',None,False),_parameter('k',None),_parameter('p',.8)]
        p['reactions'] = [_reaction('J',[('S',1)],[],_op('*','k','S'))]
        p['initial_assignments'] = [
            {'symbol':'S','math':_op('+','base','delta')},
            {'symbol':'k','math':_op('/','p',{'type':256,'integer':2})},
            {'symbol':'held','math':_name('S')}]
    elif anchor_id == 'stoichiometry':
        p['species'] = [_species('A',c,4),_species('B',c,0)]
        p['parameters'] = [_parameter('k',.2)]
        p['reactions'] = [_reaction('J',[('A',2)],[('B',1)],_op('*','k','A'))]
    else:
        p['compartments'] = [_compartment('c1',2),_compartment('c2',5)]
        p['species'] = [_species('A','c1',3),_species('B','c2',1)]
        p['parameters'] = [_parameter('k',.4)]
        p['reactions'] = [_reaction('J',[('A',1)],[('B',1)],_op('*',_op('*','k','A'),'c1'))]
    return p


def _same(expected, actual, path='root'):
    if _finite(expected):
        good = _finite(actual) and expected == actual
    elif isinstance(expected, dict):
        good = isinstance(actual, dict) and set(actual) == set(expected)
        if good:
            for key in expected:
                _same(expected[key], actual[key], path + '.' + key)
    elif isinstance(expected, list):
        good = isinstance(actual, list) and len(actual) == len(expected)
        if good:
            for index, (e, a) in enumerate(zip(expected, actual)):
                _same(e, a, f'{path}[{index}]')
    else:
        good = type(expected) is type(actual) and expected == actual
    if not good:
        raise ValueError('anchor precondition mismatch at ' + path)


def validate_preconditions(anchor_id, projection):
    """Reject semantic drift including flags, math trees, units and unmodeled constructs."""
    _same(expected_projection(anchor_id), projection)


def validate_manifest(manifest):
    """Freeze math-affecting settings and bind sources by hash before execution."""
    if not isinstance(manifest, dict):
        raise ValueError('manifest must be an object')
    _same('antimony-numerical-manifest-v1', manifest.get('schema'))
    for key, expected in [('grid',GRID),('comparison',COMPARISON),('solver',SOLVER)]:
        _same(expected, manifest.get(key), key)
    cases = manifest.get('cases')
    if (not isinstance(cases, list) or any(not isinstance(c, dict) for c in cases)
            or [c.get('id') for c in cases] != list(IDS)):
        raise ValueError('exact ordered nonempty eight-case inventory required')
    for case in cases:
        name = case['id']
        _same('fixtures/numerical/' + name + '.ant', case.get('path'))
        _same(['time', *expected_values(name, 0)], case.get('selections'))
        _same(CONSTANTS[name], case.get('constants'))
        source = ROOT / case['path']
        if hashlib.sha256(source.read_bytes()).hexdigest() != case.get('sha256'):
            raise ValueError('source hash drift: ' + name)
    return manifest


def load_manifest():
    return validate_manifest(json.loads(MANIFEST.read_text()))
