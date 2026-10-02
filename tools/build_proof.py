"""Bind a measured full-host build to its source, dependencies and binary."""
import argparse,hashlib,json,pathlib,subprocess
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
    return {'source_commit':commit,'native_source_sha256':source_fingerprint(),'dependencies_lock_sha256':digest(ROOT/'dependencies.lock.json'),'cargo_lock_sha256':digest(ROOT/'native/Cargo.lock'),'rustc':subprocess.check_output(['rustc','--version'],text=True).strip(),'features':['full-host']}
if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('--start',type=pathlib.Path);p.add_argument('--finish',type=pathlib.Path);p.add_argument('--binary',type=pathlib.Path);p.add_argument('--output',type=pathlib.Path);a=p.parse_args()
    if a.start:a.start.parent.mkdir(parents=True,exist_ok=True);a.start.write_text(json.dumps(snapshot(),indent=2),'utf8')
    else:
        previous=json.loads(a.finish.read_text('utf8'));current=snapshot()
        if previous!=current:raise RuntimeError('Source or toolchain changed while compiling; do not package this binary')
        current['binary_sha256']=digest(a.binary);current['passed']=True;a.output.write_text(json.dumps(current,indent=2),'utf8')
