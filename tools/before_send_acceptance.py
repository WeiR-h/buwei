"""Preserve uncertain sends on isolated profiles and compare a controlled retry.

Run only on the user's existing two-identity test room. Every send uses the
same native controller and SDK; the deliberate transport failure is explicit.
No firewall, account credentials, new room or model request is involved.
"""
import argparse,hashlib,json,pathlib,time
from fault_acceptance import Hosts,FaultSuite
from formal_control import Control
from migrate import migrate

def copy_pair(source,destination,version):
    for role in ('organizer','participant'):migrate(source/role,destination/role,version,version)

def read_events(s):
    s.call(s.o,'CollectEvidence')
    return json.loads((s.owner/'.run/acceptance/events.private.json').read_text('utf8'))

def uncertain(s,role,command,value,key,operation):
    control=s.o if role=='organizer' else s.p
    s.call(control,'TestFault','before_send')
    first=s.operation(s.call(control,command,value),key,'unknown');assert first['id']==operation['id']
    assert s.persisted_operation(role,operation['id'])['status']=='unknown'
    s.hosts.restart(role)
    profile=s.owner if role=='organizer' else s.participant
    control=Control(profile,s.accounts[0] if role=='organizer' else s.accounts[1],s.room)
    control.pause_automatic_sync(True);control.authorize()
    if role=='organizer':s.o=control
    else:s.p=control
    recovered=s.operation(s.call(control,'ReconcilePending'),key,'unknown');assert recovered['id']==operation['id']
    source=read_events(s)
    kind={'invitation':'org.buwei.invitation','participant':'org.buwei.reply','article':'m.room.message'}[key]
    field='action_id' if key=='participant' else 'operation_id'
    matches=[e for e in source['events'] if e['type']==kind and (e['content'].get('org.buwei.action',{}).get(field)==operation['id'] if kind=='m.room.message' else e['content'].get(field)==operation['id'])]
    assert not matches
    return {'scene':key,'fault':'before_send','persisted_status_before_recovery':'unknown','status_after_recovery':'unknown','original_operation_retained':True,'native_process_restarted':True,'service_event_count':0,'blind_resends':0,'operation_sha256':hashlib.sha256(operation['id'].encode()).hexdigest(),'locally_collected_at_unix':source['collected_at_unix'],'uncertain_profile_preserved':True}

def draft(label):return {'title':'补位发送恢复验收 '+label,'markdown':'这是现有私有双账号测试房间中的回执测试。\n\n正文不包含姓名、账号、房间编号或外部链接。发送结果不明时，保留原编号核实。'}

