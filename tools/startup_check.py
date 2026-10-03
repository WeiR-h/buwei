"""Start an actual packaged native host on a fresh profile and system-only PATH."""
import argparse,hashlib,json,os,pathlib,re,socket,struct,subprocess,tempfile,time,urllib.request,zlib

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
def rendered_window(snapshot,status,version):
    """Select the visible BuWei surface, rather than the shell's first window."""
    for window in status.get('w',[]):
        width,height=window.get('sz',[0,0]);labels=[]
        for widget in snapshot.get('s',[]):
            x,y,w,h=widget.get('r',[0,0,0,0])
            if widget.get('w')!=window['i'] or widget.get('ty')!='Label' or widget.get('v',1)==0:continue
            if w<=0 or h<=0 or x>=width or y>=height or x+w<=0 or y+h<=0:continue
            labels.append(widget.get('t',''))
        text='\n'.join(labels)
        if '补位' in text and 'v'+version in text and '未授权' in text:return window['i']
    return None
def frame_visibility(raw):
    """Check actual light-theme body pixels, excluding the shell's top menu."""
    if not raw.startswith(b'\x89PNG\r\n\x1a\n'):raise ValueError('Native capture is not PNG')
    offset=8;compressed=[];header=None
    while offset+12<=len(raw):
        size=struct.unpack_from('>I',raw,offset)[0];kind=raw[offset+4:offset+8];body=raw[offset+8:offset+8+size]
        if len(body)!=size:raise ValueError('Incomplete native PNG')
        if kind==b'IHDR':header=struct.unpack('>IIBBBBB',body)
        elif kind==b'IDAT':compressed.append(body)
        offset+=size+12
        if kind==b'IEND':break
    if header is None:raise ValueError('Native PNG header missing')
    width,height,depth,color,compression,filtering,interlace=header
    if depth!=8 or color not in (2,6) or compression or filtering or interlace or not 0<width*height<=16_000_000:raise ValueError('Unexpected native PNG format')
    channels=4 if color==6 else 3;stride=width*channels;decoder=zlib.decompressobj()
    pixels=decoder.decompress(b''.join(compressed),(stride+1)*height+1)
    if len(pixels)!=(stride+1)*height or not decoder.eof:raise ValueError('Incomplete native PNG pixels')
    previous=bytearray(stride);bright=samples=0
    def paeth(a,b,c):
        p=a+b-c;da,db,dc=abs(p-a),abs(p-b),abs(p-c)
        return a if da<=db and da<=dc else b if db<=dc else c
    for y in range(height):
        start=y*(stride+1);method=pixels[start];row=bytearray(pixels[start+1:start+1+stride])
        if method not in range(5):raise ValueError('Invalid native PNG row filter')
        if method:
            for x in range(stride):
                left=row[x-channels] if x>=channels else 0;up=previous[x];corner=previous[x-channels] if x>=channels else 0
                predictor=left if method==1 else up if method==2 else (left+up)//2 if method==3 else paeth(left,up,corner)
                row[x]=(row[x]+predictor)&255
        if y>height//10 and y%8==0:
            for x in range(0,width,8):
                i=x*channels;samples+=1;bright+=int(sum(row[i:i+3])>=300)
        previous=row
    fraction=bright/max(samples,1)
    return {'width':width,'height':height,'body_visible_fraction':round(fraction,4),'visible':fraction>=0.03}
def check(package,output):
    package=package.resolve();output.mkdir(parents=True,exist_ok=True)
    release=json.loads((package/'release.json').read_text('utf8'));exe=package/'native/buwei-rinx-dual-host.exe'
    digest=hashlib.sha256(exe.read_bytes()).hexdigest();assert digest==release['native_binary_sha256']
    with socket.socket() as s:s.bind(('127.0.0.1',0));port=s.getsockname()[1]
    env=dict(os.environ);system=pathlib.Path(env.get('SystemRoot','C:/Windows'))
    for key in ['CARGO_HOME','RUSTUP_HOME','CARGO_MANIFEST_DIR','RINX_DATA_DIR','OCTOSENSE_HOME','OCTOS_APP_CORE_DIR','MAKEPAD_REMOTE','MAKEPAD_FOCUS']:env.pop(key,None)
    env['PATH']=';'.join(str(p) for p in [system/'System32',system,system/'System32/WindowsPowerShell/v1.0']);env['MAKEPAD_HIDE_WINDOWS']='1'
    opener=urllib.request.build_opener(urllib.request.ProxyHandler({}))
    def get(route):return opener.open(f'http://127.0.0.1:{port}/{route}',timeout=15 if route.startswith('g?') else 5).read()
    report={'version':release['version'],'passed':False,'binary_sha256':digest,'fresh_profile':True,'system_only_path':True,'model_calls':0,'actual_native_render':False}
    with tempfile.TemporaryDirectory(prefix='buwei-clean-') as folder,open(output/'startup.private.log','wb') as log:
        command=[str(exe),folder,'--gui','--official-rinx',f'--remote={port}'];process=subprocess.Popen(command,cwd=exe.parent,env=env,stdout=log,stderr=subprocess.STDOUT)
        try:
            start=time.monotonic()
            while time.monotonic()-start<60:
                if process.poll() is not None:
                    log.flush();content=(output/'startup.private.log').read_text('utf8',errors='replace')
                    report['early_exit_code']=process.returncode
                    report['early_exit_hex']=hex(process.returncode & 0xffffffff)
                    # Fresh unauthenticated profile: print only bounded diagnostics,
                    # masking ephemeral Windows user paths. Never used on signed-in profiles.
                    tail='\n'.join(content.splitlines()[-18:])
                    tail=re.sub(r'[A-Z]:[\\/]Users[\\/][^\s\"]+', '<fresh-user-path>',tail,flags=re.I)
                    print('Fresh native startup diagnostic:',report['early_exit_hex'],tail,flush=True)
                    raise RuntimeError('Native host exited before rendering: '+report['early_exit_hex'])
                try:
                    snap=json.loads(get('snap?all=1'));status=json.loads(get('s'))
                    window=rendered_window(snap,status,release['version'])
                    if window is not None:break
                except (OSError,ValueError):pass
                time.sleep(.2)
            else:raise TimeoutError('Native startup/render timed out')
            binding=json.loads((pathlib.Path(folder)/'data'/('v'+release['version'])/'rinx-binding-status.json').read_text('utf8'))
            assert not binding['server_identity_verified'] and not binding['action_authorized']
            report['unauthenticated_and_unauthorized']=True;report['captured_window']=window
            second=subprocess.run(command,cwd=exe.parent,env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE,timeout=10)
            assert second.returncode!=0;report['second_writer_refused']=True
            deadline=time.monotonic()+60
            while time.monotonic()<deadline:
                raw=get('g?w='+str(window)+'&raw=1');visibility=frame_visibility(raw)
                (output/'startup.png').write_bytes(raw);report['captured_frame']=visibility
                if visibility['visible']:break
                time.sleep(.5)
            else:raise TimeoutError('Native UI labels exist but the actual frame has no visible application body')
            report['actual_native_render']=True
            get('quit');assert process.wait(timeout=25)==0
            log.flush();content=(output/'startup.private.log').read_text('utf8',errors='replace')
            report['renderer']='Windows WARP software rendering' if 'using Windows WARP software rendering' in content else 'hardware D3D11'
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
