"""Verify changed goals and receipt history through real opt-in SDK hosts.

Only the existing Controller commands mutate state. SQLite access is read-only.
Goal cases prepare and reject unsent operations; the history case performs one
explicit registration/invitation/acceptance/cancellation in the private room.
"""
import argparse
import hashlib
import json
import pathlib
import sqlite3
import time
from contextlib import closing

from fault_acceptance import Hosts
from formal_control import Control
from intent_acceptance import IntentSuite, display


def digest(value):
    return hashlib.sha256(value.encode()).hexdigest()


class GoalConfirmationSuite(IntentSuite):
    def journal(self, role):
        profile = self.owner if role == 'organizer' else self.participant
        return next((profile / 'data' / ('v' + self.version) / 'native/rinx').glob('*/operations.db'))

    def recorded(self, operation_id):
        with closing(sqlite3.connect('file:' + self.journal('participant').as_posix() + '?mode=ro', uri=True)) as db:
            row = db.execute('select status,body from operations where id=?', (operation_id,)).fetchone()
        if row is None:
            raise AssertionError('Original operation disappeared from the journal')
        body = json.loads(row[1])
        assert row[0] == body['status'] and body['id'] == operation_id
        return body

    def goal_card(self, goal_id, timeout=95):
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            snapshot = self.call(self.p, 'Refresh')
            assert snapshot['version'] == self.version
            card = next((card for card in snapshot['assistance_cards']
                         if card['kind'] == 'opportunity' and card['activity_id'] == self.activity_id
                         and card['goal_id'] == goal_id), None)
            if card:
                return card
            time.sleep(1)
        raise TimeoutError('Verified opportunity for this exact goal did not appear')

    @staticmethod
    def task_for(snapshot, operation_id):
        return next(task for task in snapshot['intentions']['tasks']
                    if any(step.get('operation_id') == operation_id for step in task['steps']))

    def prepare_goal_action(self):
        goal_id = self.goal(self.p, 'participate')
        snapshot = self.use(self.p, self.goal_card(goal_id))
        operation = self.operation(snapshot, 'participant', 'prepared')
        return goal_id, self.task_for(snapshot, operation['id']), operation

    def form_for_goal(self, goal_id, title):
        snapshot = self.call(self.p, 'Refresh')
        goal = next(goal for goal in snapshot['intentions']['goals'] if goal['id'] == goal_id)
        value = goal['input']
        return {
            'kind': value['kind'], 'activity_id': value['activity_id'], 'title': title,
            'template': value['template'], 'earliest': display(value['earliest']),
            'latest': display(value['latest']), 'group': str(value['group']),
            'target': str(value['target']) if value['target'] is not None else '',
            'check_at': display(value['check_at']) if value['check_at'] is not None else '',
            'recurrence_days': str(value.get('recurrence_days')) if value.get('recurrence_days') is not None else '',
            'preparation_hours': str(value.get('preparation_hours', 24)),
        }

    def run_changes(self):
        self.o.authorize()
        self.p.authorize()
        assert self.call(self.o, 'PauseAutomation')['success']
        self.member_goal = self.goal(self.p, 'participate')
        self.intent(self.p, 'SetGoalStatus', {'id': self.member_goal, 'status': 'paused'})
        cases, rejected_ids = [], []
        for change in ('edit', 'pause_goal', 'delete_goal', 'pause_task', 'feedback', 'undo_feedback'):
            goal_id, task, operation = self.prepare_goal_action()
            if change == 'edit':
                form = self.form_for_goal(goal_id, '本人修改后的活动目标')
                self.intent(self.p, 'SaveGoal', {'id': goal_id, 'form': form})
            elif change == 'pause_goal':
                self.intent(self.p, 'SetGoalStatus', {'id': goal_id, 'status': 'paused'})
            elif change == 'delete_goal':
                self.intent(self.p, 'DeleteGoal', goal_id)
            elif change == 'pause_task':
                self.intent(self.p, 'PauseTask', task['id'])
            else:
                feedback = self.intent(self.p, 'Feedback', {
                    'goal_id': goal_id, 'template': 'badminton', 'group': 1, 'scope': 'this_occasion',
                })
                if change == 'undo_feedback':
                    # Test a fresh preview prepared AFTER the feedback so undo
                    # invalidates this preview, rather than an already cancelled one.
                    rejected_ids.append(operation['id'])
                    changed = self.use(self.p, self.goal_card(goal_id))
                    operation = self.operation(changed, 'participant', 'prepared')
                    task = self.task_for(changed, operation['id'])
                    self.intent(self.p, 'UndoFeedback', feedback['intentions']['feedback'][-1]['id'])
            rejected = self.call(self.p, 'ConfirmParticipant', self.preferences)
            assert not rejected['success'], 'Old preview executed after ' + change
            assert self.recorded(operation['id'])['status'] == 'cancelled'
            retained = self.task_for(self.call(self.p, 'Refresh'), operation['id'])
            assert retained['id'] == task['id']
            rejected_ids.append(operation['id'])
            cases.append({'change': change, 'old_confirmation_refused': True,
                          'journal_status': 'cancelled', 'task_binding_retained': True,
                          'operation_sha256': digest(operation['id'])})
            if change != 'delete_goal':
                self.intent(self.p, 'SetGoalStatus', {'id': goal_id, 'status': 'paused'})

        # Make a verified task receipt older than the forty-row display history
        # using only new, unsent Controller previews. No raw database insertion.
        goal_id, task, prepared = self.prepare_goal_action()
        joined = self.operation(self.call(self.p, 'ConfirmParticipant', self.preferences), 'participant', 'confirmed')
        assert joined['id'] == prepared['id']
        self.wait(self.owner, 'organizer verified history-case registration', lambda a: self.person(a).get('status') == 'waiting')
        self.wait(self.participant, 'member verified history-case registration', lambda a: self.person(a).get('status') == 'waiting')
        self.intent(self.p, 'PauseTask', task['id'])
        filler_ids = []
        for index in range(41):
            filler = self.operation(self.call(self.p, 'Cancel'), 'participant', 'prepared')
            # Identical unsent actions correctly reuse their prepared ID. A
            # real goal edit cancels that preview before preparing the next one.
            form = self.form_for_goal(goal_id, '历史恢复验收目标 ' + str(index + 1))
            self.intent(self.p, 'SaveGoal', {'id': goal_id, 'form': form})
            assert self.recorded(filler['id'])['status'] == 'cancelled'
            filler_ids.append(filler['id'])
        assert len(set(filler_ids)) == 41
        with closing(sqlite3.connect('file:' + self.journal('participant').as_posix() + '?mode=ro', uri=True)) as db:
            recent = [row[0] for row in db.execute('select id from operations order by rowid desc limit 40')]
        assert joined['id'] not in recent
        self.hosts.restart('participant')
        self.p = Control(self.participant, self.accounts[1], self.room)
        self.p.authorize()
        restored = self.task_for(self.call(self.p, 'Refresh'), joined['id'])
        assert restored['id'] == task['id'] and restored['status'] == 'paused'
        steps = [step for step in restored['steps'] if step.get('operation_id') == joined['id']]
        assert len(steps) == 1 and steps[0]['status'] == 'completed'
        assert self.recorded(joined['id'])['status'] == 'confirmed'
        # A fresh manual flow remains available after the old task was paused.
        invitation = self.invite()
        reply = self.reply(True)
        cancellation = self.cancel()
        events = self.collect_evidence()
        for operation_id in rejected_ids + filler_ids:
            assert not any(event['type'] in ('org.buwei.join', 'org.buwei.reply', 'org.buwei.cancel')
                           and event['content'].get('action_id') == operation_id for event in events['events'])
        for case in cases:
            case['service_event_count'] = 0
        return {
            'version': self.version, 'passed': True,
            'source': 'actual opt-in native Controller and SDK; read-only journal checks',
            'goal_change_cases': cases,
            'history_recovery': {'new_unsent_previews': 41, 'all_filler_previews_cancelled': True,
                                 'original_outside_recent_40': True,
                                 'native_process_restarted': True, 'original_task_retained': True,
                                 'original_receipt_retained': True, 'paused_task_stays_paused': True,
                                 'operation_sha256': digest(joined['id'])},
            'fresh_manual_flow': self.evidence([joined, invitation, reply, cancellation], events),
            'unsent_previews_service_event_count': 0,
            'collected_at_unix': events['collected_at_unix'],
        }


def run(args):
    hosts = Hosts(args.executable, args.owner_profile, args.participant_profile, args.version,
                  args.owner_port, args.participant_port, args.activity_id)
    try:
        hosts.start('organizer')
        hosts.start('participant')
        suite = GoalConfirmationSuite(hosts, args.private_trace, args.activity_id)
        report = suite.run_changes()
        report['executable_sha256'] = hashlib.sha256(args.executable.read_bytes()).hexdigest()
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(report, indent=2), 'utf8')
    finally:
        for role in ('organizer', 'participant'):
            if role in hosts.processes:
                hosts.stop(role)


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    for name in ('executable', 'owner-profile', 'participant-profile', 'private-trace', 'output'):
        parser.add_argument('--' + name, type=pathlib.Path, required=True)
    parser.add_argument('--version', required=True)
    parser.add_argument('--activity-id', required=True)
    parser.add_argument('--owner-port', type=int, default=8190)
    parser.add_argument('--participant-port', type=int, default=8191)
    run(parser.parse_args())
