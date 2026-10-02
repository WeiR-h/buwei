"""Bind a measured full-host build to its source, dependencies and binary."""
import argparse,hashlib,json,pathlib,subprocess,struct,os
ROOT=pathlib.Path(__file__).resolve().parents[1]
def digest(file):return hashlib.sha256(file.read_bytes()).hexdigest()
def source_fingerprint():
    files={}
    for file in (ROOT/'native').rglob('*'):
        if not file.is_file() or {'target','.run','__pycache__'}&set(file.relative_to(ROOT/'native').parts):continue
        content=file.read_bytes()
        if file.suffix in {'.rs','.toml','.md','.txt','.svg'} or file.name=='Cargo.lock':content=content.replace(b'\r\n',b'\n')
        files[file.relative_to(ROOT).as_posix()]=hashlib.sha256(content).hexdigest()
    return hashlib.sha256(json.dumps(files,sort_keys=True,separators=(',',':')).encode()).hexdigest()
def snapshot():
    try:commit=subprocess.check_output(['git','-C',str(ROOT),'rev-parse','HEAD'],text=True).strip()
    except subprocess.CalledProcessError:commit=None
    rustc=subprocess.check_output(['rustc','--version'],text=True).strip()
    host=subprocess.check_output(['rustc','-vV'],text=True)
    compiler={}
    if 'host: x86_64-pc-windows-gnu' in host:
        compiler['gcc']=subprocess.check_output(['gcc','-dumpfullversion','-dumpversion'],text=True).strip()
        compiler['binutils']=subprocess.check_output(['ld','--version'],text=True).splitlines()[0]
    elif 'host: x86_64-pc-windows-msvc' in host:
        vswhere=pathlib.Path(os.environ.get('ProgramFiles(x86)','C:/Program Files (x86)'))/'Microsoft Visual Studio/Installer/vswhere.exe'
        args=[str(vswhere),'-latest','-products','*','-requires','Microsoft.VisualStudio.Component.VC.Tools.x86.x64']
        compiler['visual_studio']=subprocess.check_output(args+['-property','installationVersion'],text=True).strip()
        install=pathlib.Path(subprocess.check_output(args+['-property','installationPath'],text=True).strip())
        compiler['msvc_tools']=(install/'VC/Auxiliary/Build/Microsoft.VCToolsVersion.default.txt').read_text('utf8').strip()
    return {'source_commit':commit,'native_source_sha256':source_fingerprint(),'dependencies_lock_sha256':digest(ROOT/'dependencies.lock.json'),'cargo_lock_sha256':digest(ROOT/'native/Cargo.lock'),'rustc':rustc,'rust_host':host.split('host: ')[1].splitlines()[0],'compiler':compiler,'features':['full-host']}
if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('--start',type=pathlib.Path);p.add_argument('--finish',type=pathlib.Path);p.add_argument('--binary',type=pathlib.Path);p.add_argument('--output',type=pathlib.Path);a=p.parse_args()
    if a.start:a.start.parent.mkdir(parents=True,exist_ok=True);a.start.write_text(json.dumps(snapshot(),indent=2),'utf8')
    else:
        previous=json.loads(a.finish.read_text('utf8'));current=snapshot()
        if previous!=current:raise RuntimeError('Source or toolchain changed while compiling; do not package this binary')
        content=a.binary.read_bytes();offset=struct.unpack_from('<I',content,60)[0]+24
        current['stack_reserve_bytes']=struct.unpack_from('<Q',content,offset+72)[0]
        if current['stack_reserve_bytes']!=16*1024*1024:raise RuntimeError('Native main-thread stack reserve was not normalized')
        current['binary_sha256']=digest(a.binary);current['passed']=True;a.output.write_text(json.dumps(current,indent=2),'utf8')
