"""Actual proactive SDK flow on two explicitly opted-in test profiles.

Creates one private test activity, shares with the second verified identity,
and exercises the same goals, recommendations, confirmations and receipt core
as the application. Raw identities and model output stay in private traces.
"""
import argparse,datetime,hashlib,json,pathlib,sqlite3,time
from contextlib import closing
from dual_acceptance import Suite
from fault_acceptance import Hosts
from formal_control import Control

def display(timestamp):
    return datetime.datetime.fromtimestamp(timestamp,datetime.timezone(datetime.timedelta(hours=8))).strftime('%Y-%m-%d %H:%M')
def selected(profile,version):
    actor=next((profile/'data'/('v'+version)/'native/rinx').glob('*/operations.db')).parent
    identifier=(actor/'selected-activity.txt').read_text('utf8').strip() if (actor/'selected-activity.txt').exists() else None
    path=actor/'activities'/identifier/'activity.db' if identifier else actor/'activity.db'
    with closing(sqlite3.connect('file:'+path.as_posix()+'?mode=ro',uri=True)) as db:
        return json.loads(db.execute('select body from buwei_state').fetchone()[0])
def request(control,name,value=None):
    r=control.command(name,value,timeout=180)
    if not r.get('success'):raise AssertionError(name+': '+r.get('message','No result'))
    return r

def setup_activity(hosts):
    accounts=[];controls=[]
    for role in ('organizer','participant'):
        profile=hosts.profiles[role];binding=json.loads((profile/'data'/('v'+hosts.version)/'rinx-binding-status.json').read_text('utf8'))
        if not binding['server_identity_verified']:raise RuntimeError('SDK identity not verified')
        accounts.append(binding['account']);controls.append(Control(profile,binding['account'],selected(profile,hosts.version)['room']))
    if accounts[0]==accounts[1]:raise RuntimeError('Distinct SDK identities required')
    o,p=controls;o.authorize();p.authorize()
    tomorrow=datetime.datetime.now(datetime.timezone(datetime.timedelta(hours=8)))+datetime.timedelta(days=1)
    start=int(tomorrow.replace(hour=19,minute=30,second=0,microsecond=0).timestamp())
    title='补位主动帮助验收 v'+hosts.version+' '+datetime.datetime.now().strftime('%Y%m%d-%H%M%S')
    form=dict(title=title,capacity=2,start=display(start),end=display(start+7200),template='badminton',location='私有测试场地',description='用于主动建议与真实回执闭环验收；不代表实际到场。')
    created=request(o,'CreateDated',form);a=created['activity'];identifier=a['metadata']['activity_id'];room=a['room']
    o=Control(hosts.profiles['organizer'],accounts[0],room)
    request(o,'LoadContacts');preview=request(o,'PrepareShare',accounts[1]);operation=preview['share']
    if operation['action']['payload']['recipient']!=accounts[1] or operation['action']['payload']['card']['room']!=room:raise AssertionError('Share preview differs')
    sent=request(o,'ConfirmShare',accounts[1]);assert sent['share']['id']==operation['id'] and sent['share']['status']=='confirmed'
    request(p,'OpenCard',{'room':room,'activity_id':identifier});request(p,'JoinCard')
    p=Control(hosts.profiles['participant'],accounts[1],room)
    return identifier,o,p

