"""Stable releases require measured evidence, including external acceptance."""
import argparse,json,pathlib,sys
REQUIRED=('regression_tests','formal_five_loops','formal_decline','formal_expiry','formal_outage_recovery','original_rinx_article','clean_windows_build','clean_windows_startup','package_privacy','video','public_download_verified')
def check(path):
    report=json.loads(path.read_text('utf8'))
    failures=[]
    for name in REQUIRED:
        item=report.get(name,{})
        if item.get('passed') is not True or not isinstance(item.get('evidence'),str):failures.append(name);continue
        file=(path.parent/item['evidence']).resolve()
        if not file.is_relative_to(path.parent.resolve()):failures.append(name);continue
        try:evidence=json.loads(file.read_text('utf8'))
        except (OSError,ValueError):failures.append(name);continue
        if evidence.get('passed') is not True or evidence.get('version')!=report['version']:failures.append(name)
    return {'version':report['version'],'stable_release_allowed':not failures,'missing_or_failed':failures}
if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('acceptance',type=pathlib.Path);a=p.parse_args();r=check(a.acceptance);print(json.dumps(r));sys.exit(0 if r['stable_release_allowed'] else 1)
