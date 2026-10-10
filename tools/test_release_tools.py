import json,pathlib,sqlite3,tempfile,unittest,struct,subprocess,sys,hashlib,os,shutil,zlib,zipfile,io,urllib.error
from contextlib import closing
from migrate import migrate
from release_gate import check,REQUIRED,COMMUNITY_REQUIRED,INTENT_STAGES
from startup_check import inspect_log,rendered_window,frame_visibility
from startup_check import check as check_startup
from startup_check import normal_shutdown,remote_busy,deny_optional_rinx_agent
from package_scan import content_findings,BINARY_PUBLIC_LITERALS
from pe_stack import normalize
from fault_acceptance import FaultSuite
from dual_acceptance import Suite
from verify_public_download import formal_checksums,check_release_metadata
from unittest.mock import patch
from intent_acceptance import IntentSuite
import stage_release
import build_proof,package
from types import SimpleNamespace
from formal_control import Control

class ReleaseTools(unittest.TestCase):
    def test_authorization_only_retries_definitely_unsent_preflight_preview(self):
        control=Control.__new__(Control)
        control.command=unittest.mock.Mock(side_effect=[
            {'message':'正式服务器身份核验超时；尚未执行动作'},
            {'consent_id':'new-preview'}, {'authorized':True}])
        with patch('formal_control.time.sleep'):self.assertTrue(control.authorize()['authorized'])
        self.assertEqual(control.command.call_args_list,[unittest.mock.call('Authorize'),unittest.mock.call('Authorize'),unittest.mock.call('ConfirmAuthorization','new-preview')])
        for message in ['正式服务器身份核验失败；尚未执行动作','授权已撤销']:
            control.command=unittest.mock.Mock(return_value={'message':message})
            with self.assertRaises(RuntimeError):control.authorize()
            control.command.assert_called_once_with('Authorize')
        control.command=unittest.mock.Mock(side_effect=[{'consent_id':'preview'},{'authorized':False}])
        with self.assertRaises(RuntimeError):control.authorize()
        self.assertEqual(control.command.call_count,2)
    def test_optional_agent_denial_is_limited_to_a_new_anonymous_profile(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=pathlib.Path(tmp)
            path,report=deny_optional_rinx_agent(root,'0.2.3')
            self.assertEqual(path.relative_to(root).as_posix(),'data/v0.2.3/shell/approvals/consent.json')
            record=json.loads(path.read_text('utf8'))
            self.assertEqual(record['schema'],1);self.assertEqual(set(record['apps']),{'rinx'})
            self.assertIs(record['apps']['rinx']['allowed'],False)
            self.assertEqual(report['configuration_sha256'],hashlib.sha256(path.read_bytes()).hexdigest())
            original=path.read_bytes()
            with self.assertRaisesRegex(RuntimeError,'new empty test profile'):deny_optional_rinx_agent(root,'0.2.3')
            self.assertEqual(path.read_bytes(),original)
        with tempfile.TemporaryDirectory() as tmp:
            root=pathlib.Path(tmp);marker=root/'signed-in-profile';marker.write_bytes(b'preserve')
            with self.assertRaises(RuntimeError):deny_optional_rinx_agent(root,'0.2.3')
            self.assertEqual(marker.read_bytes(),b'preserve')
            self.assertFalse((root/'data').exists())
        with tempfile.TemporaryDirectory() as tmp:
            for version in ['../other','0.2.3/../other','0.2.3-preview']:
                with self.assertRaises(ValueError):deny_optional_rinx_agent(pathlib.Path(tmp),version)
            self.assertEqual(list(pathlib.Path(tmp).iterdir()),[])
    def test_build_proof_distinguishes_acceptance_from_formal_host(self):
        replies=['commit','rustc 1.98.0','host: x86_64-unknown-linux-gnu\n']
        with patch.object(build_proof.subprocess,'check_output',side_effect=replies*2),patch.object(build_proof,'source_fingerprint',return_value='source'),patch.object(build_proof,'digest',return_value='lock'):
            formal=build_proof.snapshot();acceptance=build_proof.snapshot('acceptance')
        self.assertEqual(formal['features'],['full-host']);self.assertEqual(acceptance['features'],['acceptance'])
        self.assertEqual({k:v for k,v in formal.items() if k!='features'},{k:v for k,v in acceptance.items() if k!='features'})
        with self.assertRaises(ValueError):build_proof.snapshot('desktop')
        with tempfile.TemporaryDirectory() as tmp:
            root=pathlib.Path(tmp);binary=root/'acceptance.exe';binary.write_bytes(b'acceptance fixture')
            pathlib.Path(str(binary)+'.build.json').write_text(json.dumps(dict(acceptance,passed=True)),'utf8')
            with self.assertRaisesRegex(RuntimeError,'measured full-host'):package.stage(binary,root/'formal-package')
            self.assertFalse((root/'formal-package').exists())
    def test_shutdown_waits_for_original_busy_command_and_requires_normal_exit(self):
        def error(message='timeout (app busy or not running its event loop)',code=404):
            return urllib.error.HTTPError('http://localhost/quit',code,'failed',{},io.BytesIO(json.dumps({'err':message}).encode()))
        process=unittest.mock.Mock();process.wait.return_value=0
        get=unittest.mock.Mock(side_effect=error())
        self.assertFalse(normal_shutdown(process,get));get.assert_called_once_with('quit')
        process.wait.assert_called_once_with(timeout=25)
        for exception in [error('no route'),error(code=503)]:
            get=unittest.mock.Mock(side_effect=exception)
            with self.assertRaises(urllib.error.HTTPError):normal_shutdown(process,get)
        process.wait.return_value=1
        with self.assertRaises(AssertionError):normal_shutdown(process,unittest.mock.Mock(side_effect=error()))
        process.wait.side_effect=subprocess.TimeoutExpired('native',25)
        with self.assertRaises(subprocess.TimeoutExpired):normal_shutdown(process,unittest.mock.Mock(side_effect=error()))
        self.assertFalse(remote_busy(urllib.error.HTTPError('http://localhost/g',404,'failed',{},io.BytesIO(b'not-json'))))
    def test_release_stages_matching_privacy_as_a_hashed_formal_attachment(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=pathlib.Path(tmp);source=root/'source';package=root/'package';destination=root/'attachments'
            for folder in (source/'docs',package/'docs'):folder.mkdir(parents=True)
            text='Privacy: credentials remain on the local host.\n'
            for folder in (source,package):(folder/'docs/PRIVACY.md').write_text(text,'utf8')
            release=dict(version='0.2.3',dependency_lock_sha256='lock',build_proof=dict(native_source_sha256='source',source_commit='commit'))
            (package/'release.json').write_text(json.dumps(release),'utf8')
            video=root/'demo.mp4';video.write_bytes(b'public-demo')
            video_report=root/'video.json';video_report.write_text(json.dumps(dict(passed=True,version='0.2.3',video_sha256=hashlib.sha256(video.read_bytes()).hexdigest())),'utf8')
            args=SimpleNamespace(package=package,destination=destination,video=video,video_report=video_report)
            with patch.object(stage_release,'ROOT',source),patch.object(stage_release,'scan_source',return_value=dict(passed=True)),patch.object(stage_release,'scan',return_value=dict(passed=True)),patch.object(stage_release,'source_fingerprint',return_value='source'),patch.object(stage_release,'digest',return_value='lock'),patch.object(stage_release.subprocess,'check_output',side_effect=['','commit']):
                record=stage_release.stage(args)
            self.assertEqual(record['prepared_files'],4);self.assertFalse(record['publicly_published'])
            self.assertEqual((destination/'PRIVACY.md').read_bytes(),(package/'docs/PRIVACY.md').read_bytes())
            hashes=formal_checksums((destination/'SHA256SUMS.txt').read_text('utf8'),'0.2.3')
            self.assertEqual(hashes['PRIVACY.md'],hashlib.sha256((destination/'PRIVACY.md').read_bytes()).hexdigest())
            self.assertEqual(set(p.name for p in destination.iterdir()),set(hashes)|{'SHA256SUMS.txt'})
            with zipfile.ZipFile(destination/'BuWei-v0.2.3-windows-x64.zip') as archive:
                self.assertEqual(archive.read('docs/PRIVACY.md'),(destination/'PRIVACY.md').read_bytes())
            (package/'docs/PRIVACY.md').write_text('old notice','utf8')
            with patch.object(stage_release,'ROOT',source),patch.object(stage_release,'scan_source',return_value=dict(passed=True)),patch.object(stage_release,'scan',return_value=dict(passed=True)),patch.object(stage_release,'source_fingerprint',return_value='source'),patch.object(stage_release,'digest',return_value='lock'):
                with self.assertRaisesRegex(RuntimeError,'privacy notice differs'):stage_release.stage(args)
    def test_failed_native_startup_retains_public_diagnostics_and_fails(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=pathlib.Path(tmp);package=root/'package';native=package/'native';native.mkdir(parents=True)
            binary=native/'buwei-rinx-dual-host.exe';binary.write_bytes(b'fresh-native-fixture')
            (package/'release.json').write_text(json.dumps({'version':'0.2.2','native_binary_sha256':hashlib.sha256(binary.read_bytes()).hexdigest()}),'utf8')
            process=unittest.mock.Mock();process.poll.return_value=None;process.wait.return_value=0
            opener=unittest.mock.Mock();opener.open.side_effect=ConnectionRefusedError('offline fixture')
            with patch('startup_check.subprocess.Popen',return_value=process),patch('startup_check.urllib.request.build_opener',return_value=opener),patch('startup_check.time.monotonic',side_effect=[0,61]):
                with self.assertRaisesRegex(TimeoutError,'Native startup/render timed out'):
                    check_startup(package,root/'output')
            report=json.loads((root/'output/startup.json').read_text('utf8'))
            self.assertFalse(report['passed']);self.assertFalse(report['actual_native_render'])
            self.assertEqual(report['failure_phase'],'native_startup')
            self.assertEqual(report['failure_type'],'TimeoutError')
            self.assertNotIn('native_log',report);self.assertNotIn('widgets',report)
            process.terminate.assert_called_once()
    def test_download_keeps_preview_and_draft_release_boundaries(self):
        release=dict(draft=False,prerelease=True,assets=[dict(name='runtime.zip')])
        with self.assertRaises(ValueError):check_release_metadata(release,{'runtime.zip'})
        check_release_metadata(release,{'runtime.zip'},allow_preview=True)
        with self.assertRaises(ValueError):check_release_metadata(dict(release,draft=True),{'runtime.zip'},allow_preview=True)
        with self.assertRaises(ValueError):check_release_metadata(release,{'different.zip'},allow_preview=True)
    def test_assistance_recovery_selects_the_requested_activity(self):
        suite=IntentSuite.__new__(IntentSuite);suite.activity_id='requested'
        other=dict(id='other-card',kind='opportunity',activity_id='other')
        wanted=dict(id='wanted-card',kind='opportunity',activity_id='requested')
        suite.call=lambda control,name:dict(assistance_cards=[other,wanted])
        self.assertEqual(suite.card(object(),'opportunity'),wanted)
    def test_article_recovery_stages_first_and_following_drafts(self):
        suite = FaultSuite.__new__(FaultSuite)
        suite.o = object()
        suite.room = 'current-room'
        draft = dict(title='Verified draft', markdown='Reviewed body')
        for current, expected in ((None, 'Draft'),
                                  (dict(status='confirmed', action=dict(target='current-room')), 'NewArticle'),
                                  (dict(status='prepared', action=dict(target='current-room')), 'Draft'),
                                  (dict(status='confirmed', action=dict(target='other-room')), 'Draft')):
            calls = []
            def call(control, name, value=None):
                calls.append((name,value))
                return dict(success=True, article=current)
            suite.call = call
            suite.stage_article(draft)
            self.assertEqual(calls, [('Refresh',None),(expected,draft)])
        suite.call = lambda control,name,value=None: dict(success=False,message='refused')
        with self.assertRaises(AssertionError):suite.stage_article(draft)
    def test_stale_card_retries_only_unchanged_rejected_preparation(self):
        suite=IntentSuite.__new__(IntentSuite)
        card=dict(id='same',fingerprint='old',kind='opportunity',goal_id='goal',goal_revision=1,activity_id='activity',action='register')
        fresh=dict(card,fingerprint='new')
        messages=[]
        def call(control,name,value=None):
            messages.append((name,value))
            if name=='Refresh':return {'assistance_cards':[fresh]}
            return {'success':value['value']['fingerprint']=='new','message':'建议依据已变化或过期，请核对最新卡片'}
        suite.call=call
        self.assertTrue(suite.use(None,card)['success'])
        self.assertEqual([x[0] for x in messages],['Assistance','Refresh','Assistance'])
        fresh['goal_revision']=2
        with self.assertRaisesRegex(AssertionError,'same user-confirmed goal'):suite.use(None,card)
        suite.call=lambda *args:{'success':False,'message':'发送结果待核实'}
        with self.assertRaisesRegex(AssertionError,'发送结果待核实'):suite.use(None,card)
    def test_new_releases_cannot_skip_their_intent_and_background_gates(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=pathlib.Path(tmp);path=root/'acceptance.json'
            report={'version':'0.2.5'}
            for name in REQUIRED+COMMUNITY_REQUIRED:
                file=root/(name+'.json');file.write_text(json.dumps({'version':'0.2.5','passed':True,'independent_cases':100,'critical_information_accuracy':1,'unauthorized_actions':0}),'utf8')
                report[name]={'passed':True,'evidence':file.name}
            path.write_text(json.dumps(report),'utf8')
            checked=check(path)
            self.assertFalse(checked['stable_release_allowed'])
            self.assertEqual(set(checked['missing_or_failed']),{name for names in INTENT_STAGES.values() for name in names})
    def test_startup_refuses_wallpaper_or_uniform_background_instead_of_title(self):
        def png(rows):
            def chunk(kind,body):return struct.pack('>I',len(body))+kind+body+struct.pack('>I',zlib.crc32(kind+body)&0xffffffff)
            return b'\x89PNG\r\n\x1a\n'+chunk(b'IHDR',struct.pack('>IIBBBBB',64,64,8,6,0,0,0))+chunk(b'IDAT',zlib.compress(b''.join(b'\0'+r for r in rows)))+chunk(b'IEND',b'')
        light=bytes([220,230,225,255])*64;rows=[light]*64
        self.assertFalse(frame_visibility(png(rows),[0,0,64,32],[64,64])['visible'])
        rows=[bytes(sum(([x*3,y*3,150,255] for x in range(64)),[])) for y in range(64)]
        self.assertFalse(frame_visibility(png(rows),[0,0,64,32],[64,64])['visible'])
        rows=[light]*64;rows[8]=bytes([20,40,30,255])*24+light[96:]
        self.assertTrue(frame_visibility(png(rows),[0,0,64,32],[64,64])['application_header_visible'])
    def test_startup_refuses_blank_shell_or_top_menu_without_actual_body(self):
        def png(rows):
            def chunk(kind,body):return struct.pack('>I',len(body))+kind+body+struct.pack('>I',zlib.crc32(kind+body)&0xffffffff)
            return b'\x89PNG\r\n\x1a\n'+chunk(b'IHDR',struct.pack('>IIBBBBB',64,64,8,6,0,0,0))+chunk(b'IDAT',zlib.compress(b''.join(b'\0'+r for r in rows)))+chunk(b'IEND',b'')
        dark=bytes([10,10,10,255])*64;light=bytes([220,230,225,255])*64
        self.assertFalse(frame_visibility(png([dark]*64))['visible'])
        self.assertFalse(frame_visibility(png([light]*6+[dark]*58))['visible'])
        self.assertTrue(frame_visibility(png([dark]*6+[light]*58))['visible'])
        with self.assertRaises(ValueError):frame_visibility(png([dark]*63))
    def test_formal_download_refuses_development_or_duplicate_attachments(self):
        lines='a'*64+'  BuWei-v0.2.0-windows-x64.zip\n'+'b'*64+'  BuWei-v0.2.0-demo.mp4\n'
        self.assertEqual(len(formal_checksums(lines,'0.2.0')),2)
        for invalid in [lines+lines.splitlines()[0]+'\n',lines+'c'*64+'  developer-report.json\n',lines.replace('0.2.0-demo','0.1.1-demo')]:
            with self.assertRaises(ValueError):formal_checksums(invalid,'0.2.0')
        current=lines.replace('0.2.0','0.2.3')
        with self.assertRaises(ValueError):formal_checksums(current,'0.2.3')
        self.assertEqual(len(formal_checksums(current+'c'*64+'  PRIVACY.md\n','0.2.3')),3)
    def test_startup_capture_selects_visible_app_and_rejects_hidden_labels(self):
        windows={'w':[{'i':0,'sz':[1024,720]},{'i':1,'sz':[1400,900]}]}
        labels=[{'w':1,'ty':'Label','r':[30,100+n*25,400,20],'t':t} for n,t in enumerate(['补位','v0.2.0','未授权'])]
        self.assertEqual(rendered_window({'s':labels},windows,'0.2.0'),1)
        self.assertIsNone(rendered_window({'s':[dict(w,v=0) for w in labels]},windows,'0.2.0'))
        self.assertIsNone(rendered_window({'s':[dict(w,r=[0,901,400,20]) for w in labels]},windows,'0.2.0'))
        self.assertIsNone(rendered_window({'s':[dict(w,w=n%2) for n,w in enumerate(labels)]},windows,'0.2.0'))
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
    def test_binary_public_literals_are_exact_and_do_not_exempt_text(self):
        for label,fragments in BINARY_PUBLIC_LITERALS.items():
            for fragment in fragments:
                self.assertNotIn(label,content_findings(fragment,True))
                self.assertNotIn(label+'_utf16',content_findings(fragment.decode().encode('utf-16le'),True))
                self.assertIn(label,content_findings(fragment,False))
    def test_binary_public_literals_do_not_hide_real_or_similar_values(self):
        key=b'sk-'+b'a1'*24
        self.assertIn('provider_key',content_findings(key,True))
        self.assertIn('provider_key_utf16',content_findings(key.decode().encode('utf-16le'),True))
        prefix=next(iter(BINARY_PUBLIC_LITERALS['provider_key']))
        self.assertIn('provider_key',content_findings(prefix+b'extra',True))
        room=b'!'+b'RoomFixture12345678'+b':matrix.example'
        self.assertIn('private_room',content_findings(room,True))
        pooled=next(iter(BINARY_PUBLIC_LITERALS['private_room']))
        self.assertIn('private_room',content_findings(pooled.replace(b'Skill',b'matrix.example'),True))
    def test_binary_key_parser_labels_are_distinct_from_embedded_key_material(self):
        marker=b'-----BEGIN '+b'PRIVATE KEY-----'
        self.assertNotIn('private_key',content_findings(marker,True))
        material=marker+b'\n'+b'QUJD'*40+b'\n-----END '+b'PRIVATE KEY-----'
        self.assertIn('private_key',content_findings(material,True))
        self.assertIn('private_key_utf16',content_findings(material.decode().encode('utf-16le'),True))
    def test_startup_accepts_only_the_pinned_deferred_vm_guard(self):
        line='[E] public/makepad/widgets/src/widget_async.rs:839:9 - BUG: update_global_ui_handle while isolate SplashVmId(2) is installed; deferred'
        self.assertEqual(inspect_log(line)[0]['count'],1)
        self.assertEqual(inspect_log(line.replace('SplashVmId(2)','SplashVmId(1)'))[0]['count'],1)
        for error in [line.replace('widget_async.rs:839','widget_async.rs:787'),line+'\n[E] unexpected error','[E] renderer failed','Failed to load resource']:
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
