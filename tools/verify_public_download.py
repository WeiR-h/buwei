"""Download public release assets without credentials and start their native host."""
import argparse,hashlib,json,pathlib,re,time,urllib.request,urllib.parse,zipfile
from startup_check import check as startup
from package_scan import scan
from build_proof import source_fingerprint
import build_proof

def formal_checksums(text,version):
    hashes={}
    for line in text.splitlines():
        expected,name=line.split('  ',1)
        if not re.fullmatch('[0-9a-f]{64}',expected) or name in hashes:raise ValueError('Invalid or duplicate checksum')
        hashes[name]=expected
    wanted={f'BuWei-v{version}-windows-x64.zip',f'BuWei-v{version}-demo.mp4'}
    if set(hashes)!=wanted:raise ValueError('Formal release must contain exactly the runtime and demonstration checksums')
    return hashes

def check_release_metadata(published,asset_names,allow_preview=False):
    if published.get('draft') is not False:
        raise ValueError('Release must be publicly published')
    if published.get('prerelease') not in (True,False):
        raise ValueError('Release status is missing')
    if published['prerelease'] and not allow_preview:
        raise ValueError('Preview download requires an explicit preview check')
    if {x['name'] for x in published['assets']}!=set(asset_names):
        raise ValueError('Public release attachment set differs')

def extract(archive,destination):
    destination.mkdir(parents=True,exist_ok=False)
    with zipfile.ZipFile(archive) as z:
        for item in z.infolist():
            target=(destination/item.filename).resolve()
            if not target.is_relative_to(destination.resolve()) or (item.external_attr>>16)&0o170000==0o120000:
                raise ValueError('Unsafe archive entry')
        z.extractall(destination)

def verify(a):
    url=urllib.parse.urlsplit(a.base_url)
    if url.scheme!='https' or url.netloc!='github.com' or not url.path.startswith('/WeiR-h/buwei/releases/download/') or url.query or url.fragment:
        raise ValueError('Expected a public BuWei release download URL')
    tag=url.path.rsplit('/',1)[1]
    if not re.fullmatch(r'v\d+\.\d+\.\d+',tag):raise ValueError('Expected a fixed formal release tag')
    version=tag[1:]
    a.destination.mkdir(parents=True,exist_ok=False)
    def download(name,address=None):
        if pathlib.PurePosixPath(name).name!=name or '\\' in name:raise ValueError('Invalid asset name')
        target=a.destination/name
        deadline=time.monotonic()+300
        with urllib.request.urlopen(address or a.base_url.rstrip('/')+'/'+urllib.parse.quote(name),timeout=25) as response,target.open('wb') as out:
            if response.status!=200:raise ValueError('Public asset request failed')
            while True:
                if time.monotonic()>deadline:raise TimeoutError('Public download deadline exceeded')
                chunk=response.read(2*1024*1024)
                if not chunk:break
                out.write(chunk)
                if out.tell()>512*1024*1024:raise ValueError('Unexpected public asset size')
        return target
    sums=download('SHA256SUMS.txt');hashes={}
    for name,expected in formal_checksums(sums.read_text('utf8'),version).items():
        file=download(name);actual=hashlib.sha256(file.read_bytes()).hexdigest()
        if actual!=expected:raise ValueError('Public asset checksum differs: '+name)
        hashes[name]=actual
        print('Verified public asset: '+name,flush=True)
    package=a.destination/'windows';extract(a.destination/('BuWei-v'+version+'-windows-x64.zip'),package)
    release=json.loads((package/'release.json').read_text('utf8'));proof=release['build_proof']
    if release['version']!=version or not proof['passed'] or proof['features']!=['full-host']:raise ValueError('Downloaded runtime is not the verified formal host')
    privacy=scan(package)
    if not privacy['passed']:raise ValueError('Downloaded package privacy check failed')
    runtime=startup(package,a.destination/'runtime-evidence')
    if not runtime['passed'] or runtime['version']!=version:raise ValueError('Downloaded native startup failed')
    source_zip=download('source-'+tag+'.zip','https://github.com/WeiR-h/buwei/archive/refs/tags/'+tag+'.zip')
    source=a.destination/'source';extract(source_zip,source)
    roots=list(source.iterdir())
    if len(roots)!=1 or not roots[0].is_dir():raise ValueError('Unexpected fixed-tag source archive')
    build_proof.ROOT=roots[0]
    if source_fingerprint()!=proof['native_source_sha256']:raise ValueError('Downloaded source differs from the binary build source')
    source_lock=hashlib.sha256((build_proof.ROOT/'dependencies.lock.json').read_bytes()).hexdigest()
    if source_lock!=release['dependency_lock_sha256'] or source_lock!=proof['dependencies_lock_sha256']:raise ValueError('Downloaded fixed dependencies differ from the build')
    binary=hashlib.sha256((package/'native/buwei-rinx-dual-host.exe').read_bytes()).hexdigest()
    if binary!=proof['binary_sha256'] or binary!=release['native_binary_sha256']:raise ValueError('Downloaded binary differs from the build')
    def public_api(path):
        request=urllib.request.Request('https://api.github.com/repos/WeiR-h/buwei/'+path,headers={'Accept':'application/vnd.github+json','User-Agent':'BuWei-release-verifier'})
        with urllib.request.urlopen(request,timeout=25) as response:return json.load(response)
    ref=public_api('git/ref/tags/'+urllib.parse.quote(tag,safe=''))['object']
    for _ in range(4):
        if ref['type']=='commit':break
        if ref['type']!='tag':raise ValueError('Public tag does not refer to a commit')
        ref=public_api('git/tags/'+ref['sha'])['object']
    if ref['type']!='commit' or ref['sha']!=proof['source_commit']:raise ValueError('Public tag differs from the archived source commit')
    published=public_api('releases/tags/'+tag)
    check_release_metadata(published,set(hashes)|{'SHA256SUMS.txt'},getattr(a,'allow_preview',False))
    hashes['SHA256SUMS.txt']=hashlib.sha256(sums.read_bytes()).hexdigest()
    for asset in published['assets']:
        if asset.get('digest') and asset['digest']!='sha256:'+hashes[asset['name']]:raise ValueError('Public asset differs from GitHub upload digest')
    report={'version':version,'passed':True,'public_base_url':a.base_url,'http_credentials_used':False,'assets_sha256':hashes,'source_commit':proof['source_commit'],'binary_build_source_commit':proof['source_commit'],'native_source_sha256':proof['native_source_sha256'],'fixed_tag_source_archive_sha256':hashlib.sha256(source_zip.read_bytes()).hexdigest(),'package_privacy':privacy,'actual_downloaded_native_startup':runtime,'environment':'existing Windows machine, fresh profile and system-only PATH; separate clean Windows runner evidence remains required'}
    report.update(public_tag_commit_verified=True,dependency_lock_sha256=source_lock,native_binary_sha256=binary,public_release_is_preview=published['prerelease'])
    a.output.parent.mkdir(parents=True,exist_ok=True);a.output.write_text(json.dumps(report,indent=2),'utf8')
    return {'version':version,'passed':True,'public_assets_verified':len(hashes)}

if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('--base-url',required=True);p.add_argument('--destination',type=pathlib.Path,required=True);p.add_argument('--output',type=pathlib.Path,required=True);p.add_argument('--allow-preview',action='store_true',help='Verify a public preview without declaring it a stable release')
    print(json.dumps(verify(p.parse_args())))
