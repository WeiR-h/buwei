"""Real SDK receipt recovery with durable intention-task associations.

Uses explicitly opted-in, stopped test profiles. Raw journal identities and
requests remain local; reports contain hashes and service event counts.
"""
import argparse, hashlib, json, pathlib, time
from fault_acceptance import FaultSuite, Hosts
from intent_acceptance import IntentSuite

class AssistanceFaultSuite(FaultSuite):
    intent = IntentSuite.intent
    goal = IntentSuite.goal
    card = IntentSuite.card
    use = IntentSuite.use

    def evidence(self, operations, events=None):
        if getattr(self, '_defer_evidence', False):
            return {'service_event_verification_pending': True}
        return super().evidence(operations, events)
    def prepare_intentions(self):
        self.o.authorize(); self.p.authorize()
        self.call(self.o, 'PauseAutomation')
        self.organizer_goal = self.goal(self.o, 'organize')
        self.member_goal = self.goal(self.p, 'participate')

    def call(self, control, command, value=None):
        if command == 'Prepare':
            self.use(control, self.card(control, 'shortfall'))
        elif command == 'Accept':
            self.use(control, self.card(control, 'invitation'))
        return super().call(control, command, value)

    def enroll(self):
        result = self.use(self.p, self.card(self.p, 'opportunity'))
        prepared = self.operation(result, 'participant', 'prepared')
        sent = self.operation(self.call(self.p, 'ConfirmParticipant', self.preferences), 'participant', 'confirmed')
        assert prepared['id'] == sent['id']
        self.wait(self.owner, 'verified assisted registration', lambda a:self.person(a).get('status') == 'waiting')
        self.wait(self.participant, 'received assisted registration', lambda a:self.person(a).get('status') == 'waiting')
        return sent

    def linked_tasks(self, control, operation):
        tasks = self.call(control, 'Refresh')['intentions']['tasks']
        return {t['id']:t for t in tasks if any(s.get('operation_id') == operation['id'] for s in t['steps'])}

    def fault_dispatch(self, role, mode, name, value, key, operation):
        control = self.o if role == 'organizer' else self.p
        before = self.linked_tasks(control, operation)
        if key != 'article':
            assert before, 'Assisted action must reference a durable task before sending'
        recovered = super().fault_dispatch(role, mode, name, value, key, operation)
        control = self.o if role == 'organizer' else self.p
        after = self.linked_tasks(control, recovered)
        assert set(before) <= set(after), 'Restart must retain the original task and operation association'
        for identifier in before:
            task = after[identifier]
            linked = [s for s in task['steps'] if s.get('operation_id') == operation['id']]
            assert len(linked) == 1 and linked[0]['status'] == 'completed'
            # Resume and full history pagination follow the time-sensitive
            # invitation reply. Reading all evidence first can consume its TTL.
            self.resume_checks.append((role, identifier, recovered))
        self.cases[-1]['original_task_ids_retained'] = True
        self.cases[-1]['associated_tasks'] = [hashlib.sha256(i.encode()).hexdigest() for i in before]
        self.recovered_operations.append(recovered)
        return recovered

    def run_assistance_faults(self):
        self._defer_evidence = True
        self.recovered_operations = []; self.resume_checks = []
        self.prepare_intentions()
        for mode in ('lost_ack', 'crash_before_receipt'):
            prepared = self.operation(self.use(self.p, self.card(self.p, 'opportunity')), 'participant', 'prepared')
            self.fault_dispatch('participant', mode, 'ConfirmParticipant', self.preferences, 'participant', prepared)
            self.wait(self.owner, 'recovered registration available', lambda a:self.person(a).get('status') == 'waiting')
            self.invite(); self.reply(False)
        # Existing invite/reply/article fault scripts keep the same scoring and
        # event-count requirements; only intention preparation is added.
        report = super().run_faults()
        self._defer_evidence = False
        events = self.collect_evidence()
        for record, op in zip(self.cases, self.recovered_operations):
            record.pop('service_event_verification_pending', None)
            record.update(self.evidence([op], events))
        for role, identifier, op in self.resume_checks:
            control = self.o if role == 'organizer' else self.p
            task = self.linked_tasks(control, op)[identifier]
            if task['status'] != 'completed':
                resumed = self.intent(control, 'ResumeTask', identifier)
                assert resumed.get('success')
                assert identifier in self.linked_tasks(control, op)
        report['durable_assistance_tasks'] = True
        report['registration_fault_cases'] = 2
        return report

def run(args):
    h = Hosts(args.executable, args.owner_profile, args.participant_profile, args.version, args.owner_port, args.participant_port, args.activity_id)
    try:
        h.start('organizer'); h.start('participant')
        suite = AssistanceFaultSuite(h, args.private_trace)
        report = suite.run_assistance_faults()
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(report, indent=2), 'utf8')
    finally:
        for role in ('organizer', 'participant'):
            if role in h.processes:h.stop(role)

if __name__ == '__main__':
    p = argparse.ArgumentParser()
    for name in ('executable', 'owner-profile', 'participant-profile', 'private-trace', 'output'):
        p.add_argument('--'+name, type=pathlib.Path, required=True)
    p.add_argument('--version', required=True); p.add_argument('--activity-id', required=True)
    p.add_argument('--owner-port', type=int, default=8180); p.add_argument('--participant-port', type=int, default=8181)
    run(p.parse_args())
