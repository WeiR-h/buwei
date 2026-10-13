"""Actual two-identity community receipt recovery; no model calls."""
import argparse,json,pathlib,sqlite3,time,hashlib
from planning_community_acceptance import NextStage
from formal_control import Control
from intent_acceptance import selected

class Recovery(NextStage):
    def op(self,identifier):
        folder=self.args.owner_profile/'data'/('v'+self.args.version)/'native/rinx'
        database=next(folder.glob('*/community-operations.db'))
        with sqlite3.connect('file:'+database.as_posix()+'?mode=ro',uri=True) as db:
            values=[json.loads(r[0]) for r in db.execute('SELECT body FROM operations ORDER BY rowid DESC')]
        return next(o for o in values if o['action']['permission']=='admit_entry' and o['action']['payload']['request']['activity_id']==identifier)
    def evidence(self,identifier,request_id):
        self.select(self.o,identifier);self.call(self.o,'CollectEvidence');self.call(self.o,'CollectCommunityEvidence',self.group)
        activity=json.loads((self.o.directory/'events.private.json').read_text('utf8'))
        community=json.loads((self.o.directory/'community-events.private.json').read_text('utf8'))
        requests=[e for e in community['events'] if e['content'].get('org.buwei.action',{}).get('operation_id')==request_id]
        invitations=[e for e in activity['events'] if e['type']=='m.room.member' and e.get('state_key')==self.accounts[1] and e['content'].get('membership')=='invite' and e['sender']==self.accounts[0]]
        assert len(requests)==1
        return len(invitations),[e['origin_server_ts'] for e in invitations],activity['collected_at_unix']
    def run(self):
        for role in ('organizer','participant'):self.h.start(role)
        self.accounts=[json.loads((self.h.profiles[r]/'data'/('v'+self.args.version)/'rinx-binding-status.json').read_text('utf8'))['account'] for r in ('organizer','participant')]
        self.o=Control(self.args.owner_profile,self.accounts[0],selected(self.args.owner_profile,self.args.version)['room']);self.m=Control(self.args.participant_profile,self.accounts[1],selected(self.args.participant_profile,self.args.version)['room'])
        self.o.pause_automatic_sync(True);self.m.pause_automatic_sync(True);self.o.authorize();self.m.authorize()
        self.group=json.loads((self.o.directory/'community-setup.local.json').read_text('utf8'))['room'];self.d(self.o,'Watch',self.group);self.d(self.m,'Watch',self.group)
        cases=[]
        for fault in ('lost_ack','crash_before_receipt','before_send'):
            form=self.form('badminton','社群恢复验收 · '+fault)
            existing=self.call(self.o,'Refresh').get('activities') or []
            previous=next((a for a in existing if a['title']==form['title']),None)
            if previous:
                identifier=previous['metadata']['activity_id'];self.select(self.o,identifier);original=self.op(identifier)
                assert original['status']=='confirmed' and fault=='lost_ack'
                assert any(any(row[1]=='unknown' and row[2]==original['id'] for row in (t['result'].get('community',{}).get('pending_requests') or [])) for t in self.trace)
                folder=self.args.participant_profile/'data'/('v'+self.args.version)/'native/rinx';database=next(folder.glob('*/community-operations.db'))
                with sqlite3.connect('file:'+database.as_posix()+'?mode=ro',uri=True) as db:requests=[json.loads(r[0]) for r in db.execute('SELECT body FROM operations')]
                request=next(o for o in requests if o['action']['permission']=='request_entry' and o['action']['payload']['listing']['announcement']['activity_id']==identifier)
                count,times,collected=self.evidence(identifier,request['id']);assert count==1
                cases.append(dict(fault=fault,original_operation_recovered=True,blind_resends=0,operation_sha256=hashlib.sha256(original['id'].encode()).hexdigest(),service_event_count=count,server_times_ms=times,locally_collected_at_unix=collected,status_after_recovery='confirmed',member_request_receipt_recovered=True))
                self.d(self.m,'Refresh');self.d(self.m,'PreviewEntry',self.group+'|'+identifier);self.d(self.m,'ConfirmEntry',request['id']);r=self.call(self.m,'JoinCard');self.bind(self.m,r['activity']['room']);print('Actual community lost_ack recovery reverified',flush=True);continue
            r=self.call(self.o,'CreateDated',form);identifier=r['activity']['metadata']['activity_id'];self.bind(self.o,r['activity']['room'])
            self.publish(identifier);self.access(identifier);self.d(self.m,'Refresh')
            r=self.d(self.m,'PreviewEntry',self.group+'|'+identifier);request=r['community']['operation']
            if fault=='lost_ack':self.call(self.m,'TestFault','lost_ack')
            r=self.d(self.m,'ConfirmEntry',request['id'])
            if fault=='lost_ack':
                assert r['community']['operation']['status']=='unknown';r=self.d(self.m,'Reconcile');assert r['community']['operation']['id']==request['id'] and r['community']['operation']['status']=='confirmed'
                self.d(self.m,'ConfirmEntry',request['id'])
            self.call(self.o,'TestFault',fault)
            if fault=='crash_before_receipt':
                try:self.d(self.o,'Refresh',timeout=35)
                except TimeoutError:assert self.h.processes['organizer'].poll() is not None
                else:raise AssertionError('Expected intentional receipt crash')
            else:self.d(self.o,'Refresh')
            original=self.op(identifier);assert original['status']==('dispatching' if fault=='crash_before_receipt' else 'unknown')
            if fault!='lost_ack':
                self.h.restart('organizer');self.o=Control(self.args.owner_profile,self.accounts[0],selected(self.args.owner_profile,self.args.version)['room']);self.o.pause_automatic_sync(True);self.o.authorize()
            self.d(self.o,'Reconcile');recovered=self.op(identifier)
            assert recovered['id']==original['id'] and recovered['status']==('unknown' if fault=='before_send' else 'confirmed')
            self.d(self.o,'Refresh');count,times,collected=self.evidence(identifier,request['id']);assert count==(0 if fault=='before_send' else 1)
            cases.append(dict(fault=fault,original_operation_recovered=True,blind_resends=0,operation_sha256=hashlib.sha256(original['id'].encode()).hexdigest(),service_event_count=count,server_times_ms=times,locally_collected_at_unix=collected,status_after_recovery=recovered['status'],member_request_receipt_recovered=fault=='lost_ack'))
            if fault!='before_send':
                r=self.call(self.m,'JoinCard');self.bind(self.m,r['activity']['room'])
            print('Actual community '+fault+' recovery passed',flush=True)
        report=dict(version=self.args.version,passed=True,distinct_identities=2,cases=cases,controlled_test=True,actual_user_trial=False)
        (self.out/'community-recovery.local.json').write_text(json.dumps(report,indent=2),'utf8');return report

if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('--executable',type=pathlib.Path,required=True);p.add_argument('--owner-profile',type=pathlib.Path,required=True);p.add_argument('--participant-profile',type=pathlib.Path,required=True);p.add_argument('--version',required=True);p.add_argument('--private-directory',type=pathlib.Path,required=True);a=p.parse_args();s=Recovery(a)
    try:s.run()
    finally:s.close()
