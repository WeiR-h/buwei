"""Acquire the reviewed official runtime at fixed commits, without credentials."""
import argparse, hashlib, json, pathlib, subprocess, sys, os, tempfile
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
    os.environ['GIT_CONFIG_KEY_'+str(count+1)]='core.eol'
    os.environ['GIT_CONFIG_VALUE_'+str(count+1)]='lf'
    os.environ['GIT_CONFIG_COUNT']=str(count+2)
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
    expected=json.loads((ROOT/'dependencies.lock.json').read_text('utf8'))
    framework=host/'.sources/makepad';patch=ROOT/'patches/makepad-unicode-smallvec.patch'
    product_patches=[{'path':'patches/makepad-unicode-smallvec.patch','sha256':expected['product_patch_sha256'],'changed_files':['libs/unicode/unicode-bidi/Cargo.toml']},*expected.get('additional_product_patches',[])]
    for entry in product_patches:
        if hashlib.sha256((ROOT/entry['path']).read_bytes()).hexdigest()!=entry['sha256']:raise RuntimeError('Product patch hash mismatch')
    applied=framework.exists() and subprocess.run(['git','-C',str(framework),'apply','--reverse','--check',str(patch)],capture_output=True).returncode==0
    if not applied:
        call(*setup,cwd=host)
        call(*setup,'--check',cwd=host)
    for entry in product_patches:
        file=ROOT/entry['path']
        if subprocess.run(['git','-C',str(framework),'apply','--reverse','--check',str(file)],capture_output=True).returncode:
            call('git','-C',str(framework),'apply','--check',str(file))
            call('git','-C',str(framework),'apply',str(file))
    for name,revision in expected['frameworks'].items():
        source=host/'.sources'/name
        if git(source,'rev-parse','HEAD')!=revision:raise RuntimeError('Framework revision mismatch: '+name)
        if git(source,'ls-files','--others','--exclude-standard'):raise RuntimeError('Unreviewed dependency files: '+name)
        if name=='makepad':
            overlay=json.loads((host/'runtime-patches.lock.json').read_text('utf8'))['makepad']
            if git(source,'write-tree')!=overlay['tree']:raise RuntimeError('Official overlay tree mismatch')
            changed=git(source,'diff','--name-only').splitlines()
            paths=sorted({p for entry in product_patches for p in entry['changed_files']})
            if changed!=paths:raise RuntimeError('Unreviewed Makepad modifications')
            private=ROOT/'.run';private.mkdir(exist_ok=True)
            with tempfile.TemporaryDirectory(dir=private,prefix='verify-product-') as folder:
                review=pathlib.Path(folder).resolve()
                if not review.is_relative_to(private.resolve()):raise RuntimeError('Review directory outside workspace')
                call('git','init','-q',str(review))
                for path in paths:
                    target=review/path;target.parent.mkdir(parents=True,exist_ok=True)
                    target.write_bytes(subprocess.check_output(['git','-C',str(source),'show',':'+path]))
                for entry in product_patches:call('git','-C',str(review),'apply',str(ROOT/entry['path']))
                for path in paths:
                    if (source/path).read_bytes().replace(b'\r\n',b'\n')!=(review/path).read_bytes().replace(b'\r\n',b'\n'):raise RuntimeError('Product patch differs: '+path)
            for entry in [overlay,*overlay.get('stacked',[])]:
                if hashlib.sha256((host/entry['patch']).read_bytes()).hexdigest()!=entry['sha256']:raise RuntimeError('Official overlay hash differs')
        elif git(source,'status','--porcelain'):raise RuntimeError('Modified framework preserved: '+name)
    rinx_expected=expected.get('rinx')
    if rinx_expected:
        rinx=ROOT/'.deps/rinx';rinx_patch=ROOT/rinx_expected['patch']
        if hashlib.sha256(rinx_patch.read_bytes()).hexdigest()!=rinx_expected['patch_sha256']:raise RuntimeError('Rinx patch hash mismatch')
        if not rinx.exists():
            call('git','clone','--filter=blob:none','--no-checkout','https://github.com/hagency-org/Rinx.git',str(rinx))
            call('git','-C',str(rinx),'-c','core.longpaths=true','checkout','--detach',rinx_expected['commit'])
        if git(rinx,'rev-parse','HEAD')!=rinx_expected['commit']:raise RuntimeError('Rinx revision mismatch')
        if subprocess.run(['git','-C',str(rinx),'apply','--reverse','--check',str(rinx_patch)],capture_output=True).returncode:
            call('git','-C',str(rinx),'apply','--check',str(rinx_patch));call('git','-C',str(rinx),'apply',str(rinx_patch))
        paths=['Cargo.toml','build.rs','crates/article-core/Cargo.toml','crates/article-makepad/Cargo.toml','src/event_preview.rs','src/home/room_screen.rs','src/mini_app.rs']
        if git(rinx,'diff','--name-only').splitlines()!=paths or git(rinx,'ls-files','--others','--exclude-standard'):raise RuntimeError('Unreviewed Rinx modification')
        private=ROOT/'.run';private.mkdir(exist_ok=True)
        with tempfile.TemporaryDirectory(dir=private,prefix='verify-rinx-') as folder:
            review=pathlib.Path(folder).resolve()
            if not review.is_relative_to(private.resolve()):raise RuntimeError('Rinx review directory outside workspace')
            call('git','init','-q',str(review))
            for path in paths:
                target=review/path;target.parent.mkdir(parents=True,exist_ok=True)
                target.write_bytes(subprocess.check_output(['git','-C',str(rinx),'show','HEAD:'+path]))
            call('git','-C',str(review),'apply',str(rinx_patch))
            for path in paths:
                if (rinx/path).read_bytes().replace(b'\r\n',b'\n')!=(review/path).read_bytes().replace(b'\r\n',b'\n'):raise RuntimeError('Rinx product patch differs: '+path)
    digest=hashlib.sha256(patch.read_bytes()).hexdigest()
    if digest!=expected['product_patch_sha256']:raise RuntimeError('Product patch hash mismatch')
    print(json.dumps({'official_commit':PIN,'official_reviewed_overlay':True,'product_patch_sha256':digest,'additional_product_patches':expected.get('additional_product_patches',[]),'credentials_created':False}))
if __name__=='__main__':main()
