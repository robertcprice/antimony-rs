#!/usr/bin/env python3
"""Optional, trusted-input reference oracle. The orchestrator is stdlib-only.

No network or installation is performed. Subprocess isolation is NOT a sandbox:
Antimony may follow imports. Only explicitly trusted, local inputs are accepted.
"""
from __future__ import annotations
import argparse
import hashlib
import importlib.metadata
import inspect
import io
import re
import xml.etree.ElementTree as ET
import json
import math
import os
from pathlib import Path
import platform
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[2]
SCHEMA = 'antimony-rs-oracle-v1'
VERSIONS = {'antimony': '3.2.0', 'python-libsbml': '5.21.2'}
MAX_INPUT = 4 * 1024 * 1024

def digest(data):
    return hashlib.sha256(data).hexdigest()

def emit(obj):
    print(json.dumps(obj, indent=2, sort_keys=True, allow_nan=False))

def result(status, **kw):
    return {'schema': SCHEMA, 'status': status, **kw}

def load_succeeded(index):
    return index >= 0

def optional(obj, name):
    return getattr(obj, 'get' + name)() if getattr(obj, 'isSet' + name)() else None

def number(value):
    if not math.isfinite(value):
        raise ValueError('Non-finite numeric value in SBML')
    return value

def ast(node):
    if node is None:
        return None
    out = {'type': node.getType()}
    # User-defined calls are isFunction(), not isName(). Their identity is
    # semantic: f(x) and g(x) must never collapse to identical projections.
    if node.isName() or node.isUserFunction():
        out['name'] = node.getName()
    elif node.isInteger():
        out['integer'] = node.getInteger()
    elif node.isRational():
        out['numerator'], out['denominator'] = node.getNumerator(), node.getDenominator()
    elif node.isReal():
        out['real'] = number(node.getReal())
    if node.getNumChildren():
        out['children'] = [ast(node.getChild(i)) for i in range(node.getNumChildren())]
    return out

def identity(x):
    return {'id': x.getId(), 'name': optional(x, 'Name'),
            'sbo_term': optional(x, 'SBOTerm'),
            'annotation': x.getAnnotationString() or None}

CORE = 'http://www.sbml.org/sbml/level3/version2/core'
MATH = 'http://www.w3.org/1998/Math/MathML'
# Deliberately closed XML vocabulary for the initial corpus. Unsupported data
# must never disappear behind a PASS semantic projection.
CORE_TAGS = set("sbml model listOfCompartments compartment listOfSpecies species listOfParameters parameter listOfReactions reaction listOfReactants listOfProducts speciesReference listOfModifiers modifierSpeciesReference kineticLaw listOfLocalParameters localParameter listOfInitialAssignments initialAssignment listOfRules assignmentRule rateRule algebraicRule listOfEvents event trigger delay priority listOfEventAssignments eventAssignment listOfFunctionDefinitions functionDefinition listOfUnitDefinitions unitDefinition listOfUnits unit listOfConstraints constraint".split())
CORE_ATTRS = set("level version id metaid name sboTerm substanceUnits timeUnits volumeUnits areaUnits lengthUnits extentUnits conversionFactor size spatialDimensions constant units compartment initialAmount initialConcentration boundaryCondition hasOnlySubstanceUnits value reversible species stoichiometry symbol variable useValuesFromTriggerTime persistent initialValue kind exponent scale multiplier".split())
# Numeric builtin vocabulary verified against pinned Antimony 3.2.0; no lambda,
# piecewise or arbitrary csymbol semantics are admitted by this increment.
MATH_TAGS = set('math apply ci cn sep plus minus times divide power sin cos tan exp ln log logbase root degree abs floor ceiling min max pi exponentiale gt csymbol'.split())
# Pinned Phase3B conditional vocabulary: exact AST/branch order retained.
# XOR/implies, delay and nonfinite leaves remain excluded.
MATH_TAGS |= set('true false and or not eq geq leq lt neq piecewise piece otherwise'.split())
# Pinned Phase5 user-defined function vocabulary: lambda/bvar only inside functionDefinition/math.
MATH_TAGS |= set('lambda bvar'.split())

