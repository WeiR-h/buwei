"""Real SDK workflow while the native main window is hidden by its tray.

The opt-in test endpoint dispatches the same menu handler for open, pause and
exit. It does not simulate a physical click in the Windows notification area.
Opening and enabling use the application's documented native widget RPC.
"""
import argparse,json,pathlib,sqlite3,time,urllib.parse,urllib.request
from contextlib import closing
from fault_acceptance import Hosts
from intent_acceptance import IntentSuite

class Native:
    def __init__(self,port):
        self.base='http://127.0.0.1:'+str(port)+'/'
        self.opener=urllib.request.build_opener(urllib.request.ProxyHandler({}))
    def get(self,route,**params):
        with self.opener.open(self.base+route+('?' + urllib.parse.urlencode(params) if params else ''),timeout=8) as r:return json.load(r)
    def click(self,identifier):
        for _ in range(5):
            widgets=self.get('snap')['s'];w=next((w for w in widgets if w.get('i')==identifier and w['r'][2]>0 and w['r'][3]>0),None)
            if w:
                x,y,width,height=w['r'];window=next(z for z in self.get('s')['w'] if z['i']==w['w'])
                if 0<=y+height/2<window['sz'][1]:return self.get('click',x=x+width/2,y=y+height/2,w=w['w'])
                self.get('m',k='scroll',x=x+min(width,300)/2,y=min(window['sz'][1]-80,700),dy=400,w=w['w'])
            time.sleep(.4)
        raise AssertionError('Visible native button not found: '+identifier)

def invite_count(profile,version):
    db=next((profile/'data'/('v'+version)/'native/rinx').glob('*/operations.db'))
    with closing(sqlite3.connect('file:'+db.as_posix()+'?mode=ro',uri=True)) as c:
        return sum(json.loads(row[0])['action']['permission']=='invite' for row in c.execute('select body from operations'))

def run(args):
    h=Hosts(args.executable,args.owner_profile,args.participant_profile,args.version,args.owner_port,args.participant_port,args.activity_id)
    h.visible_roles.add('organizer');native=Native(h.ports['organizer'])
    try:
        h.start('organizer');h.start('participant')
        suite=IntentSuite(h,args.private_trace,args.activity_id);suite.o.authorize();suite.p.authorize()
        native.click('nav_help');native.click('enable_background');time.sleep(1)
        state=suite.call(suite.o,'Refresh');assert state['background'] and not state['background_paused']
        settings={'invitation_minutes':5,'quiet_start':0,'quiet_end':24,'max_invitations':30}
        preview=suite.call(suite.o,'PreviewAutomation',settings)
        suite.call(suite.o,'ConfirmAutomation',{'id':preview['policy_consent_id'],'settings':settings})
        main=next(w for w in native.get('s')['w'] if w['sz'][0]>1000)
        # Makepad's /close route destroys the window without WM_CLOSE on
        # Windows. Post the native close event to this host's own window.
        suite.call(suite.o,'TestTrayAction','close_window');time.sleep(1)
        assert suite.call(suite.o,'Refresh')['tray_hidden'] and h.processes['organizer'].poll() is None
        suite.use(suite.p,suite.card(suite.p,'opportunity'))
        join=suite.operation(suite.call(suite.p,'ConfirmParticipant',suite.preferences),'participant','confirmed')
        invite=suite.automatic_invitation();suite.use(suite.p,suite.card(suite.p,'invitation'));reply=suite.reply(True);cancel=suite.cancel()
        assert suite.call(suite.o,'Refresh')['tray_hidden']
        hidden=suite.evidence([join,invite,reply,cancel]);print('Hidden native window completed actual SDK invitation/reply/cancel',flush=True)
        suite.call(suite.o,'TestTrayAction','pause');time.sleep(1)
        paused=suite.call(suite.o,'Refresh');assert paused['background_paused'] and not paused['authorized']
        before=invite_count(h.profiles['organizer'],h.version);suite.use(suite.p,suite.card(suite.p,'opportunity'))
        queued=suite.operation(suite.call(suite.p,'ConfirmParticipant',suite.preferences),'participant','confirmed')
        time.sleep(12);assert invite_count(h.profiles['organizer'],h.version)==before
        assert not suite.call(suite.o,'Refresh')['authorized']
        # Explicitly resume and reauthorize. Reading the submitted registration
        # must precede a fresh automatic invitation.
        suite.call(suite.o,'TestTrayAction','open');native.click('nav_help');native.click('enable_background')
        suite.o.authorize();preview=suite.call(suite.o,'PreviewAutomation',settings)
        suite.call(suite.o,'ConfirmAutomation',{'id':preview['policy_consent_id'],'settings':settings})
        resumed=suite.automatic_invitation();suite.reply(False)
        resume_evidence=suite.evidence([queued,resumed])
        # Exit can terminate the worker before its result file is written.
        # Process termination is the evidence for this action; do not resend.
        try:suite.o.command('TestTrayAction','exit',timeout=5)
        except TimeoutError:pass
        h.processes['organizer'].wait(timeout=90)
        assert h.processes['organizer'].returncode==0
        after_exit=invite_count(h.profiles['organizer'],h.version);time.sleep(12)
        assert invite_count(h.profiles['organizer'],h.version)==after_exit
        report={'version':h.version,'passed':True,'hidden_native_sdk_flow':hidden,'pause_prevents_new_invites':True,'fresh_consent_required_to_resume':True,'resume_verifies_registration':resume_evidence,'exit_stops_process_and_new_invites':True,'control_method':'WM_CLOSE posted to this host native window; actual SDK effects while hidden; menu actions dispatched through opt-in endpoint using the same menu handler','account_switch_checked':False}
        args.output.parent.mkdir(parents=True,exist_ok=True);args.output.write_text(json.dumps(report,indent=2),'utf8')
    finally:
        for role in ('organizer','participant'):
            if role in h.processes:h.stop(role)

if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('--executable',type=pathlib.Path,required=True);p.add_argument('--owner-profile',type=pathlib.Path,required=True);p.add_argument('--participant-profile',type=pathlib.Path,required=True);p.add_argument('--version',required=True);p.add_argument('--activity-id',required=True);p.add_argument('--owner-port',type=int,default=8160);p.add_argument('--participant-port',type=int,default=8161);p.add_argument('--private-trace',type=pathlib.Path,required=True);p.add_argument('--output',type=pathlib.Path,required=True);run(p.parse_args())
