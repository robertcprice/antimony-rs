"""Six original structural workloads; hand-authored meaning, no simulated truth.

No equations/constants are recovered from a reference export. These builders
encode the reviewed source contracts in docs/WORKLOAD_MODELS.md.
"""
from __future__ import annotations
import copy
import hashlib
import json
import math
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
MANIFEST = ROOT / 'fixtures/workloads/manifest.json'
IDS = ('enzyme_binding_catalysis', 'branched_pathway', 'coupled_transport_catalysis',
       'rational_feedback', 'reversible_shuttle', 'unit_forward_sequence')


def identity(name):
    return {'id': name, 'name': None, 'sbo_term': None, 'annotation': None}


def symbol(name):
    return {'type': 260, 'name': name}


def integer(n):
    return {'type': 256, 'integer': n}


def op(operator, *children):
    return {'type': 296 if operator == '^' else ord(operator), 'children': [symbol(c) if isinstance(c, str) else c for c in children]}


def compartment(name, size):
    return dict(identity(name), size=size, spatial_dimensions=3, constant=True, units=None)


def species(name, location, initial, boundary=False):
    return dict(identity(name), compartment=location, initial_concentration=initial,
                initial_amount=None, boundary_condition=boundary, constant=False,
                has_only_substance_units=False, substance_units=None, conversion_factor=None)


def parameter(name, value, units=None):
    return dict(identity(name), value=value, constant=True, units=units)


def reaction(name, reactants, products, math_tree, reversible=False, modifiers=()):
    def refs(names):
        return [{'species': n, 'stoichiometry': 1., 'constant': True} for n in names]
    return dict(identity(name), reactants=refs(reactants), products=refs(products),
                reversible=reversible, math=math_tree, modifiers=list(modifiers),
                compartment=None, local_parameters=[])


def rate(k, s, c='cell'):
    return op('*', op('*', k, s), c)


def expected_projection(motif_id):
    """Fresh full typed projection independently authored from the model specification."""
    if motif_id not in IDS:
        raise ValueError('unknown original motif')
    p = {'level': 3, 'version': 2, 'model': identity('__main'),
         'model_units': dict.fromkeys(['SubstanceUnits','TimeUnits','VolumeUnits',
                                      'AreaUnits','LengthUnits','ExtentUnits','ConversionFactor']),
         'packages': [], 'rules': [], 'events': [], 'constraints': [],
         'function_definitions': [], 'initial_assignments': [], 'unit_definitions': []}
    if motif_id == 'enzyme_binding_catalysis':
        compartments=[compartment('cell',2)]
        ss=[species(n,'cell',v) for n,v in [('E',1),('S',8),('C',0),('P',0)]]
        ps=[parameter(n,v) for n,v in [('kon',.3),('koff',.2),('kcat',.4)]]
        rs=[reaction('Bind',['E','S'],['C'],op('*',op('-',op('*',op('*','kon','E'),'S'),op('*','koff','C')),'cell'),True),
            reaction('Cat',['C'],['E','P'],rate('kcat','C'))]
    elif motif_id == 'branched_pathway':
        compartments=[compartment('cell',3)]
        ss=[species(n,'cell',v) for n,v in [('A',6),('B',1),('C',2),('D',0)]]
        ps=[parameter(n,v) for n,v in [('k1',None),('q',.4),('k2',.1),('k3',.3),('k4',.25)]]
        rs=[reaction(n,[a],[b],rate(k,a)) for n,a,b,k in
            [('AB','A','B','k1'),('AC','A','C','k2'),('BD','B','D','k3'),('CD','C','D','k4')]]
        p['initial_assignments']=[{'symbol':'k1','math':op('/','q',integer(2))}]
    elif motif_id == 'coupled_transport_catalysis':
        compartments=[compartment('outside',2),compartment('inside',5)]
        ss=[species('A','outside',4)]+[species(n,'inside',v) for n,v in [('B',1),('F',2),('C',0),('P',0)]]
        ps=[parameter(n,v) for n,v in [('kt',.2),('kon',.3),('koff',.1),('kcat',.05)]]
        rs=[reaction('Transport',['A'],['B'],rate('kt','A','outside')),
            reaction('Bind',['B','F'],['C'],op('*',op('-',op('*',op('*','kon','B'),'F'),op('*','koff','C')),'inside'),True),
            reaction('Cat',['C'],['F','P'],rate('kcat','C','inside'))]
    elif motif_id == 'rational_feedback':
        compartments=[compartment('cell',2)]
        ss=[species('Feed','cell',2,True)]+[species(n,'cell',v) for n,v in [('X',1),('Y',0),('Z',.5)]]
        ps=[parameter(n,v) for n,v in [('q',.4),('Ki',2),('k1',.3),('k2',.2),('d',.1)]]
        inhibited=op('/',op('*','q','Feed'),op('+',integer(1),op('^',op('/','Z','Ki'),integer(2))))
        rs=[reaction('Input',['Feed'],['X'],op('*',inhibited,'cell'),modifiers=['Z']),
            reaction('XY',['X'],['Y'],rate('k1','X')),
            reaction('YZ',['Y'],['Z'],rate('k2','Y')),
            reaction('Drain',['Z'],[],rate('d','Z'))]
    elif motif_id == 'reversible_shuttle':
        compartments=[compartment('left',2),compartment('right',5)]
        ss=[species('A','left',2),species('B','right',1),species('C','right',.5)]
        ps=[parameter(n,v) for n,v in [('kt',.2),('kr',.1),('kb',.3),('kc',.15)]]
        rs=[reaction('Exchange',['A'],['B'],op('-',rate('kt','A','left'),rate('kr','B','right')),True),
            reaction('Convert',['B'],['C'],rate('kb','B','right')),
            reaction('Return',['C'],['A'],rate('kc','C','right'))]
    else:
        compartments=[compartment('cell',4)]
        ss=[species(n,'cell',v) for n,v in [('S',3),('I',0),('P',1)]]
        ps=[parameter('kfast',None,'per'),parameter('kslow',.1,'per')]
        rs=[reaction('First',['S'],['I'],rate('kfast','S')),
            reaction('Second',['I'],['P'],rate('kslow','I'))]
        p['initial_assignments']=[{'symbol':'kfast','math':op('*','kslow',integer(2))}]
        p['model_units'].update(SubstanceUnits='mole',ExtentUnits='mole',TimeUnits='second',VolumeUnits='litre')
        p['unit_definitions']=[dict(identity('per'),units=[
            {'kind':28,'exponent':-1.,'scale':0,'multiplier':1.}])]
    p.update(compartments=sorted(compartments,key=lambda x:x['id']),
             species=sorted(ss,key=lambda x:x['id']),parameters=sorted(ps,key=lambda x:x['id']),
             reactions=sorted(rs,key=lambda x:x['id']))
    return p


