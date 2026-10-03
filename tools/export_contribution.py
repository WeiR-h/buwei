"""Export an independently reviewable addition against Git's empty tree.

This is our library addition, not a patch claimed to be merged by upstream.
"""
import argparse,hashlib,json,pathlib,subprocess
ROOT=pathlib.Path(__file__).resolve().parents[1]
EMPTY='4b825dc642cb6eb9a060e54bf8d69288fbee4904'

def export(destination):
    destination.mkdir(parents=True,exist_ok=False)
    commit=subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip()
    content=subprocess.check_output(['git','diff','--binary',EMPTY,commit,'--','native/crates/action-receipts','LICENSE',':(exclude)native/crates/action-receipts/Cargo.lock'],cwd=ROOT)
    patch=destination/'action-receipts.patch';patch.write_bytes(content)
    report={'source_commit':commit,'base':'empty Git tree','base_tree':EMPTY,'paths':['native/crates/action-receipts','LICENSE'],'workspace_lock':'native/Cargo.lock from recorded source commit; redundant nested historical lock excluded','sha256':hashlib.sha256(content).hexdigest(),'upstream_submitted':False,'upstream_merged':False,'validation':'apply to an empty review directory; reproduce fixed workspace tests from the source commit'}
    (destination/'contribution.json').write_text(json.dumps(report,indent=2),'utf8')
    (destination/'README.txt').write_text('BuWei action-receipts contribution\n\nApply in an empty Git review directory: git apply action-receipts.patch\nThe patch adds our Apache-2.0 library; it is not an upstream integration patch.\nFor reproducible dependency versions, use Cargo.lock and workspace tests from the recorded source commit.\nSee docs/BUILD.md and native/crates/action-receipts/tests for the workflow and recovery checks.\n','utf8')
    return report

if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('destination',type=pathlib.Path);a=p.parse_args();print(json.dumps(export(a.destination)))
