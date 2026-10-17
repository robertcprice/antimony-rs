#!/usr/bin/env python3
"""Two bounded builtin/time trajectories against independent analytic solutions."""
import argparse
from pathlib import Path
import check as shared
import builtin_anchors
import profile_overlay

SCHEMA = 'antimony-builtin-numerical-check-v1'


def fingerprint(manifest,native,python):
    values=shared.fingerprints(manifest,native,python)
    for path in [builtin_anchors.MANIFEST,Path(__file__),Path(builtin_anchors.__file__),shared.ROOT/'profiles/math-core-v0.2.json',profile_overlay.boundary_path()]:
        values[str(path.relative_to(shared.ROOT))]=shared.digest(path.read_bytes())
    return values


def check_all(native,python=None,reference_python=None):
    manifest=builtin_anchors.load_manifest()
    try:before=fingerprint(manifest,native,python)
    except FileNotFoundError as e:
        return {'schema':SCHEMA,'status':'ORACLE_BLOCKED','reason':str(e),'counts':{'passed':0,'blocked':2,'failed':0},'cases':[]}
    cases=[shared.check_case(c,native,python,reference_python,anchor_contract=builtin_anchors) for c in manifest['cases']]
    try:after=fingerprint(manifest,native,python)
    except OSError:after={}
    changed=[k for k,v in before.items() if after.get(k)!=v]
    counts={'passed':sum(c['status']=='PASS' for c in cases),'blocked':sum(c['status']=='ORACLE_BLOCKED' for c in cases),'failed':sum(c['status'] not in ('PASS','ORACLE_BLOCKED') for c in cases)}
    return {'schema':SCHEMA,'implemented_profile':profile_overlay.PROFILE,'corpus_profile':'math-core-v0.2-slice','status':'PASS' if counts['passed']==2 and not changed else 'FAIL',
            'counts':counts,'cases':cases,'input_sha256':before,'changed_inputs':changed,
            'grid':shared.GRID,'comparison':shared.COMPARISON,'solver':shared.SOLVER,
            'scope':'Two independently derived exp/sin/time source models only; not all builtins, domains, dimensional certification or simulation readiness.'}


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--native',required=True);parser.add_argument('--trusted-native',action='store_true')
    parser.add_argument('--python');parser.add_argument('--reference-python')
    args=parser.parse_args()
    if not args.trusted_native:parser.error('Explicit --trusted-native required; no builds or installation')
    try:
        result=check_all(Path(args.native).resolve(),str(Path(args.python).absolute()) if args.python else None,
                         str(Path(args.reference_python).absolute()) if args.reference_python else None)
    except (OSError,ValueError,TypeError,KeyError) as e:result={'schema':SCHEMA,'status':'FAIL','reason':str(e)}
    shared.emit(result);return 0 if result['status']=='PASS' else 2

if __name__=='__main__':raise SystemExit(main())
