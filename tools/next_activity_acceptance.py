"""Verify recurring preparation and explicit creation using a real SDK host."""
import argparse, hashlib, json, pathlib, time
from fault_acceptance import Hosts
from formal_control import Control
from intent_acceptance import IntentSuite, display

def run(args):
    h = Hosts(args.executable, args.owner_profile, args.participant_profile, args.version, args.owner_port, args.participant_port, args.activity_id)
    try:
        h.start('organizer'); h.start('participant')
        s = IntentSuite(h, args.private_trace, args.activity_id)
        s.o.authorize(); s.p.authorize(); s.call(s.o, 'PauseAutomation')
        a = s.state(s.owner)
        form = dict(kind='organize', activity_id=args.activity_id, title='按本人周期筹备下一场', template=a['metadata']['template'], earliest=display(a['start']), latest=display(a['end']), group='1', target=str(a['capacity']), check_at=display(int(time.time())//60*60-60), recurrence_days='1', preparation_hours='168')
        goal = s.intent(s.o, 'SaveGoal', {'id':None, 'form':form})['goal_id']
        card = s.card(s.o, 'next_activity')
        assert card['goal_id'] == goal
        before = s.call(s.o, 'Refresh')
        prepared = s.use(s.o, card)
        assert len(prepared['activities']) == len(before['activities'])
        task = next(t for t in prepared['intentions']['tasks'] if t['card_id'] == card['id'] and t['goal_id'] == goal)
        assert task['status'] == 'awaiting_confirmation'
        copied = prepared['create_form']
        assert copied and copied['start'] == copied['end'] == ''
        assert copied['capacity'] == a['capacity'] and copied['location'] == a['metadata']['location']
        confirmed = dict(copied, start=display(a['start']+86400), end=display(a['end']+86400))
        created = s.call(s.o, 'CreateDated', confirmed)
        assert created['success']
        new = created['activity']; identifier = new['metadata']['activity_id']
        assert identifier != args.activity_id and new['room'] != a['room']
        assert new['owner'] == s.accounts[0] and not new['people'] and not new['invitations']
        final_task = next(t for t in created['intentions']['tasks'] if t['id'] == task['id'])
        assert final_task['status'] == 'completed'
        assert all(step['status'] == 'completed' for step in final_task['steps'])
        # The same cycle must stop recommending another copy after creation.
        s.o = Control(s.owner, s.accounts[0], new['room'])
        refreshed = s.call(s.o, 'Refresh')
        assert not any(c['id'] == card['id'] for c in refreshed['assistance_cards'])
        report = dict(version=args.version, passed=True, source='actual SDK creation after explicit confirmation', draft_keeps_dates_unconfirmed=True, independent_activity=True, participants_reregister=True, original_task_retained=True, same_period_not_repeated=True, task_sha256=hashlib.sha256(task['id'].encode()).hexdigest(), activity_sha256=hashlib.sha256(identifier.encode()).hexdigest(), collected_at_unix=int(time.time()))
        args.output.parent.mkdir(parents=True, exist_ok=True);args.output.write_text(json.dumps(report, indent=2), 'utf8')
    finally:
        for role in ('organizer','participant'):
            if role in h.processes:h.stop(role)

if __name__ == '__main__':
    p=argparse.ArgumentParser()
    for name in ('executable','owner-profile','participant-profile','private-trace','output'):p.add_argument('--'+name,type=pathlib.Path,required=True)
    p.add_argument('--version',required=True);p.add_argument('--activity-id',required=True)
    p.add_argument('--owner-port',type=int,default=8180);p.add_argument('--participant-port',type=int,default=8181)
    run(p.parse_args())
