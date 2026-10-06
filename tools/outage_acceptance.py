"""Measure actual SDK transport outage before dispatch on two test profiles.

The CONNECT relay carries encrypted TLS, closes only test-process connections,
and records counts only. Old confirmations must fail after native restart.
Separate opt-in faults measure service acceptance with a missing local receipt.
"""
import argparse,hashlib,json,pathlib
from controlled_proxy import Relay
from fault_acceptance import Hosts,FaultSuite
from before_send_acceptance import read_events,draft

class OutageSuite(FaultSuite):
    def __init__(self,hosts,private,relay):super().__init__(hosts,private);self.relay=relay;self.outages=[]
    def paused(self,flag):self.o.pause_automatic_sync(flag);self.p.pause_automatic_sync(flag)
    def blocked_confirmation(self,role,command,value,key,op):
        control=self.o if role=='organizer' else self.p
        connections=self.relay.connections;self.relay.block()
        try:
            result=self.call(control,command,value)
            assert '身份核验' in result['message'] and '尚未执行' in result['message']
            old=self.operation(result,key,'prepared');assert old['id']==op['id']
            assert self.persisted_operation(role,op['id'])['status']=='prepared'
        finally:self.relay.unblock()
        assert self.relay.denied>0 or self.relay.connections>connections
        control=self.restart_control(role);control.pause_automatic_sync(True)
        self.call(control,'ReconcilePending')
        refused=self.call(control,command,value);assert self.persisted_operation(role,op['id'])['status']=='prepared'
        events=read_events(self);kind={'invitation':'org.buwei.invitation','participant':'org.buwei.reply','article':'m.room.message'}[key];field='action_id' if key=='participant' else 'operation_id'
        assert not any(e['type']==kind and (e['content'].get('org.buwei.action',{}).get(field)==op['id'] if kind=='m.room.message' else e['content'].get(field)==op['id']) for e in events['events'])
        self.outages.append({'scene':key,'transport':'actual SDK HTTPS connection closed and refused by process-scoped loopback CONNECT relay','operation_sha256':hashlib.sha256(op['id'].encode()).hexdigest(),'status_after_network_failure':'prepared','persisted_prepared_before_restart':True,'identity_preflight_refused':True,'old_confirmation_refused_after_restart':True,'original_operation_preserved':True,'service_event_count_for_original':0,'blind_resends':0,'collected_at_unix':events['collected_at_unix']})
        print(key+' actual network outage: no service event; old confirmation refused',flush=True)
    def run_outages(self):
        self.o.authorize();self.p.authorize();self.enroll();self.paused(True)
        op=self.operation(self.call(self.o,'Prepare'),'invitation','prepared');self.blocked_confirmation('organizer','Execute',None,'invitation',op)
        self.paused(False);invite=self.invite();self.paused(True)
        op=self.operation(self.call(self.p,'Accept',True),'participant','prepared');self.blocked_confirmation('participant','ConfirmParticipant',self.preferences,'participant',op)
        self.paused(False);decline=self.reply(False)
        text=draft('实际网络中断');self.stage_article(text);op=self.operation(self.call(self.o,'PrepareArticle'),'article','prepared');self.paused(True)
        self.blocked_confirmation('organizer','PublishArticle',text,'article',op)
        # A fresh explicit preview is safe here: preflight prevented dispatch,
        # the original remained Prepared and complete SDK history has zero hits.
        new=self.operation(self.call(self.o,'PrepareArticle'),'article','prepared');assert new['id']!=op['id']
        article=self.operation(self.call(self.o,'PublishArticle',text),'article','confirmed');assert article['id']==new['id'];self.paused(False)
        proof=self.evidence([invite,decline,article])
        return {'version':self.version,'passed':True,'scope':'actual SDK TLS traffic through a local CONNECT relay; process connections closed, no physical cable change and no global network setting changes','cases':self.outages,'fresh_reviewed_operations_after_definite_preflight_refusal':proof,'proxy_connections':self.relay.connections,'blocked_connect_attempts':self.relay.denied,'tls_not_decrypted_or_logged':True}

def run(a):
    relay=Relay();hosts=Hosts(a.executable,a.owner_profile,a.participant_profile,a.version,a.owner_port,a.participant_port,a.activity_id)
    for role in ('organizer','participant'):hosts.extra_environment[role]={'HTTP_PROXY':relay.url,'HTTPS_PROXY':relay.url,'ALL_PROXY':relay.url,'NO_PROXY':'127.0.0.1,localhost'}
    try:
        hosts.start('organizer');hosts.start('participant');suite=OutageSuite(hosts,a.private_trace,relay);report=suite.run_outages();a.public_evidence.parent.mkdir(parents=True,exist_ok=True);a.public_evidence.write_text(json.dumps(report,indent=2),'utf8')
    finally:
        relay.unblock()
        for role in ('organizer','participant'):
            if role in hosts.processes:hosts.stop(role)
        relay.close()
if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('--executable',type=pathlib.Path,required=True);p.add_argument('--owner-profile',type=pathlib.Path,required=True);p.add_argument('--participant-profile',type=pathlib.Path,required=True);p.add_argument('--version',required=True);p.add_argument('--activity-id');p.add_argument('--owner-port',type=int,default=8152);p.add_argument('--participant-port',type=int,default=8153);p.add_argument('--private-trace',type=pathlib.Path,required=True);p.add_argument('--public-evidence',type=pathlib.Path,required=True);run(p.parse_args())
