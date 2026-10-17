#!/usr/bin/env python3
"""Explicit bounded structural workload gate; no build, download or simulation."""
from __future__ import annotations
import argparse
import json
import os
from pathlib import Path
import re
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'tools/oracle'))
from oracle import digest, emit, run
import native_check as native
from roundtrip_check import native_format
import generate
import generated_contracts as contracts
import motifs
import profile_overlay

SCHEMA = 'antimony-workload-check-v1'
STAGES = ('reference', 'reference_contract', 'native_export', 'native_contract',
          'format', 'format_idempotence', 'formatted_reference',
          'formatted_reference_contract', 'formatted_native', 'formatted_native_contract')
MAX_SOURCE = 8192
FAILURES = ROOT / 'artifacts/workloads-failures'


def stable_json(value):
    return json.dumps(value, sort_keys=True, separators=(',', ':'), allow_nan=False)


def core_diagnostics(stderr):
    lines = stderr.splitlines()
    if any(not re.fullmatch(r'stage=resolve severity=(Warning|Note) code=[a-z_]+ message=.*', line) for line in lines):
        raise ValueError('Unexpected successful native stderr protocol')
    return lines


def prepare_cases(current_profile=False):
    result = []
    for case in motifs.get_cases():
        path = (ROOT / case['path']).resolve()
        if not path.is_relative_to((ROOT / 'fixtures/workloads').resolve()):
            raise ValueError('Motif path escapes owned fixtures')
        text = path.read_text()
        if digest(text.encode()) != case['sha256']:
            raise ValueError('Motif source hash mismatch')
        result.append((case, text, motifs.expected_projection(case['expectation_id'])))
    for case in generate.validate_receipt():
        result.append((case, generate.render(case['recipe']), profile_overlay.workload(case, contracts.expected(case)) if current_profile else contracts.expected(case)))
    if len(result) != 86 or len({c['id'] for c,_,_ in result}) != 86:
        raise ValueError('Exact nonduplicate six-motif/eighty-generated inventory required')
    return result


def input_paths(native_path, formatter_path):
    paths = {'native':Path(native_path), 'formatter':Path(formatter_path),
             'profile':ROOT/'profiles/kinetic-core-v0.1.json',
             'motif_manifest':motifs.MANIFEST, 'generated_receipt':generate.RECEIPT}
    for folder,names in [('tools/workloads',('check.py','generate.py','generated_contracts.py','motifs.py')),
                         ('tools/oracle',('oracle.py','native_check.py','roundtrip_check.py','contracts.py','corpus.py'))]:
        paths.update({folder+'/'+n:ROOT/folder/n for n in names})
    paths.update({c['path']:ROOT/c['path'] for c in motifs.get_cases()})
    return paths


def fingerprint(paths):
    result = {}
    for name,path in paths.items():
        try:result[name]=digest(path.read_bytes())
        except OSError:result[name]=None
    return result


