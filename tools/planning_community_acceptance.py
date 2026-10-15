"""Opt-in actual SDK test of planning, community entry and organizer exports.

Only the two already verified test identities are used. Traces, CSVs and raw
events stay in the supplied private directory. A controlled run is not a pilot.
"""
import argparse,csv,json,pathlib,time,hashlib,sqlite3
from fault_acceptance import Hosts
from formal_control import Control
from intent_acceptance import display,selected

class NextStage:
    def __init__(self,args):
        self.args=args;self.out=args.private_directory;self.out.mkdir(parents=True,exist_ok=True)
        self.h=Hosts(args.executable,args.owner_profile,args.participant_profile,args.version,8290,8291)
        self.trace=json.loads((self.out/'trace.private.json').read_text('utf8')) if (self.out/'trace.private.json').is_file() else [];self.rounds=[];self.progress={}
    def call(self,c,name,value=None,ok=True,timeout=90):
        started=time.perf_counter()
        try:r=c.command(name,value,timeout=timeout)
        except Exception:
            self.trace.append(dict(command=name,collected_at_unix=int(time.time()),elapsed_ms=(time.perf_counter()-started)*1000,outcome='unknown',run_id=getattr(self,'run_id',None)))
            (self.out/'trace.private.json').write_text(json.dumps(self.trace,ensure_ascii=False),'utf8')
            raise
        self.trace.append(dict(command=name,collected_at_unix=int(time.time()),elapsed_ms=(time.perf_counter()-started)*1000,outcome='success' if r.get('success') is True else 'refused',run_id=getattr(self,'run_id',None),result=r))
        (self.out/'trace.private.json').write_text(json.dumps(self.trace,ensure_ascii=False),'utf8')
        if ok and r.get('success') is not True:raise AssertionError(name+': '+r.get('message',''))
        return r
    def d(self,c,name,value=None,**kw):
        a={'command':name}
        if value is not None:a['value']=value
        return self.call(c,'Discovery',a,**kw)
    def p(self,c,name,value=None,**kw):
        a={'command':name}
        if value is not None:a['value']=value
        return self.call(c,'Planning',a,**kw)
    def bind(self,c,room):c.config['room']=room;c.write('config.local.json',c.config)
    def select(self,c,identifier):
        r=self.call(c,'Refresh');index=next(i for i,a in enumerate(r.get('activities') or []) if a['metadata']['activity_id']==identifier)
        r=self.call(c,'SelectActivity',index);self.bind(c,r['activity']['room']);return r['activity']
    def wait(self,c,condition,seconds=100):
        end=time.monotonic()+seconds
        while True:
            r=self.call(c,'Refresh')
            if condition(r):return r
            if time.monotonic()>end:raise TimeoutError('Verified state did not arrive')
            time.sleep(1)
    def member(self,a):return next((p for p in a['people'] if p['account']==self.accounts[1]),{})
    def form(self,template,title):
        start=int(time.time())//86400*86400+2*86400+11*3600+30*60
        return dict(title=title,template=template,capacity=2,start=display(start),end=display(start+7200),location='Rinx 线上测试社群',description='这是经授权的双账号功能验收，不代表实际到场。')
    def publish(self,identifier,withdrawn=False):
        self.select(self.o,identifier)
        r=self.d(self.o,'PreviewPublish',dict(community=self.group,withdrawn=withdrawn));op=r['community']['operation'];assert op['status'] in ('prepared','confirmed')
        r=self.d(self.o,'ConfirmPublish',op['id']);assert r['community']['operation']['id']==op['id']
        return r['community']['operation']
    def access(self,identifier):
        r=self.d(self.o,'PreviewAccess',self.group+'|'+identifier)
        return self.d(self.o,'ConfirmAccess',r['community']['access_nonce'])
    def entry(self,identifier):
        self.d(self.m,'Refresh');r=self.d(self.m,'PreviewEntry',self.group+'|'+identifier)
        op=r['community']['operation'];r=self.d(self.m,'ConfirmEntry',op['id']);assert r['community']['operation']['status']=='confirmed'
        self.d(self.o,'Refresh');r=self.call(self.m,'JoinCard');self.bind(self.m,r['activity']['room']);return op
    def proof(self,operations):
        self.call(self.o,'CollectEvidence');events=json.loads((self.o.directory/'events.private.json').read_text('utf8'))
        checked=[]
        for op in operations:
            def identifier(e):
                content=e.get('content',{});return content.get('action_id') or content.get('operation_id') or content.get('org.buwei.action',{}).get('operation_id')
            matches=[e for e in events['events'] if identifier(e)==op['id']]
            assert len(matches)==1 and matches[0]['sender']==op['account']
            checked.append(dict(operation_sha256=hashlib.sha256(op['id'].encode()).hexdigest(),service_event_count=1,server_time_ms=matches[0]['origin_server_ts'],locally_collected_at_unix=events['collected_at_unix']))
        return checked
    def run(self):
        for role in ('organizer','participant'):self.h.start(role)
        accounts=[]
        for role in ('organizer','participant'):
            s=json.loads((self.h.profiles[role]/'data'/('v'+self.args.version)/'rinx-binding-status.json').read_text('utf8'));accounts.append(s['account'])
        assert accounts[0]!=accounts[1];self.accounts=accounts
        self.o=Control(self.args.owner_profile,accounts[0],(selected(self.args.owner_profile,self.args.version) or {}).get('room',''));self.m=Control(self.args.participant_profile,accounts[1],(selected(self.args.participant_profile,self.args.version) or {}).get('room',''))
        self.o.pause_automatic_sync(True);self.m.pause_automatic_sync(True);self.o.authorize();self.m.authorize()
        setup=self.o.directory/'community-setup.local.json'
        if setup.is_file():
            group=json.loads(setup.read_text('utf8'));self.group=group.get('room')
            if not self.group or group.get('owner')!=accounts[0]:raise RuntimeError('Original test community requires reconciliation; not recreated')
        else:
            group=self.call(self.o,'TestCommunity',dict(title='补位新流程 · 双账号测试社群',member=accounts[1]));self.group=group['community']['rooms'][-1][0]
        self.call(self.m,'TestJoinContact',self.group)
        self.d(self.o,'Watch',self.group);self.d(self.m,'Watch',self.group)
        existing=self.call(self.o,'Refresh').get('activities') or []
        old_reading=next((a for a in existing if a['title']=='读书会 · 自然语言策划验收'),None)
        old_boardgame=next((a for a in existing if a['title']=='桌游 · 社群列表对照'),None)
        if old_reading and old_boardgame:
            self.reading=old_reading['metadata']['activity_id'];self.boardgame=old_boardgame['metadata']['activity_id']
            if self.args.model:
                assert any(t['command']=='Planning' and t['result'].get('success') and t['result'].get('planning',{}).get('draft',{}).get('turns') for t in self.trace)
                self.progress['single_interactive_model_call']=dict(passed=True,model='k3',matrix_actions_performed=0,reused_original_verified_plan=True)
        else:
            form=self.form('reading','读书会 · 自然语言策划验收')
            text=f"举办读书会，活动名称为{form['title']}，开始{form['start']}，结束{form['end']}，地点是{form['location']}，提供2个参与名额。请准备流程和材料建议。"
            if self.args.model:
                r=self.p(self.o,'Prepare',dict(text=text,id=None,revision=None));draft=r['planning']['draft']
                assert draft['basics']['start']==form['start'] and draft['basics']['end']==form['end'] and draft['basics']['capacity']==2
                assert not r.get('activity') and not r.get('activities')
                self.progress['single_interactive_model_call']=dict(passed=True,model='k3',matrix_actions_performed=0)
            else:
                updates=[dict(field=k,value=str(v),source=text) for k,v in form.items() if k!='description']
                r=self.p(self.o,'Proposal',dict(text=text,id=None,revision=None,proposal=dict(updates=updates,agenda=['交流读书笔记'],preparation=['本人确认所读章节'],recruitment='欢迎参加读书会',reminders=['提前一天确认人数']),call_id=None));draft=r['planning']['draft']
            r=self.p(self.o,'Save',dict(id=draft['id'],revision=draft['revision'],basics=form));draft=r['planning']['draft']
            r=self.p(self.o,'PreviewCreate');nonce=r['planning']['creation_nonce']
            stale=self.p(self.o,'ConfirmCreate',dict(nonce=nonce,basics=dict(form,capacity=3)),ok=False);assert not stale['success'] and not stale.get('activity')
            r=self.p(self.o,'ConfirmCreate',dict(nonce=nonce,basics=form));self.bind(self.o,r['activity']['room']);self.reading=r['activity']['metadata']['activity_id']
            assert r['planning']['draft']['activity_id']==self.reading
            repeat=self.p(self.o,'ConfirmCreate',dict(nonce=nonce,basics=form),ok=False);assert not repeat['success'] and len(repeat['activities'])==1
            r=self.call(self.o,'CreateDated',self.form('boardgame','桌游 · 社群列表对照'));self.bind(self.o,r['activity']['room']);self.boardgame=r['activity']['metadata']['activity_id']
        self.publish(self.reading);self.publish(self.boardgame)
        r=self.d(self.m,'Refresh');ids={l['announcement']['activity_id'] for l in r['community']['listings']};assert {self.reading,self.boardgame}<=ids
        r=self.d(self.m,'Filter',dict(template='reading',date='',available_only=False));assert len(r['community']['listings'])==1
        self.d(self.m,'Filter',dict(template='',date='',available_only=False));self.access(self.reading)
        if (selected(self.args.participant_profile,self.args.version) or {}).get('metadata',{}).get('activity_id')!=self.reading:self.entry(self.reading)
        self.select(self.o,self.reading);self.select(self.m,self.reading)
        self.call(self.o,'SyncActivity');fresh=self.call(self.m,'Refresh')
        if self.member(fresh['activity']).get('status')=='confirmed':
            prefs=dict(earliest=fresh['activity']['start'],latest=fresh['activity']['end'],group=2);self.call(self.m,'Cancel');self.call(self.m,'ConfirmParticipant',prefs);self.call(self.o,'SyncActivity');self.wait(self.m,lambda r:self.member(r['activity']).get('status')=='cancelled')
        self.o.pause_automatic_sync(False);self.m.pause_automatic_sync(False)
        settings=dict(invitation_minutes=5,quiet_start=0,quiet_end=24,max_invitations=30)
        r=self.call(self.o,'PreviewAutomation',settings);self.call(self.o,'ConfirmAutomation',dict(id=r['policy_consent_id'],settings=settings))
        for n in range(5):
            a=self.call(self.o,'Refresh')['activity'];prefs=dict(earliest=a['start'],latest=a['end'],group=2)
            r=self.call(self.m,'Join',prefs);join=r['participant'];r=self.call(self.m,'ConfirmParticipant',prefs);assert r['participant']['id']==join['id'] and r['participant']['status']=='confirmed'
            self.call(self.o,'SyncActivity')
            delivered=self.wait(self.m,lambda r:any(i['recipient']==accounts[1] and i['delivery']=='delivered' and i['reply']=='pending' for i in r['activity']['invitations']))
            invitation=next(i for i in reversed(delivered['activity']['invitations']) if i['recipient']==accounts[1] and i['reply']=='pending')
            r=self.call(self.m,'Accept',True);reply=r['participant'];assert reply['id']!=invitation['operation_id']
            r=self.call(self.m,'ConfirmParticipant',prefs);assert r['participant']['status']=='confirmed'
            self.call(self.o,'SyncActivity');confirmed=self.wait(self.o,lambda r:self.member(r['activity']).get('status')=='confirmed')['activity'];assert confirmed['capacity']==2 and self.member(confirmed)['preferences']['group']==2
            self.wait(self.m,lambda r:self.member(r['activity']).get('status')=='confirmed')
            if n==0:
                path=(self.out/'roster.csv').resolve();r=self.call(self.o,'PreviewRosterExport',str(path));self.call(self.o,'ConfirmRosterExport',r['roster_export']['nonce'])
                rows=list(csv.DictReader(path.open(encoding='utf-8-sig',newline='')));assert len(rows)==1 and rows[0]['同行人数']=='2' and rows[0]['报名状态']=='报名已确认' and rows[0]['Matrix身份'].lstrip("'")==accounts[1]
                denied=self.call(self.m,'PreviewRosterExport',str(self.out/'forbidden.csv'),ok=False);assert not denied['success'] and not (self.out/'forbidden.csv').exists()
                self.progress['roster']=dict(passed=True,owner_only=True,matches_verified_snapshot=True,formula_safe=True,member_visibility_private=True)
            r=self.call(self.m,'Cancel');cancel=r['participant'];self.call(self.m,'ConfirmParticipant',prefs);self.call(self.o,'SyncActivity');self.wait(self.o,lambda r:self.member(r['activity']).get('status')=='cancelled');self.wait(self.m,lambda r:self.member(r['activity']).get('status')=='cancelled')
            # Read the authoritative journal's original invitation for evidence.
            native=self.args.owner_profile/'data'/('v'+self.args.version)/'native/rinx'
            db=next(native.glob('*/operations.db'))
            with sqlite3.connect('file:'+db.as_posix()+'?mode=ro',uri=True) as connection:invite=json.loads(connection.execute('SELECT body FROM operations WHERE id=?',(invitation['operation_id'],)).fetchone()[0])
            checked=self.proof([join,invite,reply,cancel])
            self.rounds.append(dict(registration_confirmed=True,group_kept_whole=True,service_event_counts_verified=True,events=checked))
            (self.out/'rounds.local.json').write_text(json.dumps(self.rounds,indent=2),'utf8');print(f'Actual community round {n+1} passed',flush=True)
        self.progress.update(passed=True,version=self.args.version,distinct_identities=2,real_rounds=self.rounds,controlled_test=True,actual_user_trial=False,cross_machine_trial_completed=False,actual_system_chat_call=False)
        (self.out/'acceptance.local.json').write_text(json.dumps(self.progress,indent=2),'utf8')
        return self.progress
    def close(self):
        for role in tuple(self.h.processes):self.h.stop(role)

if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('--executable',type=pathlib.Path,required=True);p.add_argument('--owner-profile',type=pathlib.Path,required=True);p.add_argument('--participant-profile',type=pathlib.Path,required=True);p.add_argument('--version',required=True);p.add_argument('--private-directory',type=pathlib.Path,required=True);p.add_argument('--model',action='store_true');a=p.parse_args();s=NextStage(a)
    try:s.run()
    finally:s.close()
