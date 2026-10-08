"""Check each staged file including executable bytes; print no matched content."""
import argparse,hashlib,json,pathlib,re,sys
# Inspection must leave downloaded package bytes unchanged. A packaged import
# must not create an unlisted __pycache__ file before the manifest check.
sys.dont_write_bytecode=True
from public_scan import PATTERNS
FORBIDDEN={'.run','.secrets','profiles','data','logs','.git','.deps','target'}
SUFFIX={'.dpapi','.db','.sqlite','.sqlite3','.key','.pem','.log'}
PEM_BLOCK=re.compile(rb'-----BEGIN ((?:RSA |EC |OPENSSH )?PRIVATE KEY)-----\s*[A-Za-z0-9+/=\r\n]{80,}\s*-----END \1-----')

# At locked octos b0759a5, LLVM pools these separate public literals:
# memory_refresh/redact.rs key prefixes and workspace_policy/contract.rs
# HTML probes plus notification text. These exact byte fragments contain
# no credential or room. Text files and every other binary match still fail.
BINARY_PUBLIC_LITERALS={
    'provider_key':{b''.join([b'sk-',b'sk_live_',b'sk_test_',b'ghp_',b'gho_',
        b'github_pat_',b'xoxb-',b'xoxp-',b'AKIA',b'ASIA',b'ya29'])},
    'private_room':{b'!DOCTYPE'+b'notify_user:Skill'},
}

def has_content_match(content,label,pattern,binary):
    return any(not(binary and match.group() in BINARY_PUBLIC_LITERALS.get(label,set()))
        for match in pattern.finditer(content))

def content_findings(content,binary=False):
    result=[]
    for label,pattern in PATTERNS.items():
        effective=PEM_BLOCK if binary and label=='private_key' else pattern
        if has_content_match(content,label,effective,binary):result.append(label)
        if binary and any(has_content_match(content[parity::2],label,effective,True) for parity in [0,1]):result.append(label+'_utf16')
    return result

def scan(directory):
    directory=directory.resolve();report=json.loads((directory/'release.json').read_text('utf8'))
    findings=[];files=0
    for file in directory.rglob('*'):
        if not file.is_file():continue
        name=file.relative_to(directory).as_posix();files+=1
        if set(file.relative_to(directory).parts)&FORBIDDEN or file.suffix.lower() in SUFFIX:findings.append({'path':name,'reason':'private_file'})
        content=file.read_bytes()
        if name!='release.json' and report['sha256'].get(name)!=hashlib.sha256(content).hexdigest():findings.append({'path':name,'reason':'manifest_mismatch'})
        for reason in content_findings(content,file.suffix.lower() in {'.exe','.dll'}):findings.append({'path':name,'reason':reason})
    for name in report['sha256']:
        if not (directory/name).is_file():findings.append({'path':name,'reason':'missing_manifest_file'})
    return {'version':report['version'],'passed':not findings,'files':files,'findings':findings,'coverage':'file allowlist, SHA-256, source patterns and binary ASCII/UTF16; screenshots reviewed separately'}
if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('directory',type=pathlib.Path);p.add_argument('--output',type=pathlib.Path);a=p.parse_args();r=scan(a.directory)
    if a.output:a.output.parent.mkdir(parents=True,exist_ok=True);a.output.write_text(json.dumps(r,indent=2),'utf8')
    print(json.dumps(r));sys.exit(0 if r['passed'] else 1)
