"""Stable releases require measured evidence, including external acceptance."""
import argparse,json,pathlib,sys
REQUIRED=('regression_tests','formal_five_loops','formal_decline','formal_expiry','formal_outage_recovery','original_rinx_article','clean_windows_build','clean_windows_startup','package_privacy','video','public_download_verified')
def check(path):
    report=json.loads(path.read_text('utf8'))
    failures=[name for name in REQUIRED if report.get(name,{}).get('passed') is not True or not report[name].get('evidence')]
    return {'version':report['version'],'stable_release_allowed':not failures,'missing_or_failed':failures}
if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('acceptance',type=pathlib.Path);a=p.parse_args();r=check(a.acceptance);print(json.dumps(r));sys.exit(0 if r['stable_release_allowed'] else 1)
