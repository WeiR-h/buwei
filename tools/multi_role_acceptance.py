"""Five real SDK rounds in each direction on two explicitly supplied profiles.

Uses the isolated, opt-in acceptance host; publishes no raw identities or events.
The public directory alias must be an explicitly selected synthetic test alias.
"""
import argparse,hashlib,json,pathlib,re,sqlite3,sys,time
from types import SimpleNamespace
root=pathlib.Path(__file__).resolve().parents[1]
sys.path.insert(0,str(root/'tools'))
from planning_community_acceptance import NextStage
from formal_control import Control
from intent_acceptance import selected,display

from live_evidence import begin,finish,export
from build_proof import source_fingerprint
parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--executable',type=pathlib.Path,required=True)
parser.add_argument('--owner-profile',type=pathlib.Path,required=True)
parser.add_argument('--participant-profile',type=pathlib.Path,required=True)
parser.add_argument('--version',required=True)
parser.add_argument('--private-directory',type=pathlib.Path,required=True)
parser.add_argument('--public-test-alias',required=True)
parser.add_argument('--public-output',type=pathlib.Path)
args=parser.parse_args()
if args.private_directory.exists():raise ValueError('Use a new private result directory; preserve earlier runs')
if args.owner_profile.resolve()==args.participant_profile.resolve():raise ValueError('Two isolated profiles required')
if not re.fullmatch(r'#buwei-test-[A-Za-z0-9_-]+:[a-zA-Z0-9.-]+',args.public_test_alias):raise ValueError('Explicit synthetic test alias required')
binding=begin(args.executable)
if binding['build']['native_source_sha256']!=source_fingerprint():raise ValueError('Build differs from the checked-out native source')
s=NextStage(args)
s.run_id=binding['run_id']

def bind(c,r):
    if r.get('activity'):s.bind(c,r['activity']['room'])
    return r
def plan(c,basics):
    r=s.p(c,'SaveAndPreview',dict(id=None,revision=None,basics=basics))
    assert r['planning']['creation_nonce']
    r=bind(c,s.p(c,'ConfirmCreate',dict(nonce=r['planning']['creation_nonce'],basics=basics)))
    assert r['roles']['organizer'] and r['roles']['can_create']
    return r['activity']['metadata']['activity_id']
def person(a,account):return next((p for p in a['people'] if p['account']==account),{})
def publish(c,identifier):
    s.select(c,identifier)
    r=s.d(c,'PreviewPublicPublish',dict(withdrawn=False));op=r['community']['operation']
    assert op['action']['payload']['announcement']['audience']=='public'
    r=s.d(c,'ConfirmPublish',op['id']);assert r['community']['operation']['status']=='confirmed'
    r=s.d(c,'PreviewAccess',s.group+'|'+identifier)
    s.d(c,'ConfirmAccess',r['community']['access_nonce'])
def pair(creator,member,identifier,actor):
    s.select(creator,identifier)
    before=s.call(creator,'Refresh')['activity']
    s.d(member,'Refresh')
    r=s.d(member,'PreviewEntry',s.group+'|'+identifier)
    op=r['community']['operation']
    if op is not None:
        r=s.d(member,'ConfirmEntry',op['id']);assert r['community']['operation']['status']=='confirmed'
        s.d(creator,'Refresh')
        r=bind(member,s.call(member,'JoinCardAndPrepare'))
    else:
        bind(member,r);r=s.call(member,'QuickJoin')
    assert not r['roles']['organizer'] and r['roles']['can_create']
    join=r['participant'];assert join['status']=='prepared'
    assert r['activity']['owner']!=actor
    a=r['activity'];prefs=dict(earliest=a['start'],latest=a['end'],group=1)
    assert join['action']['payload']['preferences']==prefs
    prior_preferences=r['intentions']['preferences']
    understood=s.call(member,'SuggestDated',dict(requirement='参加整场，两个人',answer=''))
    assert understood['dated_suggestion']['group']==2 and understood['dated_suggestion']['questions']==[]
    assert understood['dated_suggestion']['earliest']==display(a['start']) and understood['dated_suggestion']['latest']==display(a['end'])
    assert understood['intentions']['preferences']==prior_preferences
    # A prepared preview changes neither the authoritative activity nor membership seats.
    s.call(creator,'SyncActivity');assert s.call(creator,'Refresh')['activity']==before
    r=s.call(member,'ConfirmParticipant',prefs);assert r['participant']['status']=='confirmed'
    settings=dict(invitation_minutes=5,quiet_start=0,quiet_end=24,max_invitations=30)
    r=s.call(creator,'PreviewAutomation',settings)
    s.call(creator,'ConfirmAutomation',dict(id=r['policy_consent_id'],settings=settings))
    s.call(creator,'SyncActivity')
    r=s.wait(member,lambda r:any(i['recipient']==actor and i['delivery']=='delivered' and i['reply']=='pending' for i in r['activity']['invitations']))
    invitation=next(i for i in reversed(r['activity']['invitations']) if i['recipient']==actor and i['reply']=='pending')
    reply=s.call(member,'Accept',True)['participant']
    s.call(member,'ConfirmParticipant',prefs);s.call(creator,'SyncActivity')
    s.wait(creator,lambda r:person(r['activity'],actor).get('status')=='confirmed')
    s.wait(member,lambda r:person(r['activity'],actor).get('status')=='confirmed')
    denied=s.call(member,'PreviewRosterExport',str(s.out/(hashlib.sha256(actor.encode()).hexdigest()+'.csv')),ok=False)
    assert not denied['success']
    export=s.out/(identifier+'-'+str(round_number)+'.csv')
    r=s.call(creator,'PreviewRosterExport',str(export));s.call(creator,'ConfirmRosterExport',r['roster_export']['nonce'])
    cancel=s.call(member,'Cancel')['participant'];s.call(member,'ConfirmParticipant',prefs);s.call(creator,'SyncActivity')
    s.wait(creator,lambda r:person(r['activity'],actor).get('status')=='cancelled')
    s.wait(member,lambda r:person(r['activity'],actor).get('status')=='cancelled')
    native=creator.profile/'data'/('v'+args.version)/'native/rinx';db=next(native.glob('*/operations.db'))
    with sqlite3.connect('file:'+db.as_posix()+'?mode=ro',uri=True) as c:invite=json.loads(c.execute('SELECT body FROM operations WHERE id=?',(invitation['operation_id'],)).fetchone()[0])
    original=s.o;s.o=creator
    try:evidence=s.proof([join,invite,reply,cancel])
    finally:s.o=original
    return evidence