def projection_limits(xml):
    if '<!DOCTYPE' in xml.upper() or '<!ENTITY' in xml.upper():
        return ['DTD/entity declarations are not accepted']
    root = ET.fromstring(xml)
    parents = {child: parent for parent in root.iter() for child in parent}
    def split_tag(elem):
        if elem is None or not isinstance(elem.tag, str):
            return ('', '')
        return elem.tag[1:].partition('}')[::2] if elem.tag.startswith('{') else ('', elem.tag)
    reasons = []
    for node in root.iter():
        if not isinstance(node.tag, str):
            continue
        ns, _, tag = node.tag[1:].partition('}') if node.tag.startswith('{') else ('', '', node.tag)
        if ns == CORE:
            if tag not in CORE_TAGS:
                reasons.append('Unrepresented core element: ' + tag)
            for attr in node.attrib:
                if attr not in CORE_ATTRS:
                    reasons.append('Unrepresented core attribute: ' + attr)
            if tag in ('speciesReference', 'modifierSpeciesReference') and any(k in node.attrib for k in ('id', 'name', 'sboTerm')):
                reasons.append('Species-reference metadata is outside projection v1')
            if (tag in ('assignmentRule', 'rateRule') and (set(node.attrib) - {'variable'})) or (tag == 'algebraicRule' and node.attrib):
                reasons.append('Rule metadata is outside projection v1')
            if tag == 'functionDefinition':
                if set(node.attrib) - {'id'}:
                    reasons.append('FunctionDefinition metadata is outside projection v1')
                children = [split_tag(c) for c in list(node)]
                if children != [(MATH, 'math')] or [split_tag(c) for c in list(node[0])] != [(MATH, 'lambda')]:
                    reasons.append('FunctionDefinition without lambda math is outside projection v1')
        elif ns == MATH:
            if tag not in MATH_TAGS:
                reasons.append('Unrepresented MathML element: ' + tag)
            if tag == 'lambda':
                parent = parents.get(node)
                grandparent = parents.get(parent) if parent is not None else None
                children = [split_tag(c) for c in list(node)]
                bvars = 0
                while bvars < len(children) and children[bvars] == (MATH, 'bvar'):
                    bvars += 1
                if (split_tag(parent) != (MATH, 'math')
                        or split_tag(grandparent) != (CORE, 'functionDefinition')
                        or len(children) != bvars + 1
                        or children[-1][0] != MATH
                        or children[-1][1] in ('bvar', 'lambda')):
                    reasons.append('Unrepresented MathML element: lambda')
            if tag == 'bvar':
                parent = parents.get(node)
                children = [split_tag(c) for c in list(node)]
                if split_tag(parent) != (MATH, 'lambda') or children != [(MATH, 'ci')]:
                    reasons.append('Unrepresented MathML element: bvar')
            allowed = {'type'} if tag == 'cn' else {'encoding', 'definitionURL'} if tag == 'csymbol' else set()
            if set(node.attrib) - allowed:
                reasons.append('Unrepresented MathML attributes: ' + tag)
            if tag == 'csymbol' and node.attrib.get('definitionURL') != 'http://www.sbml.org/sbml/symbols/time':
                reasons.append('Only the SBML time csymbol is represented')
        else:
            reasons.append('Unrepresented XML namespace: ' + ns)
    namespaces = list(ET.iterparse(io.StringIO(xml), events=['start-ns']))
    for _, (_, uri) in namespaces:
        if uri not in (CORE, MATH):
            reasons.append('Package/other namespace declaration: ' + uri)
    return sorted(set(reasons))

