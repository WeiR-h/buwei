"""Controlled reading-club correction on two verified identities.

This does not stand in for a three-person pilot. The same host commands require
consent and explicit previews; raw SDK evidence remains in private output.
"""
import argparse,hashlib,json,pathlib,time
from fault_acceptance import Hosts
from intent_acceptance import IntentSuite,display,setup_activity,selected,request
from formal_control import Control

class ReadingSuite(IntentSuite):
    def form(self,role,end=None):
        a=self.state(self.owner)
        return dict(kind=role,activity_id=self.activity_id,title='持续关注读书会',template='reading',earliest=display(a['start']),latest=display(end or a['end']),group='1',target='1' if role=='organize' else '',check_at=display(int(time.time())//60*60-60) if role=='organize' else '',recurrence_days='',preparation_hours='24',availability=None)
    def run_reading(self,rounds=5):
        self.o.authorize(); self.p.authorize(); self.preferences['group']=1
        settings={'invitation_minutes':5,'quiet_start':0,'quiet_end':24,'max_invitations':30}
        policy=self.call(self.o,'PreviewAutomation',settings)
        assert self.call(self.o,'ConfirmAutomation',{'id':policy['policy_consent_id'],'settings':settings})['success']
        self.organizer_goal=self.intent(self.o,'SaveGoal',{'id':None,'form':self.form('organize')})['goal_id']
        before_prefs=self.call(self.p,'Refresh')['intentions']['preferences']
        for n in range(rounds):
            # Prepare a task, then explicitly narrow this goal before any send.
            self.member_goal=self.intent(self.p,'SaveGoal',{'id':None,'form':self.form('participate')})['goal_id']
            card=self.card(self.p,'opportunity')
            if card['goal_id']!=self.member_goal:raise AssertionError('Wrong goal opportunity')
            preview=self.use(self.p,card);initial=preview['participant']
            task=next(t for t in preview['intentions']['tasks'] if any(s.get('operation_id')==initial['id'] for s in t['steps']))
            a=self.state(self.owner);partial=dict(self.preferences,latest=a['end']-3600)
            self.intent(self.p,'SaveGoal',{'id':self.member_goal,'form':self.form('participate',partial['latest'])})
            stale=self.call(self.p,'ConfirmParticipant',self.preferences);assert not stale['success']
            submitted=self.operation(self.call(self.p,'Join',partial),'participant','prepared')
            sent=self.operation(self.call(self.p,'ConfirmParticipant',partial),'participant','confirmed');assert submitted['id']==sent['id']
            current=self.wait(self.owner,'time-limited candidate verified',lambda x:self.person(x).get('status')=='waiting' and self.person(x)['preferences']==partial)
            self.wait(self.participant,'time-limited candidate received',lambda x:self.person(x).get('status')=='waiting' and self.person(x)['preferences']==partial)
            sequence=self.person(current)['joined'];invitation_count=len(current['invitations'])
            blocked=self.card(self.o,'no_candidate')
            assert '不覆盖活动' in blocked['reason'] and display(partial['latest']) in blocked['reason']
            proposed=self.intent(self.p,'PrepareGoalCorrection',{'id':self.member_goal,'text':'这次能待到晚上九点半，还是我一个人。'})
            correction=next(c for c in proposed['intentions']['corrections'] if c['goal_id']==self.member_goal and not c['applied'])
            assert not correction['questions'] and correction['after']['latest']==a['end']
            assert self.person(self.state(self.owner))['preferences']==partial
            changed=self.intent(self.p,'ConfirmGoalCorrection',{'id':correction['id'],'scope':'this_occasion'})
            assert changed['intentions']['preferences']==before_prefs
            assert self.person(self.state(self.owner))['preferences']==partial
            repeated=self.call(self.p,'Assistance',{'command':'ConfirmGoalCorrection','value':{'id':correction['id'],'scope':'this_occasion'}});assert not repeated['success']
            prepared=self.intent(self.p,'PrepareGoalRegistrationUpdate',self.member_goal)
            update=self.operation(prepared,'participant','prepared')
            continued=next(t for t in prepared['intentions']['tasks'] if any(s.get('operation_id')==update['id'] for s in t['steps']))
            assert continued['id']==task['id'] and update['id']!=sent['id']
            updated=self.operation(self.call(self.p,'ConfirmParticipant',self.preferences),'participant','confirmed');assert updated['id']==update['id']
            repeat=self.operation(self.call(self.p,'ConfirmParticipant',self.preferences),'participant','confirmed');assert repeat['id']==update['id']
            self.wait(self.owner,'corrected registration verified',lambda x:self.person(x)['preferences']==self.preferences and self.person(x)['joined']==sequence)
            invite=self.automatic_invitation();assert len(self.state(self.owner)['invitations'])==invitation_count+1
            reply=self.reply(True);assert reply['id']!=invite['id']
            self.wait(self.owner,'target registration count verified',lambda x:x['people'] and self.person(x)['status']=='confirmed')
            deadline=time.monotonic()+80
            while True:
                state=self.call(self.p,'Refresh')['intentions'];finished=next(t for t in state['tasks'] if t['id']==task['id'])
                if finished['status']=='completed':break
                if time.monotonic()>deadline:raise AssertionError('Original registration task did not finish')
                time.sleep(1)
            cancel=self.cancel();proof=self.evidence([sent,updated,invite,reply,cancel])
            proof.update(queue_position_preserved=True,original_task_preserved=True,goal_and_registration_confirmed_separately=True,preferences_unchanged=True,stale_preview_rejected=True)
            self.rounds.append(proof)
            self.intent(self.p,'SetGoalStatus',{'id':self.member_goal,'status':'completed'})
            print(f'Controlled reading round {n+1} passed',flush=True)
        return {'version':self.version,'passed':True,'controlled_two_identity_rounds':self.rounds,'three_person_pilot_completed':False,'source':'Actual SDK, official model host and server event evidence; opt-in acceptance executable'}

def run(args):
    h=Hosts(args.executable,args.owner_profile,args.participant_profile,args.version,args.owner_port,args.participant_port)
    try:
        for role in ('organizer','participant'):h.start(role)
        identifier,_,_=setup_activity(h,template='reading',capacity=1);h.activity_id=identifier
        suite=ReadingSuite(h,args.private_trace,identifier);report=suite.run_reading(args.rounds)
        args.output.parent.mkdir(parents=True,exist_ok=True);args.output.write_text(json.dumps(report,indent=2),'utf8')
    finally:
        for role in ('organizer','participant'):
            if role in h.processes:h.stop(role)
if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('--executable',type=pathlib.Path,required=True);p.add_argument('--owner-profile',type=pathlib.Path,required=True);p.add_argument('--participant-profile',type=pathlib.Path,required=True);p.add_argument('--version',required=True);p.add_argument('--rounds',type=int,default=5);p.add_argument('--owner-port',type=int,default=8190);p.add_argument('--participant-port',type=int,default=8191);p.add_argument('--private-trace',type=pathlib.Path,required=True);p.add_argument('--output',type=pathlib.Path,required=True);run(p.parse_args())
