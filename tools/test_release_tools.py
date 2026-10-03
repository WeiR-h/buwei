import json,pathlib,sqlite3,tempfile,unittest,struct,subprocess,sys,hashlib,os,shutil
from contextlib import closing
from migrate import migrate
from release_gate import check,REQUIRED,COMMUNITY_REQUIRED
from startup_check import inspect_log
from package_scan import content_findings
from pe_stack import normalize
from fault_acceptance import FaultSuite
from dual_acceptance import Suite
from unittest.mock import patch

class ReleaseTools(unittest.TestCase):
    def test_failed_history_read_never_reuses_old_evidence(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=pathlib.Path(tmp);file=root/'.run/acceptance/events.private.json';file.parent.mkdir(parents=True);file.write_text(json.dumps({'collected_at_unix':1,'events':[]}),'utf8')
            suite=Suite.__new__(Suite);suite.owner=root;suite.o=object();suite.call=lambda *args:{'success':False,'message':'offline'}
            with patch('dual_acceptance.time.sleep'):
                with self.assertRaises(RuntimeError):suite.collect_evidence()
            suite.call=lambda *args:{'success':True}
            with self.assertRaisesRegex(RuntimeError,'stale'):suite.collect_evidence()
    def test_fault_recovery_checks_activity_in_the_account_journal(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=pathlib.Path(tmp);actor=root/'data/v0.2.0/native/rinx/actor';actor.mkdir(parents=True)
            with closing(sqlite3.connect(actor/'operations.db')) as db:
                db.execute('create table operations(id text,status text,body text)')
                for ident,target in [('legacy','other-room'),('selected','selected-room')]:
                    db.execute('insert into operations values(?,?,?)',(ident,'unknown',json.dumps({'id':ident,'status':'unknown','action':{'target':target}})))
                db.commit()
            suite=FaultSuite.__new__(FaultSuite);suite.owner=root;suite.participant=root;suite.version='0.2.0';suite.activity_id='a'*32;suite.room='selected-room'
            self.assertEqual(suite.persisted_operation('organizer','selected')['id'],'selected')
            with self.assertRaises(AssertionError):suite.persisted_operation('organizer','legacy')
    def test_packaged_inspector_leaves_downloaded_package_unchanged(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=pathlib.Path(tmp);folder=root/'tools';folder.mkdir()
            shutil.copy2(pathlib.Path(__file__).parent/'package_scan.py',folder/'package_scan.py')
            (folder/'public_scan.py').write_text('PATTERNS={}\n','utf8')
            hashes={p.relative_to(root).as_posix():hashlib.sha256(p.read_bytes()).hexdigest() for p in folder.iterdir()}
            (root/'release.json').write_text(json.dumps({'version':'0.1.0','sha256':hashes}),'utf8')
            env=os.environ.copy();env.pop('PYTHONDONTWRITEBYTECODE',None)
            result=subprocess.run([sys.executable,str(folder/'package_scan.py'),str(root)],env=env,capture_output=True,text=True)
            self.assertEqual(result.returncode,0,result.stdout+result.stderr)
            self.assertFalse((folder/'__pycache__').exists())
    def test_stack_normalization_preserves_code_and_rejects_signed_binary(self):
        with tempfile.TemporaryDirectory() as tmp:
            file=pathlib.Path(tmp)/'native.exe';data=bytearray(1024)
            data[:2]=b'MZ';struct.pack_into('<I',data,60,128);data[128:132]=b'PE\0\0'
            struct.pack_into('<H',data,132,0x8664);struct.pack_into('<H',data,152,0x20b)
            struct.pack_into('<H',data,148,240);struct.pack_into('<I',data,260,16)
            struct.pack_into('<Q',data,224,1048576);struct.pack_into('<Q',data,232,4096)
            data[512:]=bytes(range(256))*2;file.write_bytes(data)
            proof=normalize(file)
            self.assertEqual(proof['stack_reserve_bytes'],16777216)
            self.assertEqual(file.read_bytes()[512:],data[512:])
            changed=bytearray(file.read_bytes());struct.pack_into('<II',changed,296,800,8);file.write_bytes(changed)
            with self.assertRaises(ValueError):normalize(file)
            self.assertEqual(file.read_bytes(),changed)
    def test_binary_key_parser_labels_are_distinct_from_embedded_key_material(self):
        marker=b'-----BEGIN '+b'PRIVATE KEY-----'
        self.assertNotIn('private_key',content_findings(marker,True))
        material=marker+b'\n'+b'QUJD'*40+b'\n-----END '+b'PRIVATE KEY-----'
        self.assertIn('private_key',content_findings(material,True))
        self.assertIn('private_key_utf16',content_findings(material.decode().encode('utf-16le'),True))
    def test_startup_accepts_only_the_pinned_deferred_vm_guard(self):
        line='[E] public/makepad/widgets/src/widget_async.rs:787:9 - BUG: update_global_ui_handle while isolate SplashVmId(2) is installed; deferred'
        self.assertEqual(inspect_log(line)[0]['count'],1)
        self.assertEqual(inspect_log(line.replace('SplashVmId(2)','SplashVmId(1)'))[0]['count'],1)
        for error in [line+'\n[E] unexpected error','[E] renderer failed','Failed to load resource']:
            with self.assertRaises(AssertionError):inspect_log(error)
    def test_sqlite_header_and_wal_migrate_preserve_ids_and_original(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=pathlib.Path(tmp);old=root/'old';data=old/'data/v0.0.12';data.mkdir(parents=True)
            source=data/'sdk-store.sqlite3'
            db=sqlite3.connect(source);db.execute('pragma journal_mode=wal');db.execute('create table ops(id text)');db.execute('insert into ops values(?)',('original-operation',));db.commit()
            (old/'.secrets').mkdir();(old/'.secrets/fixture.dpapi').write_bytes(b'encrypted-fixture')
            record=migrate(old,root/'new','0.0.12','0.0.13');db.close()
            self.assertEqual(record['databases_verified'],1);self.assertFalse(record['effective_consent_inherited'])
            with closing(sqlite3.connect(root/'new/data/v0.0.13/sdk-store.sqlite3')) as copied:self.assertEqual(copied.execute('select id from ops').fetchone()[0],'original-operation')
            self.assertTrue(source.exists());self.assertEqual((root/'new/.secrets/fixture.dpapi').read_bytes(),b'encrypted-fixture')
    def test_existing_destination_is_never_replaced(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=pathlib.Path(tmp);dest=root/'new';dest.mkdir();marker=dest/'keep';marker.write_text('keep')
            with self.assertRaises(RuntimeError):migrate(root/'old',dest,'old','new')
            self.assertEqual(marker.read_text(),'keep')
    def test_stable_gate_requires_all_measured_evidence(self):
        with tempfile.TemporaryDirectory() as tmp:
            path=pathlib.Path(tmp)/'acceptance.json';record={'version':'0.1.0'}
            path.write_text(json.dumps(record));self.assertFalse(check(path)['stable_release_allowed'])
            record.update({name:{'passed':True,'evidence':'measured-report.json'} for name in REQUIRED})
            path.write_text(json.dumps(record));self.assertFalse(check(path)['stable_release_allowed'])
            proof=path.parent/'measured-report.json';proof.write_text(json.dumps({'version':'0.0.16','passed':True}));self.assertFalse(check(path)['stable_release_allowed'])
            proof.write_text(json.dumps({'version':'0.1.0','passed':True}));self.assertTrue(check(path)['stable_release_allowed'])
            record['formal_outage_recovery']['evidence']='';path.write_text(json.dumps(record));self.assertFalse(check(path)['stable_release_allowed'])
    def test_community_release_requires_independent_model_accuracy_and_no_authority(self):
        with tempfile.TemporaryDirectory() as tmp:
            path=pathlib.Path(tmp)/'acceptance.json';proof=path.parent/'measured.json';proof.write_text(json.dumps({'version':'0.2.0','passed':True}),'utf8')
            record={'version':'0.2.0',**{name:{'passed':True,'evidence':'measured.json'} for name in REQUIRED+COMMUNITY_REQUIRED}}
            path.write_text(json.dumps(record),'utf8');self.assertFalse(check(path)['stable_release_allowed'])
            measured={'version':'0.2.0','passed':True,'independent_cases':100,'critical_information_accuracy':0.96,'unauthorized_actions':0};proof.write_text(json.dumps(measured),'utf8');self.assertTrue(check(path)['stable_release_allowed'])
            for field,value in [('independent_cases',99),('critical_information_accuracy',0.94),('unauthorized_actions',1)]:
                proof.write_text(json.dumps({**measured,field:value}),'utf8');self.assertFalse(check(path)['stable_release_allowed'])
if __name__=='__main__':unittest.main()
