"""One independent continuous-rate piecewise source, not a discontinuous-input claim."""
import math
from pathlib import Path
import json
import anchors as base

ROOT=Path(__file__).resolve().parents[2]
MANIFEST=ROOT/'fixtures/numerical/conditionals/manifest.json'
ID='continuous_piecewise_input'


def expected_values(id,t):
    if id!=ID or type(t) not in (int,float) or not math.isfinite(t) or not 0<=t<=10:
        raise ValueError('Unknown piecewise anchor or invalid time')
    # Integrate rate t until5, then constant5; both rate and accumulated S are
    # continuous at5. No jump/event-sided convention or fitted coefficient.
    value=0.5*t*t if t<=5 else 5*t-12.5
    return {'S':value,'[S]':value,'default_compartment':1.0}


def expected_projection(id):
    if id!=ID:raise ValueError('Unknown piecewise anchor')
    time={'type':262,'name':'time'};five={'type':256,'integer':5}
    expr={'type':295,'children':[time,{'type':312,'children':[time,five]},five]}
    return {'level':3,'version':2,'model':base._identity('__main'),
            'model_units':dict.fromkeys(['AreaUnits','ConversionFactor','ExtentUnits','LengthUnits','SubstanceUnits','TimeUnits','VolumeUnits']),
            'compartments':[base._compartment('default_compartment',1)],
            'species':[base._species('S','default_compartment',0)],'parameters':[],
            'reactions':[base._reaction('J',[],[('S',1)],expr)],
            'initial_assignments':[],'rules':[],'events':[],'function_definitions':[],
            'unit_definitions':[],'constraints':[],'packages':[]}


def validate_preconditions(id,projection):
    base._same(expected_projection(id),projection)


def invariant_values(id,row):
    expected=expected_values(id,0)
    if set(row)!={'time',*expected} or any(type(v) not in (int,float) or not math.isfinite(v) for v in row.values()):
        raise ValueError('Invalid piecewise invariant row')
    return {'volume':{'observed':row['default_compartment'],'expected':1.0},
            'amount_equals_concentration':{'observed':row['S']-row['[S]'],'expected':0.0}}


def load_manifest():
    import hashlib
    m=json.loads(MANIFEST.read_text())
    if set(m)!={'schema','implemented_profile','grid','comparison','solver','cases'} or m['schema']!='antimony-piecewise-numerical-v1' or m['implemented_profile']!='conditional-core-v0.3-slice':
        raise ValueError('Unknown piecewise numerical manifest')
    for k in ('grid','comparison','solver'):base._same(getattr(base,k.upper()),m[k])
    if len(m['cases'])!=1 or m['cases'][0]['id']!=ID:raise ValueError('Exactly one fixed piecewise anchor required')
    row=m['cases'][0]
    base._same('fixtures/numerical/conditionals/'+ID+'.ant',row['path'])
    base._same(['time',*expected_values(ID,0)],row['selections'])
    if hashlib.sha256((ROOT/row['path']).read_bytes()).hexdigest()!=row['sha256']:raise ValueError('Piecewise numerical source hash drift')
    return m
