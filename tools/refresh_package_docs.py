"""Copy verified CI runtime unchanged, then refresh only public docs and assets."""
import argparse,hashlib,json,pathlib,shutil,subprocess
from package_scan import scan
from build_proof import source_fingerprint,digest
ROOT=pathlib.Path(__file__).resolve().parents[1]
def refresh(source,destination):
    assert scan(source)['passed']
    old=json.loads((source/'release.json').read_text('utf8'))
    assert old['version']=='0.1.0' and old['build_proof']['native_source_sha256']==source_fingerprint()
    assert old['dependency_lock_sha256']==digest(ROOT/'dependencies.lock.json')
    if destination.exists():raise RuntimeError('Destination exists; preserved')
    shutil.copytree(source,destination)
    replace_roots={'docs','assets'}
    top={'README.md','README.en.md','CHANGELOG.md','NOTICE.md'}
    for directory in replace_roots:
        # No delete: overlay public files, preserving the measured CI runtime.
        shutil.copytree(ROOT/directory,destination/directory,dirs_exist_ok=True)
    for name in top:shutil.copy2(ROOT/name,destination/name)
    for name,sha in old['sha256'].items():
        if pathlib.PurePosixPath(name).parts[0] not in replace_roots|top:
            assert digest(destination/name)==sha,'Runtime bytes changed'
    old['documentation_source_commit']=subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip()
    old['documentation_refresh']='Only public docs, README, change log, notices and assets; executable, resources, licenses and tools unchanged'
    old['sha256']={p.relative_to(destination).as_posix():digest(p) for p in destination.rglob('*') if p.is_file() and p.name!='release.json'}
    (destination/'release.json').write_text(json.dumps(old,indent=2),'utf8')
    result=scan(destination);assert result['passed'];return result
if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('source',type=pathlib.Path);p.add_argument('destination',type=pathlib.Path)
    a=p.parse_args();print(json.dumps(refresh(a.source,a.destination)))
