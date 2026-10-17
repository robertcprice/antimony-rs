"""Two independent analytic inputs for typed exp/sin/time expressions.

Coefficients and formulae are hand-derived, never fitted from simulator output.
"""
import math
from pathlib import Path
import json
import anchors as base

ROOT = Path(__file__).resolve().parents[2]
MANIFEST = ROOT / 'fixtures/numerical/builtins/manifest.json'
IDS = ('exponential_input', 'oscillating_input')


def expected_values(id, t):
    if id not in IDS or type(t) not in (int,float) or not math.isfinite(t) or not 0 <= t <= 10:
        raise ValueError('Unknown builtin anchor or invalid time')
    # dS/dt=exp(-t), S(0)=0 => S(t)=1-exp(-t).
    # dS/dt=1+sin(t), S(0)=0 => S(t)=t+1-cos(t).
    value = -math.expm1(-t) if id == 'exponential_input' else t + 2*math.sin(t/2)**2
    return {'S':value, '[S]':value, 'default_compartment':1.0}


def expected_projection(id):
    if id not in IDS:raise ValueError('Unknown builtin anchor')
    time={'type':262,'name':'time'}
    expression=({'type':290,'children':[{'type':45,'children':[time]}]} if id=='exponential_input'
                else {'type':43,'children':[{'type':256,'integer':1},{'type':300,'children':[time]}]})
    return {'level':3,'version':2,'model':base._identity('__main'),
            'model_units':dict.fromkeys(['AreaUnits','ConversionFactor','ExtentUnits','LengthUnits','SubstanceUnits','TimeUnits','VolumeUnits']),
            'compartments':[base._compartment('default_compartment',1)],
            'species':[base._species('S','default_compartment',0)], 'parameters':[],
            'reactions':[base._reaction('J',[],[('S',1)],expression)],
            'initial_assignments':[], 'rules':[], 'events':[], 'function_definitions':[],
            'unit_definitions':[], 'constraints':[], 'packages':[]}


def validate_preconditions(id, projection):
    base._same(expected_projection(id),projection)


def invariant_values(id, row):
    expected=expected_values(id,0)
    if set(row) != {'time',*expected} or any(type(v) not in (int,float) or not math.isfinite(v) for v in row.values()):
        raise ValueError('Invalid builtin invariant row')
    return {'volume':{'observed':row['default_compartment'],'expected':1.0},
            'amount_equals_concentration':{'observed':row['S']-row['[S]'],'expected':0.0}}


def load_manifest():
    import hashlib
    m=json.loads(MANIFEST.read_text())
    if set(m) != {'schema','implemented_profile','grid','comparison','solver','cases'} or m['schema']!='antimony-builtin-numerical-v1' or m['implemented_profile']!='math-core-v0.2-slice':
        raise ValueError('Unknown builtin numerical manifest')
    for k in ('grid','comparison','solver'):base._same(getattr(base,k.upper()),m[k])
    if [r['id'] for r in m['cases']] != list(IDS):raise ValueError('Exactly two ordered builtin anchors required')
    for r in m['cases']:
        base._same('fixtures/numerical/builtins/'+r['id']+'.ant',r['path'])
        base._same(['time',*expected_values(r['id'],0)],r['selections'])
        if hashlib.sha256((ROOT/r['path']).read_bytes()).hexdigest()!=r['sha256']:raise ValueError('Builtin numerical source hash drift')
    return m
