"""Start an actual packaged native host on a fresh profile and system-only PATH."""
import argparse,hashlib,json,os,pathlib,re,socket,subprocess,tempfile,time,urllib.request

def inspect_log(content):
    # The pinned upstream guard deliberately defers an isolated VM call. Keep
    # this diagnostic visible in the report; never allow other error lines.
    errors=[line for line in content.splitlines() if '[E]' in line]
    guard=re.compile(r'BUG: update_global_ui_handle while isolate SplashVmId\([1-9][0-9]*\) is installed; deferred$')
    unexpected=[line for line in errors if not guard.search(line) or 'widget_async.rs:787:9' not in line]
    if unexpected:raise AssertionError('Unexpected native error; see private startup log')
    for marker in ['Failed to load resource','on_render closure failed','instruction limit exceeded']:
        if marker in content:raise AssertionError(marker)
    return [{'upstream_file':'makepad/widgets/src/widget_async.rs:787','diagnostic':'isolated VM global UI update safely deferred','count':len(errors),'behavior':'upstream guard deferred isolated VM update; native render and shutdown checked'}] if errors else []
def check(package,output):
    package=package.resolve();output.mkdir(parents=True,exist_ok=True)
    release=json.loads((package/'release.json').read_text('utf8'));exe=package/'native/buwei-rinx-dual-host.exe'
    digest=hashlib.sha256(exe.read_bytes()).hexdigest();assert digest==release['native_binary_sha256']
    with socket.socket() as s:s.bind(('127.0.0.1',0));port=s.getsockname()[1]
    env=dict(os.environ);system=pathlib.Path(env.get('SystemRoot','C:/Windows'))
    for key in ['CARGO_HOME','RUSTUP_HOME','CARGO_MANIFEST_DIR','RINX_DATA_DIR','OCTOSENSE_HOME','OCTOS_APP_CORE_DIR','MAKEPAD_REMOTE','MAKEPAD_FOCUS']:env.pop(key,None)
    env['PATH']=';'.join(str(p) for p in [system/'System32',system,system/'System32/WindowsPowerShell/v1.0']);env['MAKEPAD_HIDE_WINDOWS']='1'
    opener=urllib.request.build_opener(urllib.request.ProxyHandler({}))
    def get(route):return opener.open(f'http://127.0.0.1:{port}/{route}',timeout=5).read()
    report={'version':release['version'],'passed':False,'binary_sha256':digest,'fresh_profile':True,'system_only_path':True,'model_calls':0,'actual_native_render':False}
    with tempfile.TemporaryDirectory(prefix='buwei-clean-') as folder,open(output/'startup.private.log','wb') as log:
        command=[str(exe),folder,'--gui','--official-rinx',f'--remote={port}'];process=subprocess.Popen(command,cwd=exe.parent,env=env,stdout=log,stderr=subprocess.STDOUT)
        try:
            start=time.monotonic()
            while time.monotonic()-start<60:
                if process.poll() is not None:raise RuntimeError('Native host exited before rendering')
                try:
                    snap=json.loads(get('snap?all=1'));labels='\n'.join(w.get('t','') for w in snap['s'] if w['ty']=='Label')
                    if '补位' in labels and 'v'+release['version'] in labels and '未授权' in labels:break
                except (OSError,ValueError):pass
                time.sleep(.2)
            else:raise TimeoutError('Native startup/render timed out')
            binding=json.loads((pathlib.Path(folder)/'data'/('v'+release['version'])/'rinx-binding-status.json').read_text('utf8'))
            assert not binding['server_identity_verified'] and not binding['action_authorized']
            report['unauthenticated_and_unauthorized']=True;report['actual_native_render']=True
            second=subprocess.run(command,cwd=exe.parent,env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE,timeout=10)
            assert second.returncode!=0;report['second_writer_refused']=True
            (output/'startup.png').write_bytes(get('g?raw=1'))
            get('quit');assert process.wait(timeout=25)==0
            log.flush();content=(output/'startup.private.log').read_text('utf8',errors='replace')
            report['known_upstream_diagnostics']=inspect_log(content)
            report['passed']=True
        finally:
            if process.poll() is None:
                try:get('quit');process.wait(timeout=15)
                except (OSError,subprocess.TimeoutExpired):process.terminate();process.wait(timeout=10)
            (output/'startup.json').write_text(json.dumps(report,indent=2),'utf8')
    return report
if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('package',type=pathlib.Path);p.add_argument('--output',type=pathlib.Path,required=True);a=p.parse_args();print(json.dumps(check(a.package,a.output)))
