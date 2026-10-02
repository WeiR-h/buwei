"""Stage only built code, public resources, launchers and license notices."""
import argparse,hashlib,json,pathlib,shutil,subprocess,tomllib
ROOT=pathlib.Path(__file__).resolve().parents[1]
def stage(binary,destination,metadata=None):
    destination=destination.resolve()
    if destination.exists():raise RuntimeError('Package destination already exists; preserved')
    if metadata is None:
        result=subprocess.check_output(['cargo','metadata','--manifest-path',str(ROOT/'native/Cargo.toml'),'--locked','--offline','--features','full-host','--filter-platform','x86_64-pc-windows-gnu','--format-version','1'],cwd=ROOT)
        metadata=json.loads(result)
    version=tomllib.loads((ROOT/'native/Cargo.toml').read_text('utf8'))['package']['version']
    native=destination/'native';native.mkdir(parents=True)
    shutil.copy2(binary,native/'buwei-rinx-dual-host.exe')
    (native/'Cargo.toml').write_text('[package]\nname="buwei-runtime"\nversion="'+version+'"\nedition="2024"\n','utf8')
    (native/'upstream').mkdir();shutil.copy2(ROOT/'.deps/octosense/desktop/upstream/makepad.json',native/'upstream/makepad.json')
    (native/'config').mkdir();(native/'config/apps.json').write_text('[]\n','utf8')
    licenses=destination/'licenses';licenses.mkdir();inventory=[]
    for package in metadata['packages']:
        directory=pathlib.Path(package['manifest_path']).parent;name=package['name'];resource=directory/'resources'
        if resource.is_dir():shutil.copytree(resource,native/name.replace('-','_')/'resources')
        notice=licenses/(name+'-'+package['version']);notice.mkdir(exist_ok=True)
        copied=[]
        for parent in [directory,*list(directory.parents)[:3]]:
            for file in parent.iterdir():
                if file.is_file() and (file.name.upper().startswith(('LICENSE','COPYING','NOTICE')) or (package.get('license_file') and file==directory/package['license_file'])):
                    if file.stat().st_size>2_000_000:continue
                    target=notice/file.name
                    if not target.exists():shutil.copy2(file,target);copied.append(file.name)
            if copied:break
        inventory.append({'name':name,'version':package['version'],'license':package.get('license'),'source':package.get('source'),'repository':package.get('repository'),'notice_files':copied})
    (licenses/'inventory.json').write_text(json.dumps(inventory,ensure_ascii=False,indent=2),'utf8')
    tools=destination/'tools';tools.mkdir()
    for name in ['Start-BuWei.ps1','Configure-Model.ps1','migrate.py']:shutil.copy2(ROOT/'tools'/name,tools/name)
    for name in ['LICENSE','NOTICE.md','README.md','README.en.md','CHANGELOG.md','dependencies.lock.json']:shutil.copy2(ROOT/name,destination/name)
    shutil.copytree(ROOT/'docs',destination/'docs')
    for role,label in [('organizer','启动组织者'),('participant','启动参与者')]:
        (destination/(label+'.cmd')).write_bytes(('@echo off\r\npowershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0tools\\Start-BuWei.ps1" -Role '+role+' -Executable "%~dp0native\\buwei-rinx-dual-host.exe"\r\n').encode('utf8'))
    hashes={p.relative_to(destination).as_posix():hashlib.sha256(p.read_bytes()).hexdigest() for p in destination.rglob('*') if p.is_file()}
    report={'version':version,'variant':'full-host; acceptance control excluded','native_binary_sha256':hashes['native/buwei-rinx-dual-host.exe'],'dependency_lock_sha256':hashes['dependencies.lock.json'],'private_configuration_copied':False,'sha256':hashes}
    (destination/'release.json').write_text(json.dumps(report,indent=2),'utf8')
    return report
if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('--binary',type=pathlib.Path,required=True);p.add_argument('--destination',type=pathlib.Path,required=True);p.add_argument('--metadata',type=pathlib.Path);a=p.parse_args()
    r=stage(a.binary,a.destination,json.loads(a.metadata.read_text('utf8')) if a.metadata else None);print(json.dumps({'version':r['version'],'files':len(r['sha256']),'private_configuration_copied':False}))