def projection(doc):
    """Versioned semantic projection; not a claim to represent arbitrary SBML."""
    m = doc.getModel()
    if m is None:
        raise ValueError('No SBML model')
    def ordered(xs):
        return sorted(xs, key=lambda x: x['id'])
    def references(xs):
        return [{'species': x.getSpecies(), 'stoichiometry': number(x.getStoichiometry()),
                 'constant': x.getConstant()} for x in xs]
    reactions = []
    for r in m.getListOfReactions():
        k = r.getKineticLaw()
        reactions.append({**identity(r), 'reversible': r.getReversible(),
            'compartment': optional(r, 'Compartment'),
            'reactants': references(r.getListOfReactants()),
            'products': references(r.getListOfProducts()),
            'modifiers': sorted(x.getSpecies() for x in r.getListOfModifiers()),
            'math': ast(k.getMath()) if k else None,
            'local_parameters': ordered([{**identity(x), 'value': optional(x, 'Value'),
                'units': optional(x, 'Units')} for x in k.getListOfLocalParameters()]) if k else []})
    return {
        'level': doc.getLevel(), 'version': doc.getVersion(), 'model': identity(m),
        'model_units': {x: optional(m, x) for x in ['SubstanceUnits', 'TimeUnits', 'VolumeUnits', 'AreaUnits', 'LengthUnits', 'ExtentUnits', 'ConversionFactor']},
        'compartments': ordered([{**identity(x), 'size': optional(x, 'Size'),
            'spatial_dimensions': optional(x, 'SpatialDimensions'), 'constant': x.getConstant(),
            'units': optional(x, 'Units')} for x in m.getListOfCompartments()]),
        'species': ordered([{**identity(x), 'compartment': x.getCompartment(),
            'initial_amount': optional(x, 'InitialAmount'),
            'initial_concentration': optional(x, 'InitialConcentration'),
            'boundary_condition': x.getBoundaryCondition(), 'constant': x.getConstant(),
            'has_only_substance_units': x.getHasOnlySubstanceUnits(),
            'substance_units': optional(x, 'SubstanceUnits'),
            'conversion_factor': optional(x, 'ConversionFactor')} for x in m.getListOfSpecies()]),
        'parameters': ordered([{**identity(x), 'value': optional(x, 'Value'),
            'constant': x.getConstant(), 'units': optional(x, 'Units')} for x in m.getListOfParameters()]),
        'reactions': ordered(reactions),
        'initial_assignments': [{'symbol': x.getSymbol(), 'math': ast(x.getMath())} for x in m.getListOfInitialAssignments()],
        'rules': [{'kind': x.getElementName(), 'variable': x.getVariable(), 'math': ast(x.getMath())} for x in m.getListOfRules()],
        'events': [{**identity(x), 'use_values_from_trigger_time': x.getUseValuesFromTriggerTime(),
            'trigger': {'math': ast(x.getTrigger().getMath()), 'persistent': x.getTrigger().getPersistent(), 'initial_value': x.getTrigger().getInitialValue()} if x.isSetTrigger() else None,
            'delay': ast(x.getDelay().getMath()) if x.isSetDelay() else None,
            'priority': ast(x.getPriority().getMath()) if x.isSetPriority() else None,
            'assignments': [{'variable': a.getVariable(), 'math': ast(a.getMath())} for a in x.getListOfEventAssignments()]} for x in m.getListOfEvents()],
        'function_definitions': [{**identity(x), 'math': ast(x.getMath())} for x in m.getListOfFunctionDefinitions()],
        'unit_definitions': [{**identity(x), 'units': [{'kind': u.getKind(), 'exponent': number(u.getExponentAsDouble()), 'scale': u.getScale(), 'multiplier': number(u.getMultiplier())} for u in x.getListOfUnits()]} for x in m.getListOfUnitDefinitions()],
        'constraints': [{'math': ast(x.getMath()), 'message': x.getMessageString()} for x in m.getListOfConstraints()],
        'packages': [],  # projection_limits requires actual Core-only XML namespaces
    }

def messages(doc):
    return [{'id': doc.getError(i).getErrorId(),
             'severity': doc.getError(i).getSeverityAsString(),
             'category': doc.getError(i).getCategoryAsString(),
             'message': doc.getError(i).getMessage().strip()}
            for i in range(doc.getNumErrors())]

def dependencies():
    import antimony
    import libsbml
    actual = {name: importlib.metadata.version(name) for name in VERSIONS}
    if actual != VERSIONS:
        raise RuntimeError(f'Pinned versions required: {VERSIONS}; installed: {actual}')
    return antimony, libsbml

def metadata(a, s):
    names = ['loadAntimonyString', 'getMainModuleName', 'getSBMLString', 'getLastError', 'clearPreviousLoads', 'setWriteSBMLTimestamp', 'getVersionStr']
    return {'packages': {name: importlib.metadata.version(name) for name in VERSIONS},
        'antimony_library_version': a.getVersionStr(),
        'libsbml_version': s.getLibSBMLDottedVersion(),
        'python': platform.python_version(), 'platform': platform.platform(),
        'libsbml_dependencies': {name: s.getLibSBMLDependencyVersionOf(name) for name in ['expat','libxml','zlib','bzip2']},
        'libsbml_registered_packages': [s.SBMLExtensionRegistry.getRegisteredPackageName(i) for i in range(s.SBMLExtensionRegistry.getNumRegisteredPackages())],
        'antimony_api_signatures': {name: str(inspect.signature(getattr(a, name))) for name in names},
        'antimony_python_wrapper_sha256': digest(Path(inspect.getfile(a.loadAntimonyString)).read_bytes())}

