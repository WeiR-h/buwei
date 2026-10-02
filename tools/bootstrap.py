"""Acquire the reviewed official runtime at fixed commits, without credentials."""
import argparse, hashlib, json, pathlib, subprocess, sys, os
ROOT=pathlib.Path(__file__).resolve().parents[1]
PIN='ad0d738bd1c10b735af6b34e11f5826623b6a73b'
URL='https://github.com/OctoSense-org/OctoSense.git'

def call(*args,cwd=None):
    subprocess.run(list(args),cwd=cwd,check=True)
def git(path,*args):
    return subprocess.check_output(['git','-C',str(path),*args],text=True).strip()
def main():
    # Windows Git defaults may convert reviewed patch bytes to CRLF. Override
    # only this process and its children; do not change the user's Git settings.
    count=int(os.environ.get('GIT_CONFIG_COUNT','0'))
    os.environ['GIT_CONFIG_KEY_'+str(count)]='core.autocrlf'
    os.environ['GIT_CONFIG_VALUE_'+str(count)]='false'
    os.environ['GIT_CONFIG_COUNT']=str(count+1)
    p=argparse.ArgumentParser();p.add_argument('--source-cache',type=pathlib.Path);args=p.parse_args()
    host=ROOT/'.deps/octosense';host.parent.mkdir(exist_ok=True)
    if host.exists():
        if not (host/'.git').exists():raise RuntimeError('Existing dependency snapshot preserved. A clean checkout is required for reproducible bootstrap.')
        if git(host,'rev-parse','HEAD')!=PIN:raise RuntimeError('Existing dependency revision differs; preserved.')
    else:
        cache=args.source_cache/'OctoSense' if args.source_cache else None
        if cache and cache.exists():call('git','clone','--shared','--no-checkout',str(cache),str(host))
        else:call('git','clone','--filter=blob:none','--no-checkout',URL,str(host))
        call('git','-C',str(host),'-c','core.longpaths=true','checkout','--detach',PIN)
    setup=[sys.executable,'-X','utf8',str(host/'tools/setup.py')]
    setup+=['--hub',str(args.source_cache.resolve())] if args.source_cache else ['--no-hub']
    # Verify the official overlay before applying the explicitly listed product patch.
    framework=host/'.sources/makepad';patch=ROOT/'patches/makepad-unicode-smallvec.patch'
    applied=framework.exists() and subprocess.run(['git','-C',str(framework),'apply','--reverse','--check',str(patch)],capture_output=True).returncode==0
    if not applied:
        call(*setup,cwd=host)
        call(*setup,'--check',cwd=host)
        call('git','-C',str(framework),'apply','--check',str(patch))
        call('git','-C',str(framework),'apply',str(patch))
    expected=json.loads((ROOT/'dependencies.lock.json').read_text('utf8'))
    for name,revision in expected['frameworks'].items():
        source=host/'.sources'/name
        if git(source,'rev-parse','HEAD')!=revision:raise RuntimeError('Framework revision mismatch: '+name)
        if git(source,'ls-files','--others','--exclude-standard'):raise RuntimeError('Unreviewed dependency files: '+name)
        if name=='makepad':
            overlay=json.loads((host/'runtime-patches.lock.json').read_text('utf8'))['makepad']
            if git(source,'write-tree')!=overlay['tree']:raise RuntimeError('Official overlay tree mismatch')
            changed=git(source,'diff','--name-only').splitlines()
            changed_path='libs/unicode/unicode-bidi/Cargo.toml'
            if changed!=[changed_path]:raise RuntimeError('Unreviewed Makepad modifications')
            original=subprocess.check_output(['git','-C',str(source),'show',':'+changed_path])
            wanted=original.replace(b'path = "../smallvec"',b'path = "../../smallvec"')
            if (source/changed_path).read_bytes()!=wanted:raise RuntimeError('Product path patch differs')
            for entry in [overlay,*overlay.get('stacked',[])]:
                if hashlib.sha256((host/entry['patch']).read_bytes()).hexdigest()!=entry['sha256']:raise RuntimeError('Official overlay hash differs')
        elif git(source,'status','--porcelain'):raise RuntimeError('Modified framework preserved: '+name)
    digest=hashlib.sha256(patch.read_bytes()).hexdigest()
    if digest!=expected['product_patch_sha256']:raise RuntimeError('Product patch hash mismatch')
    print(json.dumps({'official_commit':PIN,'official_reviewed_overlay':True,'product_patch_sha256':digest,'credentials_created':False}))
if __name__=='__main__':main()
