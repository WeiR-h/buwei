"""Measured SDK flow for user-owned test profiles in an opt-in acceptance build.

Private traces contain identities and events. The public summary contains only
hashes, counts and server timestamps. This never creates rooms or calls a model.
"""
import argparse,hashlib,json,pathlib,sqlite3,time
from contextlib import closing
from formal_control import Control

class Suite:
    def __init__(self,owner,participant,version,private):
        self.owner=owner;self.participant=participant;self.version=version;self.private=private;self.trace=[];self.rounds=[]
        self.accounts=[]
        for profile in [owner,participant]:
            binding=json.loads((profile/'data'/('v'+version)/'rinx-binding-status.json').read_text('utf8'))
            if not binding['server_identity_verified']:raise RuntimeError('Current SDK identity is not verified')
            self.accounts.append(binding['account'])
        activity=self.state(owner);self.room=activity['room']
        if activity['owner']!=self.accounts[0] or self.accounts[0]==self.accounts[1]:raise RuntimeError('Two distinct identities required')
        self.o=Control(owner,self.accounts[0],self.room);self.p=Control(participant,self.accounts[1],self.room)
        self.preferences={'earliest':17,'latest':23,'group':1}
    def state(self,profile):
        db=next((profile/'data'/('v'+self.version)/'native/rinx').glob('*/activity.db'))
        with closing(sqlite3.connect('file:'+db.as_posix()+'?mode=ro',uri=True)) as c:return json.loads(c.execute('select body from buwei_state').fetchone()[0])
    def save(self):
        self.private.parent.mkdir(parents=True,exist_ok=True);self.private.write_text(json.dumps(self.trace,ensure_ascii=False,indent=2),'utf8')
    def call(self,control,command,value=None):
        result=control.command(command,value);self.trace.append({'step':command,'result':result});self.save();return result
    def person(self,activity):return next((p for p in activity['people'] if p['account']==self.accounts[1]),{})
    def wait(self,profile,label,predicate,timeout=70):
        start=time.monotonic()
        while time.monotonic()-start<timeout:
            activity=self.state(profile)
            if predicate(activity):
                self.trace.append({'step':label,'automatic':True,'collected_at_unix':int(time.time()),'elapsed_seconds':round(time.monotonic()-start,2),'activity':activity});self.save();return activity
            time.sleep(.4)
        raise TimeoutError('Automatic synchronization failed: '+label)
    def operation(self,result,key,status):
        operation=result[key]
        if not operation or operation['status']!=status:raise AssertionError(key+' expected '+status+'; '+result['message'])
        return operation
    def enroll(self):
        op=self.operation(self.call(self.p,'Join',self.preferences),'participant','prepared')
        sent=self.operation(self.call(self.p,'ConfirmParticipant',self.preferences),'participant','confirmed');assert sent['id']==op['id']
        self.wait(self.owner,'organizer verified registration',lambda a:self.person(a).get('status')=='waiting' and self.person(a)['preferences']==self.preferences)
        self.wait(self.participant,'participant received registration',lambda a:self.person(a).get('status')=='waiting' and self.person(a)['preferences']==self.preferences)
        return sent
    def invite(self):
        op=self.operation(self.call(self.o,'Prepare'),'invitation','prepared')
        sent=self.operation(self.call(self.o,'Execute'),'invitation','confirmed');assert sent['id']==op['id']
        again=self.operation(self.call(self.o,'Execute'),'invitation','confirmed');assert again['id']==sent['id']
        self.wait(self.participant,'participant received delivered invitation',lambda a:any(i['operation_id']==sent['id'] and i['delivery']=='delivered' for i in a['invitations']))
        return sent
    def reply(self,accept):
        op=self.operation(self.call(self.p,'Accept',accept),'participant','prepared')
        sent=self.operation(self.call(self.p,'ConfirmParticipant',self.preferences),'participant','confirmed');assert sent['id']==op['id']
        again=self.operation(self.call(self.p,'ConfirmParticipant',self.preferences),'participant','confirmed');assert again['id']==sent['id']
        status='confirmed' if accept else 'declined'
        self.wait(self.owner,'organizer verified reply',lambda a:self.person(a).get('status')==status)
        self.wait(self.participant,'participant received final reply',lambda a:self.person(a).get('status')==status)
        return sent
    def cancel(self):
        self.operation(self.call(self.p,'Cancel'),'participant','prepared')
        op=self.operation(self.call(self.p,'ConfirmParticipant',self.preferences),'participant','confirmed')
        self.wait(self.owner,'organizer released cancelled seat',lambda a:self.person(a).get('status')=='cancelled' and a['invitations'][-1]['reply']=='accepted')
        self.wait(self.participant,'participant received final cancellation',lambda a:self.person(a).get('status')=='cancelled')
        return op
    def evidence(self,operations):
        self.call(self.o,'CollectEvidence');source=self.owner/'.run/acceptance/events.private.json';events=json.loads(source.read_text('utf8'));proof=[]
        for op in operations:
            event_id=op['receipt']['evidence']['external_id'];matches=[e for e in events['events'] if e['event_id']==event_id]
            assert len(matches)==1 and matches[0]['sender']==op['account']
            key='action_id' if op['action']['permission']=='participate' else 'operation_id'
            matching_action=[e for e in events['events'] if e['type']==matches[0]['type'] and (e['content'].get('org.buwei.action',{}).get(key)==op['id'] if e['type']=='m.room.message' else e['content'].get(key)==op['id'])]
            assert len(matching_action)==1
            proof.append({'operation_sha256':hashlib.sha256(op['id'].encode()).hexdigest(),'event_sha256':hashlib.sha256(event_id.encode()).hexdigest(),'event_type':matches[0]['type'],'origin_server_ts_ms':matches[0]['origin_server_ts'],'service_event_count':1})
        return {'events':proof,'locally_collected_at_unix':events['collected_at_unix']}
    def run(self,rounds,extras):
        self.o.authorize();self.p.authorize()
        for n in range(rounds):
            join=self.enroll();invite=self.invite();reply=self.reply(True);assert reply['id']!=invite['id'];cancel=self.cancel()
            self.rounds.append(self.evidence([join,invite,reply,cancel]));print('Actual SDK round '+str(n+1)+' passed',flush=True)
        report={'version':self.version,'passed':True,'execution_variant':'opt-in acceptance; same Controller, SDK and journal as full-host','formal_rounds':self.rounds,'no_manual_refresh':True,'repeat_confirmation_uses_original_id':True,'reply_and_invitation_ids_distinct':True}
        if extras:
            join=self.enroll();invite=self.invite();reply=self.reply(False);report['decline']=self.evidence([join,invite,reply]);print('Decline passed',flush=True)
            join=self.enroll();invite=self.invite();self.wait(self.owner,'organizer released expired invitation',lambda a:a['invitations'][-1]['operation_id']==invite['id'] and a['invitations'][-1]['reply']=='expired',timeout=350)
            self.wait(self.participant,'participant received expiration',lambda a:a['invitations'][-1]['reply']=='expired')
            report['expiry']=self.evidence([join,invite]);report['expiry']['hold_deadline_unix']=invite['action']['payload']['until'];report['expiry']['clock_source']='HTTPS homeserver Date before complete SDK pagination; Matrix event origin_server_ts for reply evidence';print('Actual five-minute expiration passed',flush=True)
        return report

if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('--owner-profile',type=pathlib.Path,required=True);p.add_argument('--participant-profile',type=pathlib.Path,required=True);p.add_argument('--version',required=True);p.add_argument('--rounds',type=int,default=5);p.add_argument('--extras',action='store_true');p.add_argument('--private-trace',type=pathlib.Path,required=True);p.add_argument('--public-evidence',type=pathlib.Path,required=True);a=p.parse_args()
    if not 1<=a.rounds<=5:raise ValueError('Use one to five explicitly bounded rounds')
    suite=Suite(a.owner_profile,a.participant_profile,a.version,a.private_trace);report=suite.run(a.rounds,a.extras);a.public_evidence.parent.mkdir(parents=True,exist_ok=True);a.public_evidence.write_text(json.dumps(report,indent=2),'utf8');print(json.dumps({'version':a.version,'passed':True,'rounds':len(report['formal_rounds'])}))
