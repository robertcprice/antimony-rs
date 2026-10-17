"""Supplement-only audit of reference exports with nonfinite MathML.

Never promotes these outputs to the finite exact semantic projection. Ordinary
cases continue through oracle.run unchanged. This process is not a sandbox.
"""
import json
import math
import os
from pathlib import Path
import subprocess
import sys
sys.path.insert(0, str(Path(__file__).resolve().parent))
import oracle

SCHEMA = 'antimony-rational-nonfinite-reference-v1'


def worker(source):
    a, s = oracle.dependencies()
    meta = oracle.metadata(a, s)
    a.clearPreviousLoads(); a.setWriteSBMLTimestamp(False)
    loaded = a.loadAntimonyString(source)
    if not oracle.load_succeeded(loaded):
        return {'schema': SCHEMA, 'status': 'REFERENCE_REJECTED', 'diagnostic': a.getLastError()}
    xml = a.getSBMLString(a.getMainModuleName())
    a.clearPreviousLoads()
    doc = s.readSBMLFromString(xml)
    if doc.getModel() is None:
        raise ValueError('No reference model')
    doc.checkConsistency()
    messages = oracle.messages(doc)
    if any(m['severity'] in ('Error', 'Fatal') for m in messages):
        return {'schema': SCHEMA, 'status': 'SBML_INVALID', 'messages': messages}
    limits = oracle.projection_limits(xml)
    # Nonfinite tags are audited only for an explicit native rejection contract;
    # they remain excluded from oracle.MATH_TAGS and exact projection.
    if set(limits) - {'Unrepresented MathML element: infinity', 'Unrepresented MathML element: notanumber'}:
        return {'schema': SCHEMA, 'status': 'PROJECTION_UNSUPPORTED', 'reasons': limits}
    m = doc.getModel()
    roots = [(x.getSymbol(), x.getMath()) for x in m.getListOfInitialAssignments()]
    roots += [(x.getVariable(), x.getMath()) for x in m.getListOfRules()]
    roots += [(x.getId(), x.getKineticLaw().getMath()) for x in m.getListOfReactions() if x.isSetKineticLaw()]
    roots += [(x.getId(), x.getMath()) for x in m.getListOfFunctionDefinitions() if x.isSetMath()]
    nonfinite = []
    for owner, root in roots:
        stack = [root]
        while stack:
            node = stack.pop()
            if node is None:
                raise ValueError('Reference math missing')
            if node.isReal() and not math.isfinite(node.getReal()):
                value = node.getReal()
                nonfinite.append({'owner': owner, 'ast_type': node.getType(), 'value': 'NaN' if math.isnan(value) else '-INF' if value < 0 else 'INF'})
            stack.extend(node.getChild(i) for i in range(node.getNumChildren()))
    if not nonfinite:
        raise ValueError('Expected nonfinite reference math was not present')
    return {'schema': SCHEMA, 'status': 'REFERENCE_NONFINITE', 'source_sha256': oracle.digest(source.encode()),
            'oracle': meta, 'load_result': loaded, 'sbml': xml, 'sbml_sha256': oracle.digest(xml.encode()),
            'nonfinite_math': nonfinite, 'messages': messages, 'exact_projection': 'NOT_REPRESENTED'}


def run(source, python=None, timeout=30):
    if len(source.encode()) > oracle.MAX_INPUT:
        return {'schema': SCHEMA, 'status': 'INPUT_REJECTED'}
    env = {k: v for k, v in os.environ.items() if not k.startswith('PYTHON')}
    try:
        import tempfile
        with tempfile.TemporaryDirectory(prefix='rational-reference-') as cwd:
            p = subprocess.run([str(python or oracle.ROOT/'.venv-oracle/bin/python'), '-I', str(Path(__file__).resolve()), '--worker'],
                               input=json.dumps({'source': source}), capture_output=True, text=True, cwd=cwd, env=env, timeout=timeout)
        if p.returncode:
            return {'schema': SCHEMA, 'status': 'ORACLE_ERROR', 'reason': f'Worker exit {p.returncode}'}
        value = json.loads(p.stdout)
        if not isinstance(value, dict) or value.get('schema') != SCHEMA or 'status' not in value:
            raise ValueError('Invalid worker response')
        return value
    except FileNotFoundError as e:
        return {'schema': SCHEMA, 'status': 'ORACLE_BLOCKED', 'reason': str(e)}
    except (subprocess.TimeoutExpired, ValueError) as e:
        return {'schema': SCHEMA, 'status': 'ORACLE_ERROR', 'reason': str(e)}


def snapshot(value):
    return {k: value[k] for k in ('schema','status','source_sha256','sbml','sbml_sha256','nonfinite_math','messages','exact_projection')} | {
        'oracle_versions': value['oracle']['packages'], 'antimony_library_version': value['oracle']['antimony_library_version']}


if __name__ == '__main__':
    if sys.argv[1:] != ['--worker']:
        raise SystemExit('Only isolated worker invocation supported')
    try:
        oracle.emit(worker(json.load(sys.stdin)['source']))
    except Exception as e:
        oracle.emit({'schema': SCHEMA, 'status': 'ORACLE_ERROR', 'reason': f'{type(e).__name__}: {e}'})
