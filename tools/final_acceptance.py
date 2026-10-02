"""Start exact-version SDK acceptance and capture selected actual native states.

Uses only opted-in test profiles in the user's existing private room. Screenshots
are read-only RPC renders. Every selected public image needs manual review.
"""
import argparse,json,pathlib,time
from dual_acceptance import Suite
from fault_acceptance import Hosts
from capture_native import capture

class CapturedSuite(Suite):
    def __init__(self,hosts,private,media,font):
        super().__init__(hosts.profiles['organizer'],hosts.profiles['participant'],hosts.version,private)
        self.hosts=hosts;self.media=media;self.font=font;self.frames=[]
    def frame(self,role,label):
        if self.rounds:return
        time.sleep(.8)
        name=f'{len(self.frames)+1:02d}-{role}.png'
        proof=capture(self.hosts.ports[role],self.private.parent/'raw-media',self.media/name,label,self.font)
        self.frames.append({'file':name,'caption':label,**proof})
        (self.media/'captures.json').write_text(json.dumps(self.frames,ensure_ascii=False,indent=2),'utf8')
    def enroll(self):
        op=super().enroll();self.frame('organizer','本人报名后，组织者依据服务端事件加入候补');return op
    def invite(self):
        op=super().invite();self.frame('organizer','名额已保留、邀请已送达；接受状态分别记录')
        self.frame('participant','参与者使用自己的 Rinx 登录接收邀请');return op
    def reply(self,accept):
        op=super().reply(accept);self.frame('organizer','本人回复经组织者核验后，席位才最终确认');return op
    def cancel(self):
        op=super().cancel();self.frame('organizer','本人取消，组织者核验并释放名额');return op

def run(a):
    h=Hosts(a.executable,a.owner_profile,a.participant_profile,a.version,a.owner_port,a.participant_port)
    try:
        h.start('organizer');h.start('participant')
        suite=CapturedSuite(h,a.private_trace,a.media,a.font)
        report=suite.run(5,True)
        a.public_evidence.parent.mkdir(parents=True,exist_ok=True)
        a.public_evidence.write_text(json.dumps(report,indent=2),'utf8')
    finally:
        for role in ('organizer','participant'):
            if role in h.processes:h.stop(role)

if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('--executable',type=pathlib.Path,required=True)
    p.add_argument('--owner-profile',type=pathlib.Path,required=True);p.add_argument('--participant-profile',type=pathlib.Path,required=True)
    p.add_argument('--version',required=True);p.add_argument('--owner-port',type=int,default=8160);p.add_argument('--participant-port',type=int,default=8161)
    p.add_argument('--private-trace',type=pathlib.Path,required=True);p.add_argument('--public-evidence',type=pathlib.Path,required=True)
    p.add_argument('--media',type=pathlib.Path,required=True);p.add_argument('--font',type=pathlib.Path,required=True)
    run(p.parse_args())