def check_case(case, source, expected_projection, exporter, formatter, python=None):
    contract = expected_projection if 'reference_status' in expected_projection else {
        'reference_status':'PASS','native_status':'EXPORTED','projection':expected_projection}
    record={'schema':'antimony-workload-case-v1','id':case['id'],'kind':case['kind'],
            'source_sha256':digest(source.encode()),'expected_contract_sha256':digest(stable_json(contract).encode()),
            'status':'FAIL','stages':dict.fromkeys(STAGES,'NOT_RUN'),'numerical':'NOT_RUN',
            'warnings':{},'core_diagnostics':{},'artifact_sha256':{}}
    details={'case':case,'source':source,'expected_contract':contract}
    stages=record['stages']; active='reference'
    try:
        expected_hash=case.get('sha256',case.get('source_sha256'))
        if record['source_sha256']!=expected_hash or len(source.encode())>MAX_SOURCE or 'import' in source.lower():
            raise ValueError('Source hash/size/trusted-input precondition failed')
        ref=run('convert',source,python); details['reference']=ref
        record['reference_status']=ref['status']
        if ref['status']=='ORACLE_BLOCKED':
            stages[active]='ORACLE_BLOCKED'; record['status']='ORACLE_BLOCKED'
            record['reason']=ref.get('reason','Reference unavailable');return record
        if ref['status']!=contract['reference_status']:
            raise ValueError('Reference status differs from predeclared contract')
        stages[active]='PASS';active='reference_contract'
        if contract['projection'] is not None:contracts.same(contract['projection'],ref['projection'])
        elif contract['reference_diagnostic_contains'] not in ref.get('diagnostic',''):
            raise ValueError('Reference rejection is not the reviewed cycle diagnosis')
        stages[active]='PASS';record['oracle']=ref['oracle']
        record['warnings']['reference']=ref.get('messages',[])
        if 'sbml' in ref:record['artifact_sha256']['reference_sbml']=digest(ref['sbml'].encode())
        active='native_export';out=native.native_export(exporter,source);details['native']=out
        record['native_status']=out['status']
        if out['status']!=contract['native_status']:raise ValueError('Native status differs from predeclared contract')
        stages[active]='PASS' if out['status']=='EXPORTED' else 'EXPECTED_REJECTION'
        active='native_contract'
        if out['status']=='REJECTED':
            diagnostic=out['diagnostic'];record['native_diagnostic']=diagnostic
            if (diagnostic['stage'],diagnostic['code'])!=(contract['native_stage'],contract['native_code']):
                raise ValueError('Native rejection stage/code differs from predeclared contract')
            stages[active]='PASS';active='format'
            fmt=native_format(formatter,source);details['formatter']=serializable_format(fmt)
            if fmt['status']!='REJECTED' or (fmt['diagnostic']['stage'],fmt['diagnostic']['code'])!=(contract['native_stage'],contract['native_code']):
                raise ValueError('Formatter rejection stage/code differs from predeclared contract')
            record['formatter_diagnostic']=fmt['diagnostic'];stages[active]='EXPECTED_REJECTION'
            record.update(status='PASS',outcome='expected_rejection');return record
        inspection=run('inspect',out['xml'],python);details['native_inspection']=inspection
        if inspection['status']=='ORACLE_BLOCKED':
            record['status']='ORACLE_BLOCKED';stages[active]='ORACLE_BLOCKED';return record
        if inspection['status']!='PASS':raise ValueError('Native SBML independent inspection failed')
        contracts.same(contract['projection'],inspection['projection']);stages[active]='PASS'
        record['warnings']['native']=inspection['messages'];record['core_diagnostics']['native']=core_diagnostics(out['stderr'])
        record['artifact_sha256']['native_sbml']=out['sbml_sha256']
        active='format';fmt=native_format(formatter,source);details['formatter']=serializable_format(fmt)
        if fmt['status']!='FORMATTED':raise ValueError('Formatter failed supported case')
        record['core_diagnostics']['formatter']=core_diagnostics(fmt['stderr'])
        if record['core_diagnostics']['formatter']!=record['core_diagnostics']['native']:
            raise ValueError('Formatter/native warning sequence drift')
        stages[active]='PASS';active='format_idempotence'
        second=native_format(formatter,fmt['text']);details['second_formatter']=serializable_format(second)
        if second['status']!='FORMATTED' or second['bytes']!=fmt['bytes']:
            raise ValueError('Formatter is not byte idempotent')
        stages[active]='PASS';record['artifact_sha256']['formatted']=fmt['formatted_sha256']
        active='formatted_reference';ref2=run('convert',fmt['text'],python);details['formatted_reference']=ref2
        if ref2['status']=='ORACLE_BLOCKED':
            record['status']='ORACLE_BLOCKED';stages[active]='ORACLE_BLOCKED';return record
        if ref2['status']!='PASS':raise ValueError('Formatted reference failed')
        stages[active]='PASS';active='formatted_reference_contract'
        contracts.same(contract['projection'],ref2['projection']);stages[active]='PASS'
        record['warnings']['formatted_reference']=ref2['messages']
        active='formatted_native';out2=native.native_export(exporter,fmt['text']);details['formatted_native']=out2
        if out2['status']!='EXPORTED':raise ValueError('Formatted native export failed')
        stages[active]='PASS';active='formatted_native_contract'
        inspection2=run('inspect',out2['xml'],python);details['formatted_native_inspection']=inspection2
        if inspection2['status']=='ORACLE_BLOCKED':
            record['status']='ORACLE_BLOCKED';stages[active]='ORACLE_BLOCKED';return record
        if inspection2['status']!='PASS':raise ValueError('Formatted native SBML inspection failed')
        contracts.same(contract['projection'],inspection2['projection']);stages[active]='PASS'
        record['warnings']['formatted_native']=inspection2['messages']
        record['core_diagnostics']['formatted_native']=core_diagnostics(out2['stderr'])
        if record['core_diagnostics']['formatted_native']!=record['core_diagnostics']['native']:
            raise ValueError('Reparsed native warning sequence drift')
        record['artifact_sha256']['formatted_native_sbml']=out2['sbml_sha256']
        record.update(status='PASS',outcome='supported_motif' if case['kind']=='motif' else 'supported_generated')
    except (ValueError,KeyError,TypeError,OSError,UnicodeError,OverflowError) as error:
        stages[active]='FAIL';record['reason']=str(error)
    finally:
        if record['status']=='FAIL':record['_failure_bundle']=details
    return record


def serializable_format(result):
    return {k:v for k,v in result.items() if k!='bytes'}


