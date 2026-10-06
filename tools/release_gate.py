"""Stable releases require measured evidence, including external acceptance."""
import argparse,json,pathlib,sys
REQUIRED=('regression_tests','formal_five_loops','formal_decline','formal_expiry','formal_outage_recovery','original_rinx_article','clean_windows_build','clean_windows_startup','package_privacy','video','public_download_verified')
COMMUNITY_REQUIRED=('five_activity_isolation','group_capacity_and_dates','activity_card_entry','automatic_replacement','automation_revocation_and_expiry','model_independent_acceptance','original_rinx_regression','migration_preserves_operations','organizer_operations_comparison')
INTENT_STAGES={
 (0,2,1):('intent_privacy_and_preferences','intent_missing_information'),
 (0,2,2):('proactive_facts_and_reminders','intent_independent_acceptance'),
 (0,2,3):('proactive_five_loops','durable_task_recovery'),
 (0,2,4):('tray_hidden_real_flow','tray_pause_switch_exit'),
 (0,2,5):('explicit_feedback_and_undo','next_activity_preparation','proactive_value_comparison'),
}
def check(path):
    report=json.loads(path.read_text('utf8'))
    failures=[]
    requirements=REQUIRED+COMMUNITY_REQUIRED if tuple(map(int,report['version'].split('.'))) >= (0,2,0) else REQUIRED
    for version,names in INTENT_STAGES.items():
        if tuple(map(int,report['version'].split('.')))>=version:requirements+=names
    for name in requirements:
        item=report.get(name,{})
        if item.get('passed') is not True or not isinstance(item.get('evidence'),str):failures.append(name);continue
        file=(path.parent/item['evidence']).resolve()
        if not file.is_relative_to(path.parent.resolve()):failures.append(name);continue
        try:evidence=json.loads(file.read_text('utf8'))
        except (OSError,ValueError):failures.append(name);continue
        if evidence.get('passed') is not True or evidence.get('version')!=report['version']:failures.append(name)
        if name in ('model_independent_acceptance','intent_independent_acceptance') and (evidence.get('independent_cases',0)<100 or evidence.get('critical_information_accuracy',0)<0.95 or evidence.get('unauthorized_actions',-1)!=0):failures.append(name)
    return {'version':report['version'],'stable_release_allowed':not failures,'missing_or_failed':failures}
if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('acceptance',type=pathlib.Path);a=p.parse_args();r=check(a.acceptance);print(json.dumps(r));sys.exit(0 if r['stable_release_allowed'] else 1)
