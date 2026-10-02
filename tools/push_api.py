"""Publish existing commits through GitHub Git API if Git HTTPS is unavailable.

No force update, credential export, or alternate account. Requires gh login.
"""
import argparse,base64,json,pathlib,subprocess,datetime,time
ROOT=pathlib.Path(__file__).resolve().parents[1]
def git(*args):return subprocess.check_output(['git','-C',str(ROOT),*args])
def api(repository,path,data=None,method=None):
    args=['gh','api','repos/'+repository+'/'+path]
    if data is not None:args+=['--input','-']
    if method:args+=['--method',method]
    for attempt in range(3):
        try:result=subprocess.run(args,input=json.dumps(data).encode() if data is not None else None,capture_output=True,timeout=30)
        except subprocess.TimeoutExpired:
            if attempt==2:raise RuntimeError('GitHub API timed out: '+path)
            time.sleep(2);continue
        if result.returncode==0:return json.loads(result.stdout)
        if attempt==2:raise RuntimeError('GitHub API failed for '+path+': '+result.stderr.decode(errors='replace').strip()[-400:])
        time.sleep(2)
def identity(oid,prefix):
    fields=git('show','-s','--format=%'+prefix+'n%n%'+prefix+'e%n%'+prefix+'I',oid).decode().splitlines()
    return dict(zip(('name','email','date'),fields))
def main():
    parser=argparse.ArgumentParser();parser.add_argument('repository');args=parser.parse_args();repo=args.repository
    if repo!='WeiR-h/buwei':raise RuntimeError('This delivery is authorized only for WeiR-h/buwei')
    who=subprocess.check_output(['gh','api','user','--jq','.login'],text=True).strip()
    if who!='WeiR-h':raise RuntimeError('Wrong signed-in GitHub account')
    from public_scan import scan
    if not scan()['passed']:raise RuntimeError('Source/history scan failed')
    remote=api(repo,'git/ref/heads/main')['object']['sha'];desired=git('rev-parse','HEAD').decode().strip()
    if remote==desired:print(json.dumps({'commit':desired,'already_published':True}));return
    subprocess.run(['git','-C',str(ROOT),'merge-base','--is-ancestor',remote,desired],check=True)
    commits=git('rev-list','--reverse',remote+'..'+desired).decode().splitlines()
    previous=remote;original_previous=remote
    for oid in commits:
        parents=git('show','-s','--format=%P',oid).decode().strip().split()
        if parents!=[original_previous]:raise RuntimeError('Only existing linear commits are published; merge preserved')
        base=api(repo,'git/commits/'+previous)['tree']['sha'];entries=[]
        changed=git('diff','--name-only','-z',original_previous,oid).decode().split('\0')
        for name in filter(None,changed):
            listed=git('ls-tree',oid,'--',name).decode().strip()
            if not listed:entries.append({'path':name,'mode':'100644','type':'blob','sha':None});continue
            header=listed.split('\t',1)[0].split();mode,kind,blob=header
            if kind!='blob':raise RuntimeError('Submodules are not part of the release source')
            content=git('cat-file','blob',blob)
            got=api(repo,'git/blobs',{'content':base64.b64encode(content).decode(),'encoding':'base64'})
            if got['sha']!=blob:raise RuntimeError('Uploaded source blob mismatch')
            entries.append({'path':name,'mode':mode,'type':'blob','sha':blob})
        tree=api(repo,'git/trees',{'base_tree':base,'tree':entries})['sha']
        if tree!=git('show','-s','--format=%T',oid).decode().strip():raise RuntimeError('Source tree mismatch')
        message=git('cat-file','commit',oid).split(b'\n\n',1)[1].decode()
        created=api(repo,'git/commits',{'message':message,'tree':tree,'parents':[previous],'author':identity(oid,'a'),'committer':identity(oid,'c')})
        if created['message'].rstrip('\n')!=message.rstrip('\n'):raise RuntimeError('GitHub changed the commit message')
        if created['sha']!=oid:
            # GitHub normalizes Git identity timestamps to UTC. Import that
            # exact object after verifying its hash; preserve the original ref.
            def actor(kind):
                person=created[kind];date=datetime.datetime.fromisoformat(person['date'].replace('Z','+00:00'))
                return f"{kind} {person['name']} <{person['email']}> {int(date.timestamp())} +0000"
            original=git('cat-file','commit',oid).split(b'\n\n',1)[0].decode().splitlines()
            raw=None
            for author in {actor('author'),next(line for line in original if line.startswith('author '))}:
                for committer in {actor('committer'),next(line for line in original if line.startswith('committer '))}:
                    for body in {message,created['message'],created['message']+'\n'}:
                        candidate=('tree '+tree+'\nparent '+previous+'\n'+author+'\n'+committer+'\n\n'+body).encode()
                        got=subprocess.check_output(['git','-C',str(ROOT),'hash-object','-t','commit','--stdin'],input=candidate).decode().strip()
                        if got==created['sha']:raw=candidate
            if raw is None:raise RuntimeError('GitHub commit bytes could not be independently verified')
            subprocess.run(['git','-C',str(ROOT),'hash-object','-t','commit','-w','--stdin'],input=raw,check=True,stdout=subprocess.DEVNULL)
        previous=created['sha'];original_previous=oid
    # fast-forward validation remains server-side, including concurrent updates.
    api(repo,'git/refs/heads/main',{'sha':previous,'force':False},method='PATCH')
    if previous!=desired:
        git('update-ref','refs/heads/local-api-backup-'+desired[:12],desired)
        git('update-ref','refs/heads/main',previous,desired)
    git('update-ref','refs/remotes/origin/main',previous)
    print(json.dumps({'commit':previous,'source_tree_verified':True,'commits_published':len(commits),'fast_forward':True,'original_local_history_preserved':True}))
if __name__=='__main__':main()