def worker(request):
    try:
        a, s = dependencies()
    except (ImportError, OSError, RuntimeError, importlib.metadata.PackageNotFoundError) as e:
        return result('ORACLE_BLOCKED', reason=str(e))
    meta = metadata(a, s)
    if request['mode'] == 'probe':
        return result('PASS', oracle=meta)
    text = request['text']
    source_hash = digest(text.encode())
    extra = {}
    if request['mode'] == 'convert':
        a.clearPreviousLoads()
        a.setWriteSBMLTimestamp(False)
        loaded = a.loadAntimonyString(text)
        if not load_succeeded(loaded):
            return result('REFERENCE_REJECTED', source_sha256=source_hash, oracle=meta,
                          load_result=loaded, diagnostic=a.getLastError())
        module = a.getMainModuleName()
        xml = a.getSBMLString(module)
        if not xml:
            return result('ORACLE_ERROR', source_sha256=source_hash, oracle=meta,
                          diagnostic=a.getLastError())
        extra = {'load_result': loaded, 'main_module': module, 'sbml': xml}
        embedded = re.search(r'with libSBML version ([^ ]+)', xml)
        meta['antimony_bundled_libsbml_reported_by_xml'] = embedded.group(1).rstrip('.') if embedded else None
        a.clearPreviousLoads()
    else:
        xml = text
    if '<!DOCTYPE' in xml.upper() or '<!ENTITY' in xml.upper():
        return result('INPUT_REJECTED', reason='DTD/entity declarations are not accepted')
    doc = s.readSBMLFromString(xml)
    parse_messages = messages(doc)
    if any(m['severity'] in ('Error', 'Fatal') for m in parse_messages) or doc.getModel() is None:
        return result('SBML_INVALID', source_sha256=source_hash, oracle=meta, messages=parse_messages)
    limits = projection_limits(xml)
    if doc.getLevel() != 3 or doc.getVersion() != 2:
        limits.append('Only SBML L3V2 is represented')
    if limits:
        return result('PROJECTION_UNSUPPORTED', source_sha256=source_hash, oracle=meta, reasons=limits)
    doc.checkConsistency()
    all_messages = messages(doc)
    # Warnings are retained in full. They are not errors or blanket-suppressed.
    status = 'SBML_INVALID' if any(m['severity'] in ('Error', 'Fatal') for m in all_messages) else 'PASS'
    return result(status, source_sha256=source_hash, oracle=meta,
                  projection=projection(doc), messages=all_messages, **extra)

def run(mode, text='', python=None, timeout=30):
    if len(text.encode()) > MAX_INPUT:
        return result('INPUT_REJECTED', reason='Input exceeds 4 MiB')
    interpreter = str(python or ROOT / '.venv-oracle/bin/python')
    # No shell; controlled working directory and isolated Python import path.
    env = {k: v for k, v in os.environ.items() if not k.startswith('PYTHON')}
    try:
        with tempfile.TemporaryDirectory(prefix='antimony-oracle-') as cwd:
            child = subprocess.run([interpreter, '-I', str(Path(__file__).resolve()), '_worker'],
                input=json.dumps({'mode': mode, 'text': text}), text=True, capture_output=True,
                cwd=cwd, env=env, timeout=timeout, check=False)
    except FileNotFoundError as e:
        return result('ORACLE_BLOCKED', reason=str(e))
    except subprocess.TimeoutExpired:
        return result('ORACLE_ERROR', reason='Reference subprocess timeout')
    if child.returncode:
        return result('ORACLE_ERROR', reason=f'Reference subprocess exit {child.returncode}', stderr=child.stderr[-4000:])
    try:
        value = json.loads(child.stdout)
        if not isinstance(value, dict) or value.get('schema') != SCHEMA or 'status' not in value:
            raise ValueError('Unexpected worker protocol')
        return value
    except (ValueError, TypeError):
        return result('ORACLE_ERROR', reason='Invalid subprocess JSON', stdout=child.stdout[-1000:], stderr=child.stderr[-1000:])

def read_trusted(path):
    data = Path(path).read_bytes()
    if len(data) > MAX_INPUT:
        raise ValueError('Input exceeds 4 MiB')
    return data.decode('utf-8')

def main():
    if sys.argv[1:] == ['_worker']:
        try:
            emit(worker(json.load(sys.stdin)))
        except Exception as e:
            emit(result('ORACLE_ERROR', reason=f'{type(e).__name__}: {e}'))
        return 0
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--python', help='Reference Python interpreter (default .venv-oracle)')
    p.add_argument('mode', choices=['probe', 'convert', 'inspect'])
    p.add_argument('path', nargs='?')
    p.add_argument('--trusted', action='store_true', help='Acknowledge the input is locally trusted; this is not a sandbox')
    args = p.parse_args()
    if args.mode != 'probe' and (not args.trusted or not args.path):
        p.error('convert/inspect require a path and --trusted; never process untrusted imports')
    try:
        response = run(args.mode, read_trusted(args.path) if args.path else '', args.python)
    except (OSError, UnicodeError, ValueError) as e:
        response = result('INPUT_REJECTED', reason=str(e))
    emit(response)
    return 0 if response['status'] == 'PASS' else 2

if __name__ == '__main__':
    raise SystemExit(main())
