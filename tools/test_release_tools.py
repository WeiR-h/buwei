import json,pathlib,sqlite3,tempfile,unittest,struct
from contextlib import closing
from migrate import migrate
from release_gate import check,REQUIRED
from startup_check import inspect_log
from package_scan import content_findings
from pe_stack import normalize

class ReleaseTools(unittest.TestCase):
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
if __name__=='__main__':unittest.main()
