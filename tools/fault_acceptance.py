"""Actual SDK acknowledgement-loss and crash recovery on isolated test profiles.

Requires an explicit acceptance executable and two stopped, migrated profiles.
No keys or identities are accepted as arguments. It reuses verified SDK binding.
"""
import argparse,json,os,pathlib,sqlite3,subprocess,time,urllib.request
from contextlib import closing
from dual_acceptance import Suite
from formal_control import Control

class Hosts:
    def __init__(self,exe,owner,participant,version,owner_port,participant_port):
        self.exe=exe.resolve();self.profiles={'organizer':owner.resolve(),'participant':participant.resolve()};self.ports={'organizer':owner_port,'participant':participant_port};self.version=version;self.processes={};self.logs=[];self.extra_environment={}
    def start(self,role):
        profile=self.profiles[role];env=dict(os.environ);env.update(self.extra_environment.get(role,{}));env['BUWEI_PROFILE']=role;env['MAKEPAD_HIDE_WINDOWS']='1'
        log=open(profile/('fault-host-'+str(time.time_ns())+'.private.log'),'wb');self.logs.append(log)
        startup=subprocess.STARTUPINFO();startup.dwFlags|=subprocess.STARTF_USESHOWWINDOW;startup.wShowWindow=0
        process=subprocess.Popen([str(self.exe),str(profile),'--gui','--official-rinx','--acceptance','--remote='+str(self.ports[role])],cwd=self.exe.parent,env=env,stdout=log,stderr=subprocess.STDOUT,startupinfo=startup);self.processes[role]=process
        deadline=time.monotonic()+100
        while time.monotonic()<deadline:
            if process.poll() is not None:raise RuntimeError('Acceptance host exited during login restore')
            try:
                status=json.loads((profile/'data'/('v'+self.version)/'rinx-binding-status.json').read_text('utf8'))
                if status['process_id']==process.pid and status['server_identity_verified'] and not status['action_authorized']:return
            except (OSError,ValueError):pass
            time.sleep(.3)
        raise TimeoutError('SDK login binding did not restore; preserved profile')
    def stop(self,role):
        process=self.processes.get(role)
        if process and process.poll() is not None:return
        opener=urllib.request.build_opener(urllib.request.ProxyHandler({}))
        try:opener.open('http://127.0.0.1:'+str(self.ports[role])+'/quit',timeout=8).read()
        except OSError:pass
        # The upstream RPC replies after four seconds, but its queued Quit can
        # still complete later during software rendering. Wait for this exact
        # child without resending or killing a signed-in process.
        if process:
            try:process.wait(timeout=90)
            except subprocess.TimeoutExpired:raise RuntimeError('Own host did not finish queued shutdown; profile preserved')
            if process.returncode!=0:raise RuntimeError('Own host shutdown failed; profile preserved')
    def restart(self,role):self.stop(role);self.start(role)

