"""Scan tracked source and all reachable Git blobs without printing contents."""
import json,pathlib,re,subprocess,sys
ROOT=pathlib.Path(__file__).resolve().parents[1]
ALLOWED={'.cargo','.github','native','tools','patches','docs','evidence','assets','licenses'}
TOP={'README.md','README.en.md','LICENSE','NOTICE.md','CHANGELOG.md','.gitignore','.gitattributes','dependencies.lock.json','rust-toolchain.toml'}
BLOCKED={'.deps','.run','.secrets','data','logs','profiles','private-evidence','target','__pycache__'}
PATTERNS={
 'private_windows_path':re.compile(rb'[A-Z]:[\\/](?:Users|xwechat_files)[\\/]',re.I),
 'provider_key':re.compile(rb'\b(?:sk-|gh[pousr]_)[A-Za-z0-9_-]{30,}'),
 'private_key':re.compile(rb'-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----'),
 'private_rinx_identity':re.compile(rb'@WeiR-h\d*:matrix\.rinx\.chat'),
 'private_room':re.compile(rb'![A-Za-z0-9_-]{16,}:[a-zA-Z0-9.-]+'),
}

def findings(content):return [name for name,pattern in PATTERNS.items() if pattern.search(content)]
def run(*args):return subprocess.check_output(['git','-C',str(ROOT),*args])
def scan():
    failures=[];files=run('ls-files','-z').decode('utf8').split('\0');count=0
    for name in filter(None,files):
        path=pathlib.PurePosixPath(name);count+=1
        if (len(path.parts)==1 and name not in TOP) or (len(path.parts)>1 and path.parts[0] not in ALLOWED) or any(p in BLOCKED for p in path.parts) or path.suffix in {'.db','.dpapi','.key','.pem','.log','.exe','.dll'}:
            failures.append({'path':name,'reason':'outside_public_allowlist'})
        for reason in findings((ROOT/name).read_bytes()):failures.append({'path':name,'reason':reason})
    # Read object bytes, but never include secrets or excerpts in output.
    objects=0
    try:
        listing=run('rev-list','--objects','--all').splitlines()
    except subprocess.CalledProcessError:listing=[]
    for entry in listing:
        oid=entry.split(b' ',1)[0].decode('ascii')
        if run('cat-file','-t',oid).strip()!=b'blob':continue
        objects+=1
        for reason in findings(run('cat-file','blob',oid)):failures.append({'object':oid,'reason':reason})
    return {'tracked_files':count,'history_blobs':objects,'passed':not failures,'findings':failures,'coverage':'source allowlist, known private identifiers, provider key and private path patterns; manual package review still required'}
if __name__=='__main__':
    report=scan();print(json.dumps(report,ensure_ascii=True,indent=2));sys.exit(0 if report['passed'] else 1)
