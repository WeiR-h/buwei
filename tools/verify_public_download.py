"""Download public release assets without credentials and start their native host."""
import argparse,hashlib,json,pathlib,urllib.request,urllib.parse,zipfile
from startup_check import check as startup
from package_scan import scan
from build_proof import source_fingerprint
import build_proof

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
    a.destination.mkdir(parents=True,exist_ok=False)
    def download(name):
        if pathlib.PurePosixPath(name).name!=name or '\\' in name:raise ValueError('Invalid asset name')
        target=a.destination/name
        with urllib.request.urlopen(a.base_url.rstrip('/')+'/'+urllib.parse.quote(name),timeout=60) as response,target.open('wb') as out:
            if response.status!=200:raise ValueError('Public asset request failed')
            while True:
                chunk=response.read(2*1024*1024)
                if not chunk:break
                out.write(chunk)
        return target
    sums=download('SHA256SUMS.txt');hashes={}
    for line in sums.read_text('utf8').splitlines():
        expected,name=line.split('  ',1)
        if len(expected)!=64 or any(c not in '0123456789abcdef' for c in expected):raise ValueError('Invalid checksum')
        file=download(name);actual=hashlib.sha256(file.read_bytes()).hexdigest()
        if actual!=expected:raise ValueError('Public asset checksum differs: '+name)
        hashes[name]=actual
        print('Verified public asset: '+name,flush=True)
    proof=json.loads((a.destination/'PROVENANCE.json').read_text('utf8'))
    if proof['version']!='0.1.0':raise ValueError('Unexpected release version')
    package=a.destination/'windows';extract(a.destination/'BuWei-v0.1.0-windows-x64.zip',package)
    privacy=scan(package)
    if not privacy['passed']:raise ValueError('Downloaded package privacy check failed')
    runtime=startup(package,a.destination/'runtime-evidence')
    if not runtime['passed'] or runtime['version']!='0.1.0':raise ValueError('Downloaded native startup failed')
    source=a.destination/'source';extract(a.destination/'BuWei-v0.1.0-source.zip',source)
    build_proof.ROOT=source/'buwei-v0.1.0'
    if source_fingerprint()!=proof['native_source_sha256']:raise ValueError('Downloaded source differs from the binary build source')
    report={'version':'0.1.0','passed':True,'public_base_url':a.base_url,'http_credentials_used':False,'assets_sha256':hashes,'source_commit':proof['source_commit'],'binary_build_source_commit':proof['binary_build_source_commit'],'native_source_sha256':proof['native_source_sha256'],'package_privacy':privacy,'actual_downloaded_native_startup':runtime,'environment':'existing Windows machine, fresh profile and system-only PATH; separate clean Windows runner evidence remains required'}
    a.output.parent.mkdir(parents=True,exist_ok=True);a.output.write_text(json.dumps(report,indent=2),'utf8')
    return {'version':'0.1.0','passed':True,'public_assets_verified':len(hashes)}

if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('--base-url',required=True);p.add_argument('--destination',type=pathlib.Path,required=True);p.add_argument('--output',type=pathlib.Path,required=True)
    print(json.dumps(verify(p.parse_args())))