class FaultSuite(Suite):
    def __init__(self,hosts,private):
        super().__init__(hosts.profiles['organizer'],hosts.profiles['participant'],hosts.version,private);self.hosts=hosts;self.cases=[]
    def restart_control(self,role):
        self.hosts.restart(role);profile=self.owner if role=='organizer' else self.participant;account=self.accounts[0] if role=='organizer' else self.accounts[1]
        control=Control(profile,account,self.room);control.authorize()
        if role=='organizer':self.o=control
        else:self.p=control
        return control
    def persisted_operation(self,role,operation_id):
        profile=self.owner if role=='organizer' else self.participant
        db=next((profile/'data'/('v'+self.version)/'native/rinx').glob('*/operations.db'))
        with closing(sqlite3.connect('file:'+db.as_posix()+'?mode=ro',uri=True)) as connection:
            row=connection.execute('select status,body from operations where id=?',(operation_id,)).fetchone()
        if row is None:raise AssertionError('Operation missing from durable journal')
        body=json.loads(row[1]);assert row[0]==body['status'] and body['id']==operation_id
        return body
    def fault_dispatch(self,role,mode,name,value,key,operation):
        control=self.o if role=='organizer' else self.p;self.call(control,'TestFault',mode)
        before='unknown'
        if mode=='crash_before_receipt':
            try:self.call(control,name,value)
            except TimeoutError:
                process=self.hosts.processes[role]
                if process.poll() is None:raise AssertionError('Expected native process crash did not occur')
            else:raise AssertionError('Crash fault returned without terminating the process')
            before='dispatching'
        else:
            uncertain=self.operation(self.call(control,name,value),key,'unknown');assert uncertain['id']==operation['id']
        persisted=self.persisted_operation(role,operation['id']);assert persisted['status']==before
        control=self.restart_control(role);recovered=self.operation(self.call(control,'ReconcilePending'),key,'confirmed');assert recovered['id']==operation['id']
        proof=self.evidence([recovered]);record={'scene':key,'fault':mode,'persisted_status_before_recovery':before,'status_after_recovery':'confirmed','original_operation_retained':True,'native_process_restarted':True,'blind_resends':0,**proof};self.cases.append(record)
        (self.private.parent/'completed-cases.private.json').write_text(json.dumps(self.cases,indent=2),'utf8')
        print(key+' '+mode+' recovered once',flush=True);return recovered
    def run_faults(self):
        self.o.authorize();self.p.authorize()
        for mode in ['lost_ack','crash_before_receipt']:
            self.enroll();op=self.operation(self.call(self.o,'Prepare'),'invitation','prepared')
            recovered=self.fault_dispatch('organizer',mode,'Execute',None,'invitation',op)
            self.wait(self.participant,'participant read recovered invitation',lambda a:any(i['operation_id']==recovered['id'] and i['delivery']=='delivered' for i in a['invitations']))
            self.reply(False)
        for mode in ['lost_ack','crash_before_receipt']:
            self.enroll();invite=self.invite();op=self.operation(self.call(self.p,'Accept',True),'participant','prepared');assert op['id']!=invite['id']
            self.fault_dispatch('participant',mode,'ConfirmParticipant',self.preferences,'participant',op)
            self.wait(self.owner,'organizer accepted recovered participant reply',lambda a:self.person(a).get('status')=='confirmed')
            self.wait(self.participant,'participant read recovered final acceptance',lambda a:self.person(a).get('status')=='confirmed')
            self.cancel()
        for mode in ['lost_ack','crash_before_receipt']:
            draft={'title':'补位回执恢复验收 '+mode,'markdown':'这是现有私有双账号测试房间中的文章恢复验收。\n\n服务端已收到消息时，沿原操作编号核实回执，重复确认不得再次发布。\n\n本测试正文不包含姓名、账号、房间编号或外部链接。'}
            self.call(self.o,'NewArticle',draft);op=self.operation(self.call(self.o,'PrepareArticle'),'article','prepared');assert op['action']['payload']['title']==draft['title'] and op['action']['payload']['markdown']==draft['markdown']
            self.fault_dispatch('organizer',mode,'PublishArticle',draft,'article',op)
        return {'version':self.version,'passed':True,'scope':'actual SDK sends and service event verification; deliberate receipt loss or native abort in opt-in acceptance build; network cable was not disconnected','cases':self.cases,'all_cases_restart_native_process':True}

if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('--executable',type=pathlib.Path,required=True);p.add_argument('--owner-profile',type=pathlib.Path,required=True);p.add_argument('--participant-profile',type=pathlib.Path,required=True);p.add_argument('--version',required=True);p.add_argument('--owner-port',type=int,default=8140);p.add_argument('--participant-port',type=int,default=8141);p.add_argument('--private-trace',type=pathlib.Path,required=True);p.add_argument('--public-evidence',type=pathlib.Path,required=True);a=p.parse_args()
    hosts=Hosts(a.executable,a.owner_profile,a.participant_profile,a.version,a.owner_port,a.participant_port)
    hosts.start('organizer');hosts.start('participant');suite=FaultSuite(hosts,a.private_trace)
    try:
        report=suite.run_faults();a.public_evidence.parent.mkdir(parents=True,exist_ok=True);a.public_evidence.write_text(json.dumps(report,indent=2),'utf8')
    finally:
        hosts.stop('organizer');hosts.stop('participant')