def retain_failure(record, fingerprints, ordinal):
    bundle=record.pop('_failure_bundle',None)
    if bundle is None:return
    if ordinal>=20:
        record['retention']={'status':'LIMIT_REACHED','limit':20};return
    FAILURES.mkdir(parents=True,exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='.staging-',dir=FAILURES) as staging:
        # JSON captures original source, expected contract, each available process
        # result and XML. Fresh directory, never overwrite a prior discrepancy.
        payload={**bundle,'record':record,'input_sha256':fingerprints,
                 'minimization':{'status':'NOT_RUN','reason':'Automatic reduction not reviewed; preserve original discrepancy first.'}}
        data=json.dumps(payload,indent=2,sort_keys=True,allow_nan=False).encode()
        if len(data)>32*1024*1024:raise ValueError('Failure bundle exceeds retained byte budget')
        Path(staging,'failure.json').write_bytes(data)
        target=Path(tempfile.mkdtemp(prefix=record['id']+'-',dir=FAILURES))
        Path(staging,'failure.json').replace(target/'failure.json')
        record['retention']={'status':'RETAINED','path':str((target/'failure.json').relative_to(ROOT)),
                             'sha256':digest(data),'minimization':'NOT_RUN'}


def check_all(exporter,formatter,python=None,current_profile=False):
    paths=input_paths(exporter,formatter)
    if current_profile:
        paths.update(overlay=profile_overlay.PATH,overlay_checker=Path(profile_overlay.__file__))
    before=fingerprint(paths)
    if any(v is None for v in before.values()) or any(not os.access(Path(p),os.X_OK) for p in (exporter,formatter)):
        return {'schema':SCHEMA,'status':'ORACLE_BLOCKED','reason':'Required prebuilt binary or fingerprint input unavailable',
                'counts':{'motifs':0,'generated_supported':0,'expected_rejections':0,'failed_or_blocked':86,'numerical_not_run':86},
                'cases':[],'input_sha256':before,'changed_inputs':[]}
    prepared=prepare_cases(current_profile) if current_profile else prepare_cases()
    after_preparation=fingerprint(paths)
    changed=[k for k,v in before.items() if after_preparation.get(k)!=v]
    if changed:
        return {'schema':SCHEMA,'status':'FAIL','reason':'Inputs changed while preparing cached sources',
                'counts':{'motifs':0,'generated_supported':0,'expected_rejections':0,'failed_or_blocked':86,'numerical_not_run':86},
                'cases':[],'input_sha256':before,'changed_inputs':changed}
    records=[];retained=0
    for case,text,expected in prepared:
        record=check_case(case,text,expected,exporter,formatter,python)
        if record['status']=='FAIL':
            try:retain_failure(record,before,retained)
            except (OSError,ValueError) as error:record.pop('_failure_bundle',None);record['retention']={'status':'FAIL','reason':str(error)}
            retained+=1
        records.append(record)
    after=fingerprint(paths);changed=[k for k,v in before.items() if after.get(k)!=v]
    counts={'motifs':sum(r.get('outcome')=='supported_motif' for r in records),
            'generated_supported':sum(r.get('outcome')=='supported_generated' for r in records),
            'expected_rejections':sum(r.get('outcome')=='expected_rejection' for r in records),
            'failed_or_blocked':sum(r['status']!='PASS' for r in records),'numerical_not_run':len(records)}
    required={'motifs':6,'generated_supported':68 if current_profile else 64,'expected_rejections':12 if current_profile else 16,'failed_or_blocked':0,'numerical_not_run':86}
    return {'schema':SCHEMA,'status':'PASS' if not changed and counts==required and all(r['status']=='PASS' for r in records) else 'FAIL',
            'counts':counts,'cases':records,'input_sha256':before,'changed_inputs':changed,
            'implemented_profile':profile_overlay.PROFILE if current_profile else 'historical', 'generator_version':generate.VERSION,'root_seed':generate.ROOT_SEED,
            'scope':'Six original structural motifs and eighty finite-grid cases; not fuzzing, numerical validation or full-language coverage.'}


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--native',required=True);parser.add_argument('--formatter',required=True)
    parser.add_argument('--trusted-native',action='store_true');parser.add_argument('--python')
    parser.add_argument('--current-profile',action='store_true')
    args=parser.parse_args()
    if not args.trusted_native:parser.error('Explicit --trusted-native required; no automatic builds')
    try:report=check_all(Path(args.native).resolve(),Path(args.formatter).resolve(),args.python,args.current_profile)
    except (OSError,ValueError,TypeError,KeyError) as error:report={'schema':SCHEMA,'status':'FAIL','reason':str(error)}
    emit(report);return 0 if report['status']=='PASS' else 2


if __name__=='__main__':raise SystemExit(main())