# Amount-space linear invariants derived from effective stoichiometry, not trajectories.
# Boundary species' effective reaction row is zero even when listed as a reactant.
_BALANCES = {
    'enzyme_binding_catalysis': [({'E':1,'C':1},2),({'S':1,'C':1,'P':1},16)],
    'branched_pathway': [({'A':1,'B':1,'C':1,'D':1},27)],
    'coupled_transport_catalysis': [({'A':1,'B':1,'C':1,'P':1},13),({'F':1,'C':1},10)],
    'rational_feedback': [({'Feed':1},4)],
    'reversible_shuttle': [({'A':1,'B':1,'C':1},11.5)],
    'unit_forward_sequence': [({'S':1,'I':1,'P':1},16)],
}

def balance_laws(motif_id):
    """Return (amount weights, initial total) pairs; derived, NOT trajectory-verified."""
    if motif_id not in IDS:
        raise ValueError('unknown original motif')
    return copy.deepcopy(_BALANCES[motif_id])


def finite(value):
    try:
        return type(value) in (int,float) and math.isfinite(value)
    except OverflowError:
        return False


def _same(expected, actual, path='root'):
    if finite(expected):
        equal=finite(actual) and expected==actual
    elif isinstance(expected,dict):
        equal=isinstance(actual,dict) and set(expected)==set(actual)
        if equal:
            for k in expected:_same(expected[k],actual[k],path+'.'+k)
    elif isinstance(expected,list):
        equal=isinstance(actual,list) and len(expected)==len(actual)
        if equal:
            for i,(a,b) in enumerate(zip(expected,actual)):_same(a,b,f'{path}[{i}]')
    else:
        equal=type(expected) is type(actual) and expected==actual
    if not equal:raise ValueError('motif contract mismatch at '+path)


def validate_preconditions(motif_id, projection):
    _same(expected_projection(motif_id),projection)


def validate_manifest(manifest):
    if not isinstance(manifest,dict) or manifest.get('schema')!='antimony-workload-manifest-v1':
        raise ValueError('invalid original workload manifest')
    cases=manifest.get('cases')
    if (not isinstance(cases,list) or any(not isinstance(c,dict) for c in cases)
            or [c.get('id') for c in cases]!=list(IDS)):
        raise ValueError('exact ordered six original motifs required')
    for case in cases:
        name=case['id']
        for key,value in [('kind','motif'),('path',f'fixtures/workloads/{name}.ant'),
                          ('expectation_id',name),('numerical','NOT_RUN')]:
            _same(value,case.get(key))
        if not isinstance(case.get('provenance'),str) or not case['provenance'].strip():
            raise ValueError('original provenance required')
        if hashlib.sha256((ROOT/case['path']).read_bytes()).hexdigest()!=case.get('sha256'):
            raise ValueError('motif source hash drift: '+name)
    return manifest


def _unique(pairs):
    result={}
    for key,value in pairs:
        if key in result:raise ValueError('duplicate manifest JSON key')
        result[key]=value
    return result


def load_manifest():
    return validate_manifest(json.loads(MANIFEST.read_text(),object_pairs_hook=_unique,
        parse_constant=lambda x: (_ for _ in ()).throw(ValueError('nonfinite JSON'))))


def get_cases():
    return load_manifest()['cases']
