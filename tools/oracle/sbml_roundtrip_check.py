#!/usr/bin/env python3
"""SBML -> Antimony -> SBML roundtrip & RoadRunner verification gate.

Verifies that every accepted model across `fixtures/modular/manifest.json` and
`fixtures/collaboration/manifest.json` roundtrips through `export` ->
`import_sbml` -> `export` with byte-identical SBML, identical libSBML
structural projections, and (when `--roadrunner-python` is supplied) zero
trajectory divergence on all complete models.
"""
from __future__ import annotations
import argparse
import json
import os
import subprocess
import tempfile
from pathlib import Path

from oracle import ROOT, digest, emit, run
from corpus import fixture_source
from cli_check import cli_process
from collaboration_check import run_roadrunner_differential

MODULAR_MANIFEST = ROOT / 'fixtures/modular/manifest.json'
COLLAB_MANIFEST = ROOT / 'fixtures/collaboration/manifest.json'


def load_accepted_rows() -> list[dict]:
    rows = []
    for manifest_path in (MODULAR_MANIFEST, COLLAB_MANIFEST):
        data = json.loads(manifest_path.read_text())
        for row in data.get('fixtures', []):
            if row.get('native') == 'accepted':
                rows.append(row)
    if not rows:
        raise ValueError('No accepted fixtures found for SBML roundtrip check')
    return rows


def decompile_sbml_via_binary(import_bin: Path, sbml_bytes: bytes) -> bytes:
    with tempfile.NamedTemporaryFile(suffix='.xml', delete=False) as tmp:
        tmp.write(sbml_bytes)
        tmp_path = Path(tmp.name)
    try:
        proc = subprocess.run(
            [str(import_bin), str(tmp_path)],
            capture_output=True,
            timeout=15,
            check=False,
        )
        if proc.returncode != 0:
            stderr = proc.stderr.decode('utf-8', errors='replace').strip()
            raise ValueError(f'import_sbml failed (exit {proc.returncode}): {stderr}')
        return proc.stdout
    finally:
        tmp_path.unlink(missing_ok=True)


def check_roundtrip_case(row: dict, cli: Path, import_bin: Path, rr_python: str | None = None) -> dict:
    source = fixture_source(row)
    exp1 = cli_process(cli, 'export', source)
    if exp1.get('status') != 'SUCCESS':
        return {
            'id': row['id'],
            'readiness': row['readiness'],
            'status': 'FAIL',
            'reason': f"Initial export failed: {exp1.get('reason')}",
        }
    sbml1_bytes = exp1['artifact']
    try:
        decompiled_ant = decompile_sbml_via_binary(import_bin, sbml1_bytes)
    except ValueError as err:
        return {
            'id': row['id'],
            'readiness': row['readiness'],
            'status': 'FAIL',
            'reason': str(err),
        }

    exp2 = cli_process(cli, 'export', decompiled_ant.decode('utf-8'))
    if exp2.get('status') != 'SUCCESS':
        return {
            'id': row['id'],
            'readiness': row['readiness'],
            'status': 'FAIL',
            'reason': f"Re-export of decompiled Antimony failed: {exp2.get('reason')}",
        }
    sbml2_bytes = exp2['artifact']

    if sbml1_bytes != sbml2_bytes:
        return {
            'id': row['id'],
            'readiness': row['readiness'],
            'status': 'FAIL',
            'reason': 'Re-exported SBML differs from initial SBML',
        }

    sbml1_str = sbml1_bytes.decode('utf-8')
    sbml2_str = sbml2_bytes.decode('utf-8')
    expected = json.loads((ROOT / row['expectation']).read_text())
    proj2 = run('project', sbml2_str)
    if proj2.get('status') != 'PASS' or proj2.get('projection') != expected.get('projection'):
        return {
            'id': row['id'],
            'readiness': row['readiness'],
            'status': 'FAIL',
            'reason': 'libSBML structural projection after roundtrip differs from reference oracle projection',
        }

    record = {
        'id': row['id'],
        'readiness': row['readiness'],
        'status': 'PASS',
        'sbml_sha256': digest(sbml1_bytes),
        'decompiled_antimony_sha256': digest(decompiled_ant),
    }

    if row['readiness'] == 'complete':
        if rr_python:
            rr_res = run_roadrunner_differential(sbml1_str, sbml2_str, rr_python)
            record['numerical'] = rr_res['status']
            if rr_res['status'] != 'PASS':
                record['status'] = 'FAIL'
                record['reason'] = rr_res.get('reason', 'RoadRunner roundtrip trajectory mismatch')
        else:
            record['numerical'] = 'NOT_RUN'
    else:
        record['numerical'] = 'NOT_SIMULATION_READY'

    return record


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--cli', required=True, help='Path to prebuilt antimony_cli binary')
    parser.add_argument('--import-bin', required=True, help='Path to prebuilt import_sbml binary')
    parser.add_argument('--trusted-native', action='store_true')
    parser.add_argument('--roadrunner-python', help='Optional Python interpreter with libroadrunner')
    args = parser.parse_args()
    if not args.trusted_native:
        parser.error('--trusted-native is required')
    cli = Path(args.cli).resolve()
    import_bin = Path(args.import_bin).resolve()
    for b in (cli, import_bin):
        if not b.is_file() or not os.access(b, os.X_OK):
            parser.error(f'Binary missing or not executable: {b}')
    try:
        rows = load_accepted_rows()
        records = [check_roundtrip_case(row, cli, import_bin, args.roadrunner_python) for row in rows]
        ok = bool(records) and all(r['status'] == 'PASS' for r in records)
        emit({
            'schema': 'antimony-sbml-roundtrip-check-v1',
            'status': 'PASS' if ok else 'FAIL',
            'counts': {
                'cases': len(records),
                'complete_cases': sum(r['readiness'] == 'complete' for r in records),
                'incomplete_cases': sum(r['readiness'] == 'incomplete' for r in records),
                'numerical_passed': sum(r.get('numerical') == 'PASS' for r in records),
                'failed': sum(r['status'] != 'PASS' for r in records),
            },
            'cases': records,
        })
        return 0 if ok else 2
    except (OSError, ValueError, KeyError, TypeError) as err:
        emit({'schema': 'antimony-sbml-roundtrip-check-v1', 'status': 'FAIL', 'reason': str(err)})
        return 2


if __name__ == '__main__':
    raise SystemExit(main())