class IntentSuite(Suite):
    def __init__(self,hosts,private,identifier):
        super().__init__(hosts.profiles['organizer'],hosts.profiles['participant'],hosts.version,private,identifier)
        self.hosts=hosts;self.tasks=[];self.organizer_goal=None;self.member_goal=None
    def intent(self,control,name,value=None):
        packet={'command':name}
        if value is not None:packet['value']=value
        r=self.call(control,'Assistance',packet)
        if not r.get('success'):raise AssertionError(name+': '+r['message'])
        return r
    def goal(self,control,role):
        a=self.state(self.owner);form=dict(kind=role,activity_id=self.activity_id if role=='organize' else None,title='按本人确认的目标持续跟进',template='badminton',earliest=display(a['start']),latest=display(a['end']),group='1' if role=='organize' else str(self.preferences['group']),target=str(a['capacity']) if role=='organize' else '',check_at=display(int(time.time())//60*60-60) if role=='organize' else '',recurrence_days='',preparation_hours='24')
        r=self.intent(control,'SaveGoal',{'id':None,'form':form});return r['goal_id']
    def card(self,control,kind,timeout=95):
        deadline=time.monotonic()+timeout
        while time.monotonic()<deadline:
            r=self.call(control,'Refresh')
            c=next((c for c in r.get('assistance_cards',[]) if c['kind']==kind and c.get('activity_id')==self.activity_id),None)
            if c:return c
            time.sleep(1)
        raise TimeoutError('Verified proactive card did not appear: '+kind)
    def use(self,control,card):
        # A background sync may refresh the facts between reading and using a
        # card. Retry only a rejected preparation, with the latest same card.
        # Confirmations and network effects must never be retried this way.
        for attempt in range(3):
            result = self.call(control,'Assistance',{'command':'UseCard','value':{'id':card['id'],'fingerprint':card['fingerprint']}})
            if result.get('success'):return result
            if result.get('message') != '建议依据已变化或过期，请核对最新卡片' or attempt == 2:
                raise AssertionError('UseCard: '+result.get('message','No result'))
            refreshed = self.call(control,'Refresh')
            latest = next((c for c in refreshed.get('assistance_cards',[]) if c['id'] == card['id']),None)
            if not latest or any(latest.get(k) != card.get(k) for k in ('kind','goal_id','goal_revision','activity_id','action')):
                raise AssertionError('Card no longer represents the same user-confirmed goal')
            card = latest
        raise AssertionError('Card preparation did not finish')
    def automatic_invitation(self):
        a=self.wait(self.participant,'automatic invitation delivered',lambda a:any(i['recipient']==self.accounts[1] and i['reply']=='pending' and i['delivery']=='delivered' for i in a['invitations']),timeout=95)
        identifier=a['invitations'][-1]['operation_id'];r=self.call(self.o,'Refresh')
        # Automatic operations are in the journal, independent of the selected preview.
        db=next((self.owner/'data'/('v'+self.version)/'native/rinx').glob('*/operations.db'))
        with closing(sqlite3.connect('file:'+db.as_posix()+'?mode=ro',uri=True)) as c:
            op=json.loads(c.execute('select body from operations where id=?',(identifier,)).fetchone()[0])
        assert op['status']=='confirmed' and op['action']['target']==self.room
        return op
    def run_intents(self,rounds=5):
        self.o.authorize();self.p.authorize()
        pref={'template':'badminton','group':self.preferences['group'],'weekdays':[],'earliest_minute':None,'latest_minute':None,'quiet_start':22,'quiet_end':8,'reminders':True,'confirmed_at':0}
        self.intent(self.p,'SavePreferences',pref)
        owner_pref=self.call(self.o,'Refresh')['intentions']['preferences']
        member_pref=self.call(self.p,'Refresh')['intentions']['preferences']
        assert owner_pref['group'] is None and member_pref['group']==self.preferences['group']
        self.organizer_goal=self.goal(self.o,'organize');self.member_goal=self.goal(self.p,'participate')
        for role,control,own_goal,other_goal in [('organizer',self.o,self.organizer_goal,self.member_goal),('participant',self.p,self.member_goal,self.organizer_goal)]:
            state=self.call(control,'Refresh')['intentions'];assert state['account']==self.accounts[0 if role=='organizer' else 1];assert any(g['id']==own_goal for g in state['goals']);assert not any(g['id']==other_goal for g in state['goals'])
        self.use(self.o,self.card(self.o,'shortfall'))
        settings={'invitation_minutes':5,'quiet_start':0,'quiet_end':24,'max_invitations':30}
        preview=self.call(self.o,'PreviewAutomation',settings);self.call(self.o,'ConfirmAutomation',{'id':preview['policy_consent_id'],'settings':settings})
        for n in range(rounds):
            ready=self.use(self.p,self.card(self.p,'opportunity'));join=self.operation(ready,'participant','prepared')
            sent=self.operation(self.call(self.p,'ConfirmParticipant',self.preferences),'participant','confirmed');assert join['id']==sent['id']
            invite=self.automatic_invitation();card=self.card(self.p,'invitation');self.use(self.p,card)
            reply=self.reply(True);assert reply['id']!=invite['id']
            state=self.call(self.p,'Refresh')['intentions'];completed=[t for t in state['tasks'] if t['status']=='completed'];assert any(any(s.get('operation_id')==join['id'] for s in t['steps']) for t in completed)
            cancel=self.cancel();self.rounds.append(self.evidence([sent,invite,reply,cancel]));print('Proactive SDK round '+str(n+1)+' passed',flush=True)
        # Explicit feedback changes only the chosen scope and can be undone.
        before=self.call(self.p,'Refresh')['intentions']['preferences']
        once=self.intent(self.p,'Feedback',{'goal_id':self.member_goal,'template':'badminton','group':1,'scope':'this_occasion'})
        assert once['intentions']['preferences']==before
        feedback=once['intentions']['feedback'][-1];self.intent(self.p,'UndoFeedback',feedback['id'])
        long=self.intent(self.p,'Feedback',{'goal_id':self.member_goal,'template':'badminton','group':1,'scope':'long_term'})
        assert long['intentions']['preferences']['group']==1;self.intent(self.p,'UndoFeedback',long['intentions']['feedback'][-1]['id'])
        return {'version':self.version,'passed':True,'formal_rounds':self.rounds,'proactive_detection':True,'personal_confirmation':True,'automatic_invitations_verified':rounds,'account_data_isolation':True,'explicit_feedback_and_undo':True,'source':'actual SDK hosts and server event evidence; opt-in acceptance executable','goal_ids_sha256':[hashlib.sha256(i.encode()).hexdigest() for i in (self.organizer_goal,self.member_goal)]}

def run(args):
    h=Hosts(args.executable,args.owner_profile,args.participant_profile,args.version,args.owner_port,args.participant_port)
    try:
        h.start('organizer');h.start('participant');identifier,_,_=setup_activity(h);h.activity_id=identifier
        suite=IntentSuite(h,args.private_trace,identifier);report=suite.run_intents()
        args.output.parent.mkdir(parents=True,exist_ok=True);args.output.write_text(json.dumps(report,indent=2),'utf8')
    finally:
        for role in ('organizer','participant'):
            if role in h.processes:h.stop(role)
if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('--executable',type=pathlib.Path,required=True);p.add_argument('--owner-profile',type=pathlib.Path,required=True);p.add_argument('--participant-profile',type=pathlib.Path,required=True);p.add_argument('--version',required=True);p.add_argument('--owner-port',type=int,default=8160);p.add_argument('--participant-port',type=int,default=8161);p.add_argument('--private-trace',type=pathlib.Path,required=True);p.add_argument('--output',type=pathlib.Path,required=True);run(p.parse_args())
