"""Offline unit/controlled-subprocess tests; never builds the project."""
import copy
import json
import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parent))
import run as r


class ReceiptTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name).resolve()
        (self.root/'Cargo.toml').write_text('[package]\nname="test"\nversion="0.1.0"\n')
        (self.root/'Cargo.lock').write_text('version=4\npackage=[]\n')
        (self.root/'src').mkdir()
        (self.root/'src/lib.rs').write_text('// original')

    def test_source_addition_deletion_mutation(self):
        before = r.source_snapshot(self.root)
        (self.root/'src/lib.rs').unlink()
        (self.root/'src/new.rs').write_text('// new')
        self.assertEqual(r.changed(before,r.source_snapshot(self.root)),['src/lib.rs','src/new.rs'])
        (self.root/'Cargo.toml').write_text('modified')
        self.assertIn('Cargo.toml',r.changed(before,r.source_snapshot(self.root)))

    def test_source_excludes_caches_receipts_not_tools(self):
        for name in ['tools/oracle/native-evidence.json','tools/numerical/evidence.json',
                     'tools/verification/__pycache__/run.pyc','target/object','artifacts/log']:
            path=self.root/name;path.parent.mkdir(parents=True,exist_ok=True);path.write_text('ignored')
        (self.root/'tools/verification/run.py').write_text('runner source')
        snap=r.source_snapshot(self.root)
        self.assertIn('tools/verification/run.py',snap)
        self.assertFalse(any('evidence.json' in p or '__pycache__' in p or p.startswith(('target/','artifacts/')) for p in snap))

    def test_source_rejects_symlink(self):
        (self.root/'src/linked.rs').symlink_to(self.root/'Cargo.toml')
        with self.assertRaises(ValueError):r.source_snapshot(self.root)

    def test_dangling_optional_source_rejected(self):
        (self.root/"build.rs").symlink_to(self.root/"missing")
        with self.assertRaises(ValueError):r.source_snapshot(self.root)

    def test_config_rejects_external_wrapper(self):
        config=self.root/".cargo/config.toml";config.parent.mkdir()
        config.write_text('[build]\nrustc-wrapper="/external/tool"\n')
        with self.assertRaises(ValueError):r.config_snapshot(self.root,{"HOME":str(self.root)})

    def test_missing_lock_rejected(self):
        (self.root/'Cargo.lock').unlink()
        with self.assertRaises(ValueError):r.source_snapshot(self.root)

    def test_config_new_deleted_and_include(self):
        home=self.root/'home';home.mkdir();env={'HOME':str(home),'CARGO_HOME':str(home)}
        before=r.config_snapshot(self.root,env)
        config=self.root/'.cargo/config.toml';config.parent.mkdir();config.write_text('[build]\njobs=1\n')
        after=r.config_snapshot(self.root,env)
        self.assertIn(str(config),r.changed(before,after))
        config.unlink();self.assertIn(str(config),r.changed(after,r.config_snapshot(self.root,env)))
        config.write_text('include="other.toml"\n')
        with self.assertRaises(ValueError):r.config_snapshot(self.root,env)

    def test_environment_excludes_secrets_and_wrappers(self):
        with patch.dict(os.environ,{'SECRET_TOKEN':'private','RUSTC_WRAPPER':'evil','RUSTFLAGS':'injected'}):
            env=r.environment(self.root,{'rustc':'/trusted/rustc','rustdoc':'/trusted/rustdoc'})
        self.assertNotIn('SECRET_TOKEN',env);self.assertNotIn('RUSTC_WRAPPER',env);self.assertNotIn('RUSTFLAGS',env)
        self.assertEqual(env['RUSTC'],'/trusted/rustc');self.assertEqual(env['CARGO_NET_OFFLINE'],'true')

    def test_dependencies_capture_tree_change(self):
        base=self.root/'registry/pkg';base.mkdir(parents=True);(base/'Cargo.toml').write_text('package')
        (self.root/'Cargo.lock').write_text('version=4\n[[package]]\nname="pkg"\nversion="1"\nsource="registry+local"\nchecksum="a"\n')
        metadata={'packages':[{'id':'pkg1','name':'pkg','version':'1','source':'registry+local','manifest_path':str(base/'Cargo.toml')}]}
        before=r.dependencies(metadata,self.root);(base/'new.rs').write_text('changed')
        self.assertEqual(r.changed(before,r.dependencies(metadata,self.root)),['pkg1'])
        self.assertEqual(before['pkg1']['lock_checksum'],'a')

    def test_unlocked_dependency_rejected(self):
        meta={'packages':[{'name':'unlocked','version':'1','id':'x','source':'registry+local','manifest_path':str(self.root/'Cargo.toml')}]}
        with self.assertRaises(ValueError):r.dependencies(meta,self.root)

    def test_external_path_dependency_rejected(self):
        meta={'packages':[{'source':None,'manifest_path':str(self.root/'external/Cargo.toml')}]}
        with self.assertRaises(ValueError):r.dependencies(meta,self.root)

    def test_toolchain_file_link_binds_target_and_link(self):
        base=self.root/'compilerlib';base.mkdir();target=self.root/'objecttool';target.write_text('v1')
        link=base/'rust-objcopy';link.symlink_to(target)
        before=r.toolchain_snapshot(base);self.assertEqual(before['rust-objcopy']['symlink'],str(target))
        target.write_text('v2');self.assertEqual(r.changed(before,r.toolchain_snapshot(base)),['rust-objcopy'])
        target.unlink()
        with self.assertRaises(ValueError):r.toolchain_snapshot(base)

    def test_native_plan_locked_offline_final_allfeatures(self):
        plan=r.native_plan('/cargo',self.root)
        for name,argv in plan:
            if name!='fmt':
                self.assertIn('--locked',argv);self.assertIn('--offline',argv);self.assertIn('--target-dir',argv)
        self.assertEqual(plan[-1][0],'final_artifacts')
        self.assertIn('--all-features',plan[-1][1]);self.assertIn('--message-format=json',plan[-1][1])
        self.assertIn('cli_only_test',[name for name,_ in plan])

    def artifact_output(self):
        records=[]
        for name,kind in [('antimony','bin'),('export_sbml','example'),('format_antimony','example')]:
            path=self.root/name;path.write_text(name);path.chmod(0o700)
            records.append({'reason':'compiler-artifact','target':{'name':name,'kind':[kind]},'executable':str(path),'features':['cli','sbml'],'fresh':True})
        records.append({'reason':'build-finished','success':True})
        return records

    def test_artifacts_from_cargo_not_guessed_paths(self):
        records=self.artifact_output();got=r.artifact_paths('\n'.join(map(json.dumps,records)),self.root)
        self.assertEqual(set(got),{'antimony','export_sbml','format_antimony'})
        self.assertTrue(all(item['cargo_fresh'] for item in got.values()))

    def test_artifact_missing_failed_outside_or_wrong_feature(self):
        original=self.artifact_output()
        variants=[original[:-1], original[1:],copy.deepcopy(original),copy.deepcopy(original)]
        variants[2][0]['features']=['cli'];variants[3][-1]['success']=False
        for records in variants:
            with self.assertRaises(ValueError):r.artifact_paths('\n'.join(map(json.dumps,records)),self.root)
        with self.assertRaises(ValueError):r.artifact_paths('\n'.join(map(json.dumps,original)),self.root/'different')

    def valid_report(self, schema, count=2):
        inventory={};cases=[]
        for i in range(count):
            identity=f'case_{i}';sha='a'*64
            expected={'id':identity,'sha256':sha,'native_profile_expectation':'target_supported',
                      'reference_status':'PASS','selections':['time','S'],'kind':'motif'}
            inventory[identity]=expected
            base={'id':identity,'fixture':identity,'source_sha256':sha,'status':'PASS','reference_status':'PASS'}
            if schema=='antimony-numerical-check-v1':
                base.update(stages=dict.fromkeys(('reference','export','projection','native_simulation',
                    'reference_simulation','native_analytic','reference_analytic','differential','invariants'),'PASS'),
                    invariant_failures=[],comparisons={k:{'status':'PASS','failures':[]} for k in ('native_analytic','reference_analytic','differential')},
                    simulations={k:{'status':'PASS','selections':['time','S'],'column_names':['time','S'],
                                    'timeseries':[[0,1]]*101} for k in ('native','reference')})
                counts={'passed':count,'failed':0,'blocked':0}
            elif schema=='antimony-workload-check-v1':
                base.update(kind='motif',numerical='NOT_RUN',outcome='supported_motif',native_status='EXPORTED',
                    stages=dict.fromkeys(('reference','reference_contract','native_export','native_contract','format',
                        'format_idempotence','formatted_reference','formatted_reference_contract','formatted_native','formatted_native_contract'),'PASS'))
                counts={'motifs':count,'generated_supported':0,'expected_rejections':0,'failed_or_blocked':0,'numerical_not_run':count}
            else:
                base.update(native_process={'status':'EXPORTED'},native_stages={'syntax':'PASS','resolve':'PASS','export':'PASS','numerical':'NOT_RUN'},
                            sbml_inspection_status='PASS',differences=[])
                outcome={'antimony-rs-native-check-v1':'supported_export','antimony-rs-roundtrip-check-v1':'supported_roundtrip','antimony-rs-cli-check-v1':'supported_cli'}[schema]
                base['roundtrip_outcome' if schema=='antimony-rs-roundtrip-check-v1' else 'outcome']=outcome
                key={'antimony-rs-native-check-v1':'supported_exports','antimony-rs-roundtrip-check-v1':'supported_roundtrips','antimony-rs-cli-check-v1':'supported_cli'}[schema]
                counts={key:count,'expected_rejections':0,'failed_or_blocked':0,'numerical_not_run':count}
                if schema=='antimony-rs-roundtrip-check-v1':
                    base.update(original_native_outcome='supported_export',format_status='FORMATTED',idempotence='PASS',
                        formatted_reference_status='PASS',formatted_native_status='PASS',formatted_reference_differences=[],formatted_native_differences=[])
                    counts['byte_idempotent']=count
                if schema=='antimony-rs-cli-check-v1':
                    base.update(numerical='NOT_RUN',byte_idempotent=True,export_status='PASS',formatted_reference_status='PASS',
                        commands={k:{'status':'SUCCESS','report':{'status':'success','exit_code':0}} for k in ('check','format','export')})
            cases.append(base)
        return {'schema':schema,'status':'PASS','counts':counts,'cases':cases},inventory

    def test_checker_zeroexit_falsepass_and_duplicates(self):
        schema='antimony-numerical-check-v1';valid,inventory=self.valid_report(schema)
        self.assertEqual(r.validate_checker(valid,schema,2,inventory)['case_count'],2)
        for update in [{'status':'FAIL'},{'cases':[]},{'schema':'wrong'},
                       {'changed_inputs':['source']},{'manifest_changed_during_check':True},
                       {'counts':{}},{'counts':{'passed':2,'failed':1,'blocked':0}},
                       {'cases':[valid['cases'][0]]*2}]:
            with self.assertRaises(ValueError):r.validate_checker(dict(valid,**update),schema,2,inventory)

    def test_numeric_wrong_ids_and_failed_substages_cannot_pass(self):
        schema='antimony-numerical-check-v1'
        for field,value in [('native_analytic','FAIL'),('reference_analytic','NOT_RUN')]:
            report,inventory=self.valid_report(schema,8);report['cases'][0]['stages'][field]=value
            with self.assertRaises(ValueError):r.validate_checker(report,schema,8,inventory)
        report,inventory=self.valid_report(schema,8)
        for i,case in enumerate(report['cases']):case['id']=case['fixture']=f'wrong_{i}'
        with self.assertRaises(ValueError):r.validate_checker(report,schema,8,inventory)

    def test_every_schema_rejects_missing_substage_or_outcome(self):
        for schema in ['antimony-rs-native-check-v1','antimony-rs-roundtrip-check-v1','antimony-rs-cli-check-v1',
                       'antimony-numerical-check-v1','antimony-workload-check-v1']:
            report,inventory=self.valid_report(schema)
            r.validate_checker(report,schema,2,inventory)
            case=report['cases'][0]
            for key in ['stages','native_stages','commands','roundtrip_outcome','outcome']:case.pop(key,None)
            with self.assertRaises(ValueError):r.validate_checker(report,schema,2,inventory)


    def test_expected_negative_native_and_roundtrip_stages_remain_valid(self):
        for schema in ['antimony-rs-native-check-v1','antimony-rs-roundtrip-check-v1']:
            report,inventory=self.valid_report(schema,1);inventory['case_0']['native_profile_expectation']='strict_profile_rejected'
            case=report['cases'][0];diagnostic={'stage':'resolve','kind':'unsupported','code':'unsupported_feature'}
            case.update(native_process={'status':'REJECTED','diagnostic':diagnostic},
                        native_stages={'syntax':'PASS','resolve':'UNSUPPORTED_PROFILE','export':'NOT_RUN','numerical':'NOT_RUN'})
            if schema=='antimony-rs-native-check-v1':case['outcome']='expected_rejection';key='supported_exports'
            else:
                key='supported_roundtrips';case.update(roundtrip_outcome='expected_rejection',original_native_outcome='expected_rejection',
                    format_status='REJECTED',formatter_process={'diagnostic':diagnostic},idempotence='NOT_RUN',
                    formatted_reference_status='NOT_RUN',formatted_native_status='NOT_RUN')
                report['counts']['byte_idempotent']=0
            report['counts'].update({key:0,'expected_rejections':1})
            r.validate_checker(report,schema,1,inventory)
            case['native_stages']['export']='PASS'
            with self.assertRaises(ValueError):r.validate_checker(report,schema,1,inventory)

    def test_workload_negative_requires_exact_diagnostic_and_notrun(self):
        schema='antimony-workload-check-v1';report,inventory=self.valid_report(schema,1)
        inventory['case_0'].update(kind='generated_negative',recipe={'family':'cycle'})
        case=report['cases'][0];case.update(kind='generated_negative',outcome='expected_rejection',native_status='REJECTED',reference_status='REFERENCE_REJECTED',
            native_diagnostic={'stage':'resolve','code':'initialization_cycle'},formatter_diagnostic={'stage':'resolve','code':'initialization_cycle'})
        case['stages'].update(native_export='EXPECTED_REJECTION',format='EXPECTED_REJECTION',
            **dict.fromkeys(('format_idempotence','formatted_reference','formatted_reference_contract','formatted_native','formatted_native_contract'),'NOT_RUN'))
        report['counts'].update(motifs=0,expected_rejections=1)
        r.validate_checker(report,schema,1,inventory)
        case['formatter_diagnostic']['code']='syntax'
        with self.assertRaises(ValueError):r.validate_checker(report,schema,1,inventory)

    def test_strict_json(self):
        for text in ['{"x":1,"x":2}','{"value":NaN}','not JSON']:
            with self.assertRaises(ValueError):r.strict_json(text)

    def execute(self,script,timeout=2,limit=10000):
        logs=self.root/'logs';logs.mkdir(exist_ok=True)
        return r.command([sys.executable,'-c',script],self.root,dict(os.environ),logs,'test',timeout,limit)

    def test_subprocess_success_logs(self):
        result=self.execute('print("hello")')
        self.assertEqual(result['status'],'PASS');self.assertEqual(result['returncode'],0)
        self.assertEqual(Path(result['logs']['stdout']['path']).read_text(),'hello\n')

    def test_subprocess_nonzero(self):
        result=self.execute('raise SystemExit(3)')
        self.assertEqual(result['status'],'FAIL');self.assertEqual(result['returncode'],3)

    def test_subprocess_crash(self):
        result=self.execute('import os,signal;os.kill(os.getpid(),signal.SIGKILL)')
        self.assertEqual(result['status'],'FAIL');self.assertLess(result['returncode'],0)

    def test_subprocess_timeout(self):
        result=self.execute('import time;time.sleep(5)',timeout=.05)
        self.assertEqual(result['status'],'FAIL');self.assertEqual(result['reason'],'timeout')

    def test_subprocess_output_limit(self):
        result=self.execute('print("x"*100000)',limit=100)
        self.assertEqual(result['status'],'FAIL')
        self.assertEqual(result['logs']['stdout']['bytes'],100)

    def test_missing_command_blocked(self):
        result=r.command(['/missing/program'],self.root,{},self.root,'missing',1)
        self.assertEqual(result['status'],'BLOCKED')

    def fake_receipt(self, mutation=None, bad_checker=False):
        output=self.root/'output';output.mkdir()
        sysroot=self.root/'compiler';(sysroot/'lib').mkdir(parents=True)
        (sysroot/'lib/core.rlib').write_text('compiler library')
        records=self.artifact_output()
        metadata={'packages':[{'source':None,'manifest_path':str(self.root/'Cargo.toml')}]}
        checker_specs={name:(schema,count) for name,_,schema,count in r.checker_plan(
            self.root,{name:{'path':str(self.root/name)} for name in ['antimony','export_sbml','format_antimony']},sys.executable,sys.executable)}

        def fake(argv,cwd,env,logs,name,timeout):
            text='version'
            if name=='rustc_sysroot':text=str(sysroot)
            if name=='cargo_metadata':text=json.dumps(metadata)
            if name=='final_artifacts':text='\n'.join(map(json.dumps,records))
            if name in checker_specs:
                schema,count=checker_specs[name]
                report,_=self.valid_report(schema,count)
                if bad_checker:report['status']='FAIL'
                text=json.dumps(report)
            if mutation and name=='all_test':mutation()
            stdout=logs/(name+'.stdout.log');stdout.write_text(text)
            stderr=logs/(name+'.stderr.log');stderr.write_text('Ran 1 test in 0.001s\nOK\n' if name.endswith('_unit') else '')
            return {'name':name,'status':'PASS','returncode':0,'logs':{'stdout':{'path':str(stdout)},'stderr':{'path':str(stderr)}}}
        with patch.object(r.shutil,'which',return_value=sys.executable),patch.object(r,'command',side_effect=fake),patch.object(r,'config_snapshot',return_value={}),patch.object(r,'expected_inventory',side_effect=lambda root,schema:self.valid_report(schema,next(count for s,count in checker_specs.values() if s==schema))[1]):
            return r.run(self.root,self.root,output,sys.executable,sys.executable)

    def test_aggregate_observed_build_pass(self):
        result=self.fake_receipt()
        self.assertEqual(result['status'],'PASS',result.get('reason'))
        self.assertEqual(result['not_run_commands'],[])
        self.assertEqual(len(result['checker_results']),5)

    def test_aggregate_mutation_invalidates_all_successful_commands(self):
        result=self.fake_receipt(lambda:(self.root/'src/lib.rs').write_text('changed'))
        self.assertEqual(result['status'],'FAIL')
        self.assertEqual(result['changed_source_inputs'],['src/lib.rs'])
        self.assertTrue(all(c['status']=='PASS' for c in result['commands']))

    def test_aggregate_false_checker_pass_rejected(self):
        result=self.fake_receipt(bad_checker=True)
        self.assertEqual(result['status'],'FAIL')
        self.assertIn('roundtrip',result['not_run_commands'])

    def test_checker_paths_use_exact_built_outputs(self):
        artifacts={name:{'path':'/isolated/'+name} for name in ['antimony','export_sbml','format_antimony']}
        plan=r.checker_plan(self.root,artifacts,'/oracle','/numerical')
        self.assertEqual(len(plan),5)
        self.assertEqual([p[3] for p in plan],[90,90,90,8,86])
        for _,argv,_,_ in plan:self.assertIn('/isolated/export_sbml',argv)
        self.assertIn('/isolated/antimony',plan[2][1])


if __name__=='__main__':unittest.main()