def run(a):
    base=a.destination.resolve();base.mkdir(exist_ok=False);copy_pair(a.source_profiles,base/'canonical',a.version)
    def hosts(folder,participant=None):return Hosts(a.executable,folder/'organizer',participant or folder/'participant',a.version,a.owner_port,a.participant_port,a.activity_id)
    def suite(h,label):
        h.start('organizer');h.start('participant');return FaultSuite(h,base/(label+'.private.json'))
    h=hosts(base/'canonical');s=suite(h,'registration')
    try:s.o.authorize();s.p.authorize();s.enroll()
    finally:h.stop('organizer');h.stop('participant')
    copy_pair(base/'canonical',base/'uncertain-owner',a.version)
    h=hosts(base/'uncertain-owner');s=suite(h,'owner-uncertain');cases=[]
    try:
        s.o.pause_automatic_sync(True);s.p.pause_automatic_sync(True);s.o.authorize();s.p.authorize()
        op=s.operation(s.call(s.o,'Prepare'),'invitation','prepared');cases.append(uncertain(s,'organizer','Execute',None,'invitation',op))
        assert s.state(s.owner)['invitations'][-1]['delivery']=='unknown' and s.state(s.owner)['invitations'][-1]['reply']=='pending'
        cases[-1]['reservation_preserved']=True
        text=draft('发送前不可用');s.call(s.o,'NewArticle',text);op=s.operation(s.call(s.o,'PrepareArticle'),'article','prepared')
        cases.append(uncertain(s,'organizer','PublishArticle',text,'article',op))
    finally:h.stop('organizer');h.stop('participant')
    copy_pair(base/'canonical',base/'controlled-baseline',a.version)
    h=hosts(base/'controlled-baseline');s=suite(h,'controlled-baseline')
    try:
        s.o.pause_automatic_sync(True);s.p.pause_automatic_sync(True);s.o.authorize();s.p.authorize()
        text=draft('受控基线');s.call(s.o,'NewArticle',text);op=s.operation(s.call(s.o,'PrepareArticle'),'article','prepared')
        assert op['action']['payload']['title']==text['title'] and op['action']['payload']['markdown']==text['markdown']
        s.call(s.o,'TestFault','lost_ack');s.call(s.o,'TestLegacyArticle',text)
        attempts=json.loads((s.owner/'.run/acceptance/legacy-attempts.private.json').read_text('utf8'))['ids'];assert len(attempts)==2
        events=read_events(s);matched=[e for e in events['events'] if e['type']=='m.room.message' and e['sender']==s.accounts[0] and e['content'].get('org.buwei.action',{}).get('operation_id') in attempts]
        assert len(matched)==2 and len({e['event_id'] for e in matched})==2
        baseline={'scene':'article','fault':'lost_ack','scope':'controlled SDK comparison with journal execution/reconciliation bypassed; not historical product measurements','naive_retry_service_events':2,'fresh_transaction_ids':2,'framework_lost_ack_service_events':1,'framework_evidence':a.framework_evidence.as_posix(),'server_timestamps_ms':[e['origin_server_ts'] for e in matched],'event_sha256':[hashlib.sha256(e['event_id'].encode()).hexdigest() for e in matched],'locally_collected_at_unix':events['collected_at_unix']}
    finally:h.stop('organizer');h.stop('participant')
    # Keep the original participant journal free of uncertain test replies.
    migrate(base/'canonical/participant',base/'uncertain-participant',a.version,a.version)
    h=hosts(base/'canonical',base/'uncertain-participant');s=suite(h,'reply-uncertain')
    try:
        s.o.authorize();s.p.authorize();invite=s.invite()
        op=s.operation(s.call(s.p,'Accept',True),'participant','prepared');assert op['id']!=invite['id']
        cases.append(uncertain(s,'participant','ConfirmParticipant',s.preferences,'participant',op))
        s.wait(s.owner,'actual server-clock expiration after unsent reply',lambda activity:activity['invitations'][-1]['operation_id']==invite['id'] and activity['invitations'][-1]['reply']=='expired',timeout=360)
        cases[-1]['organizer_expired_unanswered_invitation']=True
    finally:h.stop('organizer');h.stop('participant')
    report={'version':a.version,'passed':True,'scope':'actual SDK event reads and restarts; opt-in before-send transport injection; cable connected','cases':cases,'controlled_baseline':baseline,'next_authoritative_source':'private canonical organizer and preserved canonical participant profiles; reauthorize and sync before using'}
    a.public_evidence.parent.mkdir(parents=True,exist_ok=True);a.public_evidence.write_text(json.dumps(report,indent=2),'utf8');print('Three uncertain-send cases and controlled baseline passed',flush=True)

if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('--executable',type=pathlib.Path,required=True);p.add_argument('--source-profiles',type=pathlib.Path,required=True);p.add_argument('--destination',type=pathlib.Path,required=True);p.add_argument('--version',required=True);p.add_argument('--activity-id');p.add_argument('--owner-port',type=int,default=8142);p.add_argument('--participant-port',type=int,default=8143);p.add_argument('--framework-evidence',type=pathlib.Path,required=True);p.add_argument('--public-evidence',type=pathlib.Path,required=True);run(p.parse_args())
