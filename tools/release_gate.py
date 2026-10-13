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
READING_REQUIRED=('weekly_availability_and_privacy','sparse_goal_correction','correction_independent_acceptance',
                  'reading_correction_five_loops','reading_update_recovery','three_identity_reading_trial',
                  'continuous_operation_video','recovery_technical_video')
PLANNING_REQUIRED=('scoped_agent_planning','verified_roster_export','community_selection_five_loops',
                   'community_admission_recovery','cross_machine_community_trial')
def check(path):
    report=json.loads(path.read_text('utf8'))
    failures=[]
    requirements=REQUIRED+COMMUNITY_REQUIRED if tuple(map(int,report['version'].split('.'))) >= (0,2,0) else REQUIRED
    for version,names in INTENT_STAGES.items():
        if tuple(map(int,report['version'].split('.')))>=version:requirements+=names
    if tuple(map(int,report['version'].split('.'))) >= (0,2,5):requirements+=READING_REQUIRED
    if tuple(map(int,report['version'].split('.'))) >= (0,3,0):requirements+=PLANNING_REQUIRED
    for name in requirements:
        item=report.get(name,{})
        if item.get('passed') is not True or not isinstance(item.get('evidence'),str):failures.append(name);continue
        file=(path.parent/item['evidence']).resolve()
        if not file.is_relative_to(path.parent.resolve()):failures.append(name);continue
        try:evidence=json.loads(file.read_text('utf8'))
        except (OSError,ValueError):failures.append(name);continue
        if evidence.get('passed') is not True or evidence.get('version')!=report['version']:failures.append(name)
        if name in ('model_independent_acceptance','intent_independent_acceptance') and (evidence.get('independent_cases',0)<100 or evidence.get('critical_information_accuracy',0)<0.95 or evidence.get('unauthorized_actions',-1)!=0):failures.append(name)
        if name=='correction_independent_acceptance' and (evidence.get('independent_cases',0)<100 or evidence.get('whole_case_accuracy',0)<0.95 or evidence.get('unauthorized_actions',-1)!=0):failures.append(name)
        if name=='reading_correction_five_loops':
            rounds=evidence.get('controlled_two_identity_rounds',[])
            if len(rounds)!=5 or any(len(r.get('events',[]))!=5 or any(e.get('service_event_count')!=1 for e in r['events']) or not all(r.get(k) is True for k in ('queue_position_preserved','original_task_preserved','goal_and_registration_confirmed_separately','preferences_unchanged','stale_preview_rejected')) for r in rounds):failures.append(name)
        if name=='three_identity_reading_trial' and (evidence.get('distinct_identities',0)<3 or evidence.get('actual_user_trial') is not True):failures.append(name)
        if name=='reading_update_recovery':
            cases=evidence.get('controlled_two_identity_rounds',[])
            if len(cases)!=2 or {c.get('fault') for c in cases}!={'lost_ack','crash_before_receipt'} or any(c.get('native_process_restarted') is not True or c.get('original_operation_recovered') is not True or c.get('blind_resends')!=0 or c.get('original_task_preserved') is not True or any(e.get('service_event_count')!=1 for e in c.get('events',[])) for c in cases):failures.append(name)
        if name in ('continuous_operation_video','recovery_technical_video') and evidence.get('continuous_native_capture') is not True:failures.append(name)
        if name=='scoped_agent_planning' and (evidence.get('actual_system_chat_call') is not True or evidence.get('account_scope_verified') is not True or evidence.get('unauthorized_actions',-1)!=0):failures.append(name)
        if name=='verified_roster_export' and not all(evidence.get(k) is True for k in ('owner_only','matches_verified_snapshot','formula_safe','member_visibility_private')):failures.append(name)
        if name=='community_selection_five_loops' and (evidence.get('distinct_identities',0)<2 or len(evidence.get('real_rounds',[]))<5 or any(r.get('registration_confirmed') is not True or r.get('service_event_counts_verified') is not True for r in evidence.get('real_rounds',[]))):failures.append(name)
        if name=='community_admission_recovery' and ({c.get('fault') for c in evidence.get('cases',[])}!={'before_send','lost_ack','crash_before_receipt'} or any(c.get('blind_resends')!=0 or c.get('original_operation_recovered') is not True for c in evidence.get('cases',[]))):failures.append(name)
        if name=='cross_machine_community_trial' and (evidence.get('distinct_identities',0)<3 or evidence.get('windows_machines',0)<2 or evidence.get('actual_user_confirmation') is not True):failures.append(name)
    return {'version':report['version'],'stable_release_allowed':not failures,'missing_or_failed':failures}
if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('acceptance',type=pathlib.Path);a=p.parse_args();r=check(a.acceptance);print(json.dumps(r));sys.exit(0 if r['stable_release_allowed'] else 1)
