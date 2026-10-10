"""Stage the four formal attachments without publishing."""
import argparse,hashlib,json,pathlib,shutil,subprocess,zipfile
from package_scan import scan
from public_scan import scan as scan_source
from build_proof import source_fingerprint,digest
ROOT=pathlib.Path(__file__).resolve().parents[1]
def archive_tree(source,destination):
    with zipfile.ZipFile(destination,'w',zipfile.ZIP_DEFLATED,compresslevel=6) as z:
        for file in sorted(source.rglob('*')):
            if file.is_file():z.write(file,file.relative_to(source).as_posix())
def stage(a):
    if not scan_source()['passed'] or not scan(a.package)['passed']:raise RuntimeError('Public source or package scan failed')
    release=json.loads((a.package/'release.json').read_text('utf8'));version=release['version']
    if release['build_proof']['native_source_sha256']!=source_fingerprint():raise RuntimeError('Package source differs')
    if release['dependency_lock_sha256']!=digest(ROOT/'dependencies.lock.json'):raise RuntimeError('Package dependencies differ')
    privacy=a.package/'docs/PRIVACY.md'
    if privacy.read_bytes()!=(ROOT/'docs/PRIVACY.md').read_bytes():raise RuntimeError('Package privacy notice differs from the reviewed source')
    if subprocess.check_output(['git','status','--porcelain','--untracked-files=no'],cwd=ROOT,text=True).strip():raise RuntimeError('Commit source before staging')
    commit=subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip()
    if release['build_proof']['source_commit']!=commit:raise RuntimeError('Build commit differs')
    video=json.loads(a.video_report.read_text('utf8'))
    if video.get('passed') is not True or video.get('version')!=version or hashlib.sha256(a.video.read_bytes()).hexdigest()!=video['video_sha256']:raise RuntimeError('Verified version-matched demonstration required')
    a.destination.mkdir(parents=True,exist_ok=False)
    archive_tree(a.package,a.destination/f'BuWei-v{version}-windows-x64.zip')
    shutil.copy2(a.video,a.destination/f'BuWei-v{version}-demo.mp4')
    shutil.copy2(privacy,a.destination/'PRIVACY.md')
    hashes={p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(a.destination.iterdir()) if p.is_file()}
    (a.destination/'SHA256SUMS.txt').write_text(''.join(f'{sha}  {name}\n' for name,sha in hashes.items()),'utf8')
    return {'version':version,'prepared_files':4,'source_commit':commit,'source_archive':'GitHub fixed tag archive','publicly_published':False}
if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('--package',type=pathlib.Path,required=True);p.add_argument('--video',type=pathlib.Path,required=True);p.add_argument('--video-report',type=pathlib.Path,required=True);p.add_argument('--destination',type=pathlib.Path,required=True);print(json.dumps(stage(p.parse_args())))
