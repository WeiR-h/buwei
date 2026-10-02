"""Local control for explicitly opted-in acceptance builds, not release builds.

The host checks the logged-in SDK account and room. No login credentials cross
this interface. Results stay in the caller's ignored private profile directory.
"""
import json,pathlib,time,uuid
class Control:
    def __init__(self,profile,account,room):
        self.profile=pathlib.Path(profile);self.directory=self.profile/'.run/acceptance';self.directory.mkdir(parents=True,exist_ok=True)
        self.nonce=uuid.uuid4().hex
        self.config={'account':account,'room':room,'nonce':self.nonce,'expires_at':int(time.time())+3500}
        self.write('config.local.json',self.config)
    def write(self,name,value):
        temporary=self.directory/(name+'.tmp');temporary.write_text(json.dumps(value,ensure_ascii=False),'utf8');temporary.replace(self.directory/name)
    def command(self,name,value=None,timeout=55):
        request=uuid.uuid4().hex;action={'command':name}
        if value is not None:action['value']=value
        self.write('request.local.json',{'id':request,'nonce':self.nonce,'action':action})
        path=self.directory/(request+'.result.json');deadline=time.monotonic()+timeout
        while time.monotonic()<deadline:
            if path.exists():
                try:result=json.loads(path.read_text('utf8'))
                except json.JSONDecodeError:time.sleep(.1);continue
                if result['account']!=self.config['account'] or result['request_id']!=request:raise RuntimeError('Host identity or request mismatch')
                return result
            time.sleep(.2)
        raise TimeoutError('Host result unavailable; request ID retained. Do not resend blindly: '+request)
    def authorize(self):
        preview=self.command('Authorize');nonce=preview.get('consent_id')
        if not nonce:raise RuntimeError('Host consent preview unavailable')
        result=self.command('ConfirmAuthorization',nonce)
        if not result.get('authorized'):raise RuntimeError('Host did not grant consent')
        return result
    def pause_automatic_sync(self,paused):
        self.config['pause_automatic_sync']=bool(paused);self.write('config.local.json',self.config)
