"""Versioned finite parameter grid, not random fuzzing or a language grammar.

Rendering knows no SBML projection API. Independent expectations live elsewhere.
"""
import hashlib
import json
from pathlib import Path

VERSION = 'antimony-structured-grid-v1'
ROOT_SEED = '2026-10-10-grid-01'
ROOT = Path(__file__).resolve().parents[2]
RECEIPT = Path(__file__).with_name('generated-v1.json')
FAMILIES = ('dag', 'compartments', 'units', 'numeric')
NEGATIVES = ('cycle', 'dimensions', 'unset', 'envelope')
LITERALS = ('0', '-0', '0e-999', '1e-307', '-1e-307', '1e308', '-1e308',
            '1.23456789012345', '0002.50', '2e3', '2E-3', '-2', '+2', '(3)', '(-2)', '2+0')
VOLUMES = (1, 2, 3, 5)
UNIT_TEXT = ('2second', '0.001mole', 'mole*litre', 'litre*mole')
REJECTED_LITERALS = ('1.1e308', '-1.1e308', '1.234567890123456', '-1.234567890123456')


def sha(data):
    return hashlib.sha256(data).hexdigest()


def recipe(family, index):
    if family in FAMILIES and type(index) is int and 0 <= index < 16:
        return {'family': family, 'index': index}
    if family in NEGATIVES and type(index) is int and 0 <= index < 4:
        return {'family': family, 'index': index}
    raise ValueError('Outside frozen parameter grid')


def render(spec):
    family, i = spec['family'], spec['index']
    recipe(family, i)
    name = 'grid_' + family + f'_{i:02}'
    if family == 'dag':
        base, factor = i + 1, i % 3 + 2
        dependency = 'S' if i % 2 else 'a'
        before = 'S=2; P=0; ' if i % 2 else ''
        after = ' J: ' + ('$S' if i % 4 == 3 else 'S') + ' => P; k*S;' if i % 2 else ''
        body = before + f'formula z=b+a,b={dependency}*{factor},a={base},k=0.25;' + after
    elif family == 'compartments':
        c1, c2 = VOLUMES[i // 4], VOLUMES[i % 4]
        rate = 'k*S*c1' if i % 2 else 'k*S'
        body = f'compartment c1={c1}; compartment c2={c2}; species S in c1,P in c2; S=3; P=1; k=0.5; J:S=>P;{rate};'
    elif family == 'units':
        body = f'unit u={UNIT_TEXT[i // 4]}; p={i % 4 + 1}; p has u;'
    elif family == 'numeric':
        body = 'a=' + LITERALS[i] + ';'
    elif family == 'cycle':
        body = f'a=b+{i+1};b=a+{i+2};'
    elif family == 'dimensions':
        body = f'a={i+1};a has second;b={i+2};b has mole;p=a+b;p has second;'
    elif family == 'unset':
        body = f'formula a; b={i+1};'
    else:
        body = 'a=' + REJECTED_LITERALS[i] + ';'
    # Only trivia/header spelling varies; wrapping scalar literals is NOT a no-op.
    header = name + ('()' if i % 2 else '')
    separator = '\r\n' if i % 4 == 1 else '\n'
    return f'model {header}{separator}// original deterministic grid {family}/{i}\n{body}\nend\n'


def cases():
    result = []
    for family, count in [(f, 16) for f in FAMILIES] + [(f, 4) for f in NEGATIVES]:
        for i in range(count):
            spec = recipe(family, i)
            text = render(spec)
            result.append({'id': f'grid_{family}_{i:02}',
                           'kind': 'generated_positive' if family in FAMILIES else 'generated_negative',
                           'recipe': spec, 'seed_id': sha(f'{VERSION}\0{ROOT_SEED}\0{family}\0{i}'.encode()),
                           'source_sha256': sha(text.encode()), 'numerical': 'NOT_RUN'})
    return result


def receipt():
    return {'schema': 'antimony-generated-grid-v1', 'generator_version': VERSION,
            'root_seed': ROOT_SEED, 'generation': 'finite ordered parameter grid; no PRNG',
            'cases': cases()}


def validate_receipt():
    data = json.loads(RECEIPT.read_text())
    if data != receipt():
        raise ValueError('Frozen generated inventory/source hashes changed; review required')
    if len({c['source_sha256'] for c in data['cases']}) != 80:
        raise ValueError('Duplicate generated source bytes')
    return data['cases']


if __name__ == '__main__':
    # Printing is explicit, never overwrite a frozen receipt or capture oracle truth.
    print(json.dumps(receipt(), indent=2, sort_keys=True))
