"""Bounded protocol tests; live tests are explicit and unavailable tools are skips."""
import copy
import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parent))
import worker

SBML = '''<sbml xmlns="http://www.sbml.org/sbml/level3/version2/core" level="3" version="2"><model id="probe"><listOfCompartments><compartment id="c" size="2" constant="true"/></listOfCompartments><listOfSpecies><species id="S" compartment="c" initialConcentration="4" hasOnlySubstanceUnits="false" boundaryCondition="false" constant="false"/><species id="P" compartment="c" initialConcentration="0" hasOnlySubstanceUnits="false" boundaryCondition="false" constant="false"/></listOfSpecies><listOfParameters><parameter id="k" value="3" constant="true"/></listOfParameters></model></sbml>'''


def request(sbml=SBML):
    return dict(schema=worker.SCHEMA, sbml=sbml,
                sbml_sha256=hashlib.sha256(sbml.encode()).hexdigest(),
                start=0, end=10, steps=100, selections=['time', '[S]', 'S', '[P]', 'P', 'k'],
                solver=copy.deepcopy(worker.SOLVER))


class ProtocolTests(unittest.TestCase):
    def test_valid(self):
        self.assertEqual(worker.validate(request()), request()['sbml_sha256'])

    def test_fields(self):
        for update in [dict(extra=1), dict(schema='wrong')]:
            with self.assertRaises(ValueError):
                worker.validate(dict(request(), **update))

    def test_digest(self):
        with self.assertRaises(ValueError):
            worker.validate(dict(request(), sbml_sha256='0'*64))

    def test_uri_is_not_sbml(self):
        for value in ['https://example.invalid/model.xml', '/tmp/model.xml']:
            self.assertEqual(worker.run(request(value))['status'], 'ORACLE_ERROR')

    def test_entities(self):
        for declaration in ['<!DOCTYPE sbml>', '<!ENTITY x SYSTEM "file:///etc/passwd">']:
            self.assertEqual(worker.run(request(declaration + SBML))['status'], 'ORACLE_ERROR')

    def test_namespace(self):
        self.assertEqual(worker.run(request('<sbml/>'))['status'], 'ORACLE_ERROR')

    def test_grid(self):
        for update in [dict(start=True),dict(end=11),dict(steps=100.0),dict(steps=99)]:
            self.assertEqual(worker.run(dict(request(), **update))['status'], 'ORACLE_ERROR')

    def test_solver(self):
        for key,value in [('stiff',1),('relative_tolerance',1e-5),('absolute_tolerance',1e-10),('name','rk4')]:
            req=request();req['solver'][key]=value
            self.assertEqual(worker.run(req)['status'], 'ORACLE_ERROR')

    def test_selections(self):
        for sels in [[],['S','time'],['time','S','S'],['time','S\n'],['time',7],['time','S\'']]:
            self.assertEqual(worker.run(dict(request(),selections=sels))['status'],'ORACLE_ERROR')

    def test_duplicate_json(self):
        with self.assertRaises(ValueError):
            json.loads('{"x":1,"x":2}',object_pairs_hook=worker.unique_object)

    def test_nonfinite(self):
        for x in [float('nan'),float('inf'),-float('inf')]:
            with self.assertRaises(ValueError): worker.plain([x])

    def test_missing_runtime_blocked(self):
        with patch.object(worker.importlib.metadata,'version',side_effect=worker.importlib.metadata.PackageNotFoundError('missing')):
            self.assertEqual(worker.run(request())['status'],'ORACLE_BLOCKED')

    def test_wrong_version_blocked(self):
        with patch.object(worker.importlib.metadata,'version',return_value='0.0'):
            self.assertEqual(worker.run(request())['status'],'ORACLE_BLOCKED')

    def test_sbml_cap(self):
        with patch.object(worker,'MAX_SBML_BYTES',3):
            self.assertEqual(worker.run(request())['status'],'ORACLE_ERROR')

    def test_invalid_utf8_and_nonfinite_json(self):
        for data in [b"\xff", b"{\"x\": NaN}"]:
            p=subprocess.run([sys.executable,"-I",str(Path(worker.__file__).resolve())],input=data,capture_output=True,timeout=10)
            self.assertEqual(p.returncode,1);self.assertEqual(json.loads(p.stdout)["status"],"ORACLE_ERROR")

    def test_json_byte_cap(self):
        p=subprocess.run([sys.executable,"-I",str(Path(worker.__file__).resolve())],input=b" "*(worker.MAX_INPUT_BYTES+1),capture_output=True,timeout=10)
        self.assertEqual(p.returncode,1);self.assertIn("byte limit",json.loads(p.stdout)["reason"])

    def test_unpaired_surrogate(self):
        req=request();req["sbml"]="\ud800"
        self.assertEqual(worker.run(req)["status"],"ORACLE_ERROR")

    def test_subprocess_bad_json(self):
        p=subprocess.run([sys.executable,'-I',str(Path(worker.__file__).resolve())],input=b'{bad',capture_output=True,timeout=10)
        self.assertEqual(p.returncode,1);self.assertEqual(json.loads(p.stdout)['status'],'ORACLE_ERROR')
        self.assertEqual(p.stderr,b'')


@unittest.skipUnless(importlib.util.find_spec('roadrunner'), 'pinned numerical environment required')
class LiveTests(unittest.TestCase):
    def test_amount_concentration_grid_and_readback(self):
        result=worker.run(request());self.assertEqual(result['status'],'PASS',result)
        self.assertEqual(len(result['timeseries']),101)
        for index,row in enumerate(result['timeseries']): self.assertAlmostEqual(row[0],index/10,places=12)
        self.assertEqual(result['column_names'],request()['selections'])
        self.assertEqual(result['timeseries'][0],[0,4,8,0,0,3])
        self.assertEqual(result['timeseries'][-1],[10,4,8,0,0,3])
        self.assertEqual(result['effective_absolute_tolerance_before'],[8e-12,2e-12])
        for key,value in worker.SOLVER.items():self.assertEqual(result['solver_readback'][key],value)

    def test_unknown_selection_error(self):
        self.assertEqual(worker.run(dict(request(),selections=['time','unknown']))['status'],'ORACLE_ERROR')

    def test_isolated_process(self):
        p=subprocess.run([sys.executable,'-I',str(Path(worker.__file__).resolve())],input=json.dumps(request()).encode(),capture_output=True,timeout=60,cwd='/tmp')
        self.assertEqual(p.returncode,0,p.stderr)
        self.assertEqual(json.loads(p.stdout)['status'],'PASS');self.assertEqual(p.stderr,b'')

if __name__=='__main__': unittest.main()