try:
    alias=args.public_test_alias
    for role in ('organizer','participant'):
        s.h.extra_environment[role]={'BUWEI_PUBLIC_TEST_ALIAS':alias}
        s.h.start(role)
    s.accounts=[json.loads((s.h.profiles[r]/'data'/('v'+args.version)/'rinx-binding-status.json').read_text('utf8'))['account'] for r in ('organizer','participant')]
    assert s.accounts[0]!=s.accounts[1]
    s.o=Control(args.owner_profile,s.accounts[0],(selected(args.owner_profile,args.version) or {}).get('room',''))
    s.m=Control(args.participant_profile,s.accounts[1],(selected(args.participant_profile,args.version) or {}).get('room',''))
    for c in (s.o,s.m):c.pause_automatic_sync(True);c.authorize()
    for c in (s.o,s.m):
        r=s.d(c,'PreviewPublicDirectory')
        if r['public_directory']['nonce']:
            r=s.d(c,'ConfirmPublicDirectory',r['public_directory']['nonce'])
        assert r['public_directory']['mode']
        room=r['public_directory']['room'];assert room
        if c is s.o:s.group=room
        else:assert room==s.group
    start=int(time.time())//86400*86400+3*86400+11*3600+30*60
    basic=dict(title='多组织者 · 公开读书会',template='reading',start=display(start),end=display(start+7200),location='Rinx线上测试大厅',capacity=2,description='本机两个真实身份的受控测试，不代表实际到场。')
    existing=s.call(s.o,'Refresh').get('activities') or []
    reading=next((a['metadata']['activity_id'] for a in existing if a['title']==basic['title']),None) or plan(s.o,basic)
    other=dict(basic,title='多组织者 · 公开桌游',template='boardgame',start=display(start+3*3600),end=display(start+5*3600))
    existing=s.call(s.m,'Refresh').get('activities') or []
    board=next((a['metadata']['activity_id'] for a in existing if a['title']==other['title']),None) or plan(s.m,other)
    publish(s.o,reading);publish(s.m,board)
    for c in (s.o,s.m):
        r=s.d(c,'Refresh');assert {reading,board}<={l['announcement']['activity_id'] for l in r['community']['listings']}
    s.o.pause_automatic_sync(False);s.m.pause_automatic_sync(False)
    evidence_a=[];evidence_b=[]
    for round_number in range(5):
        evidence_a+=pair(s.o,s.m,reading,s.accounts[1])
        evidence_b+=pair(s.m,s.o,board,s.accounts[0])
        print('Latest SDK pair round '+str(round_number+1)+' passed',flush=True)
    for c,own,other_id in ((s.o,reading,board),(s.m,board,reading)):
        s.select(c,own);r=s.call(c,'Refresh');assert r['roles']['organizer'] and r['roles']['can_create']
        s.select(c,other_id);r=s.call(c,'Refresh');assert not r['roles']['organizer'] and r['roles']['can_create']
    # Private-scope publication into the public directory must be rejected.
    s.select(s.o,reading)
    r=s.d(s.o,'PreviewPublish',dict(community=s.group,withdrawn=False),ok=False)
    assert not r['success']
    report=dict(version=args.version,run_id=s.run_id,passed=True,real_rounds=5,actual_sdk_identities=2,distinct_organizers=2,both_accounts_created_and_participated=True,public_alias_join_without_invitation=True,both_public_activities_visible=True,activity_roles_and_roster_permissions_verified=True,private_scope_into_public_directory_rejected=True,explicit_natural_language_understood_without_model=True,long_term_preferences_unchanged=True,all_service_event_counts_one=True,events=evidence_a+evidence_b,model_calls=0,controlled_test=True,actual_user_trial=False,cross_machine_trial_completed=False,core_creation_confirmation_actions=dict(before=3,after=2),core_join_and_signup_actions=dict(before=3,after=2),measurement_scope='Host commands corresponding to the UI buttons; selection, authority setup and seat acceptance reported separately; no end-to-end 30% claim')
    (s.out/'acceptance.local.json').write_text(json.dumps(report,indent=2),'utf8')
    if source_fingerprint()!=binding['build']['native_source_sha256']:raise ValueError('Native source changed during the run')
    finish(binding,args.executable,s.out)
    if args.public_output:print(json.dumps(export(s.out,args.public_output)),flush=True)
    else:print(json.dumps({k:v for k,v in report.items() if k!='events'}),flush=True)
finally:s.close()
