"""Prepare immutable reviewable attachments; this script never publishes."""
import argparse,hashlib,json,pathlib,shutil,subprocess,zipfile
from package_scan import scan
from public_scan import scan as scan_source
from export_contribution import export
from build_proof import source_fingerprint,digest
ROOT=pathlib.Path(__file__).resolve().parents[1]

def archive_tree(source,destination):
    with zipfile.ZipFile(destination,'w',zipfile.ZIP_DEFLATED,compresslevel=6) as z:
        for file in sorted(source.rglob('*')):
            if file.is_file():z.write(file,file.relative_to(source).as_posix())

def stage(a):
    assert scan_source()['passed'] and scan(a.package)['passed']
    release=json.loads((a.package/'release.json').read_text('utf8'));assert release['version']=='0.1.0'
    if release['build_proof']['native_source_sha256']!=source_fingerprint():
        raise RuntimeError('Package native source does not match the release source')
    if release['dependency_lock_sha256']!=digest(ROOT/'dependencies.lock.json'):
        raise RuntimeError('Package fixed dependencies do not match the release source')
    status=subprocess.check_output(['git','status','--porcelain','--untracked-files=no'],cwd=ROOT,text=True).strip()
    if status:raise RuntimeError('Tracked source has uncommitted changes')
    commit=subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip()
    video=json.loads((a.video_report).read_text('utf8'));assert video['passed'] and video['version']=='0.1.0'
    assert hashlib.sha256(a.video.read_bytes()).hexdigest()==video['video_sha256']
    a.destination.mkdir(parents=True,exist_ok=False)
    archive_tree(a.package,a.destination/'BuWei-v0.1.0-windows-x64.zip')
    subprocess.run(['git','archive','--format=zip','--prefix=buwei-v0.1.0/','--output='+str((a.destination/'BuWei-v0.1.0-source.zip').resolve()),commit],cwd=ROOT,check=True)
    export(a.destination/'contribution')
    archive_tree(a.destination/'contribution',a.destination/'BuWei-v0.1.0-contribution.zip')
    archive_tree(ROOT/'evidence',a.destination/'BuWei-v0.1.0-evidence.zip')
    shutil.copy2(a.video,a.destination/'BuWei-v0.1.0-demo.mp4')
    for name in ['icon.svg','icon.png','buwei.ico']:shutil.copy2(ROOT/'assets'/name,a.destination/name)
    proof={'version':'0.1.0','source_commit':commit,'binary_build_source_commit':release['build_proof']['source_commit'],'native_binary_sha256':release['native_binary_sha256'],'native_source_sha256':release['build_proof']['native_source_sha256'],'dependency_lock_sha256':release['dependency_lock_sha256'],'stable_publication_requires_release_gate':True,'video':video}
    (a.destination/'PROVENANCE.json').write_text(json.dumps(proof,indent=2),'utf8')
    hashes={p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(a.destination.iterdir()) if p.is_file()}
    (a.destination/'SHA256SUMS.txt').write_text(''.join(f'{sha}  {name}\n' for name,sha in hashes.items()),'utf8')
    return {'version':'0.1.0','prepared_files':len(hashes)+1,'source_commit':commit,'publicly_published':False}

if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('--package',type=pathlib.Path,required=True);p.add_argument('--video',type=pathlib.Path,required=True);p.add_argument('--video-report',type=pathlib.Path,required=True);p.add_argument('--destination',type=pathlib.Path,required=True);print(json.dumps(stage(p.parse_args())))
