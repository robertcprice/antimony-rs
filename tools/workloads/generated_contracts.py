"""Hand-authored structural truth for the finite generated recipe grid.

Does not import the source renderer, native crate or reference oracle. No observed
projection is promoted to expected truth. Numeric constants/types are explicit.
"""
import math


def identity(name):
    return {'id': name, 'name': None, 'sbo_term': None, 'annotation': None}


def name(value):
    return {'type': 260, 'name': value}


def integer(value):
    return {'type': 256, 'integer': value}


def op(symbol, *children):
    return {'type': ord(symbol), 'children': list(children)}


def parameter(label, value, constant=True, units=None):
    return dict(identity(label), value=value, constant=constant, units=units)


def compartment(label, value):
    return dict(identity(label), size=value, spatial_dimensions=3, constant=True,
                units=None, sbo_term=410 if label == 'default_compartment' else None)


def species(label, where, initial, boundary=False):
    return dict(identity(label), compartment=where, initial_amount=None,
                initial_concentration=initial, boundary_condition=boundary, constant=False,
                has_only_substance_units=False, substance_units=None, conversion_factor=None)


def reaction(expression, boundary=False):
    return dict(identity('J'), compartment=None, reversible=False,
                reactants=[{'species':'S','stoichiometry':1,'constant':True}],
                products=[{'species':'P','stoichiometry':1,'constant':True}],
                modifiers=[], local_parameters=[], math=expression)


def base(label):
    return {'level':3,'version':2,'model':identity(label),
            'model_units': dict.fromkeys(['AreaUnits','ConversionFactor','ExtentUnits','LengthUnits','SubstanceUnits','TimeUnits','VolumeUnits']),
            'compartments':[], 'species':[], 'parameters':[], 'reactions':[],
            'initial_assignments':[], 'unit_definitions':[], 'rules':[], 'events':[],
            'function_definitions':[], 'constraints':[], 'packages':[]}


def expected(case):
    spec = case['recipe']; family, i = spec['family'], spec['index']
    limit = 16 if family in ('dag','compartments','units','numeric') else 4
    if family not in ('dag','compartments','units','numeric','cycle','dimensions','unset','envelope') or type(i) is not int or not 0 <= i < limit:
        raise ValueError('Unknown independent recipe')
    p = base(f'grid_{family}_{i:02}')
    contract = {'reference_status':'PASS','native_status':'EXPORTED','projection':p}
    if family == 'dag':
        species_dep = i % 2 == 1; boundary = i % 4 == 3
        p['parameters'] = [parameter('a',i+1),parameter('b',None,not species_dep or boundary),
                           parameter('k',.25),parameter('z',None,not species_dep or boundary)]
        p['initial_assignments'] = [
            {'symbol':'z','math':op('+',name('b'),name('a'))},
            {'symbol':'b','math':op('*',name('S' if species_dep else 'a'),integer(i%3+2))}]
        if species_dep:
            p['compartments']=[compartment('default_compartment',1)]
            p['species']=[species('P','default_compartment',0),species('S','default_compartment',2,boundary)]
            p['reactions']=[reaction(op('*',name('k'),name('S')))]
    elif family == 'compartments':
        volumes=(1,2,3,5)
        p['compartments']=[compartment('c1',volumes[i//4]),compartment('c2',volumes[i%4])]
        p['species']=[species('P','c2',1),species('S','c1',3)]
        p['parameters']=[parameter('k',.5)]
        expression=op('*',name('k'),name('S'))
        if i%2:expression=op('*',expression,name('c1'))
        p['reactions']=[reaction(expression)]
    elif family == 'units':
        # libSBML kind constants: second28, mole23, litre18. Order is semantic receipt.
        terms = ([{'kind':28,'exponent':1,'scale':0,'multiplier':2}],
                 [{'kind':23,'exponent':1,'scale':-3,'multiplier':1}],
                 [{'kind':23,'exponent':1,'scale':0,'multiplier':1},{'kind':18,'exponent':1,'scale':0,'multiplier':1}],
                 [{'kind':18,'exponent':1,'scale':0,'multiplier':1},{'kind':23,'exponent':1,'scale':0,'multiplier':1}])
        p['unit_definitions']=[dict(identity('u'),units=terms[i//4])]
        p['parameters']=[parameter('p',i%4+1,units='u')]
    elif family == 'numeric':
        values=(0,0,0,1e-307,-1e-307,1e308,-1e308,1.23456789012345,2.5,2000,.002,-2,None,None,None,None)
        p['parameters']=[parameter('a',values[i])]
        if i>=12:
            trees=(integer(2),integer(3),integer(-2),op('+',integer(2),integer(0)))
            p['initial_assignments']=[{'symbol':'a','math':trees[i-12]}]
    elif family == 'cycle':
        contract.update(reference_status='REFERENCE_REJECTED',native_status='REJECTED',
                        native_stage='resolve',native_code='initialization_cycle',projection=None,
                        reference_diagnostic_contains='Loop detected')
    elif family == 'dimensions':
        p['parameters']=[parameter('a',i+1,units='second'),parameter('b',i+2,units='mole'),parameter('p',None,units='second')]
        p['initial_assignments']=[{'symbol':'p','math':op('+',name('a'),name('b'))}]
        contract.update(native_status='REJECTED',native_stage='resolve',native_code='units_inconsistency')
    elif family == 'unset':
        p['parameters']=[parameter('a',None),parameter('b',i+1)]
        contract.update(native_status='REJECTED',native_stage='resolve',native_code='unsupported_feature')
    else:
        # Pinned upstream serialization rounds these rejected 16-digit inputs.
        # This explicit negative-case expectation is NOT a general normalizer.
        p['parameters']=[parameter('a',(1.1e308,-1.1e308,1.23456789012346,-1.23456789012346)[i])]
        contract.update(native_status='REJECTED',native_stage='syntax',native_code='unsupported_feature')
    return contract


def same(expected_value, observed, at='$'):
    """Closed shape and bool/null/string types; finite mathematical numeric equality."""
    if type(expected_value) in (int,float):
        try: good=type(observed) in (int,float) and math.isfinite(observed) and observed==expected_value
        except OverflowError: good=False
    elif isinstance(expected_value,dict):
        good=isinstance(observed,dict) and set(observed)==set(expected_value)
        if good:
            for k,v in expected_value.items():same(v,observed[k],at+'.'+k)
    elif isinstance(expected_value,list):
        good=isinstance(observed,list) and len(observed)==len(expected_value)
        if good:
            for i,(a,b) in enumerate(zip(expected_value,observed)):same(a,b,f'{at}[{i}]')
    else:good=type(expected_value) is type(observed) and expected_value==observed
    if not good:raise ValueError('Independent contract mismatch at '+at)
