use action_receipts::*;
use buwei_host_core::{Activity, Preferences, participation::Intent};
use std::collections::BTreeMap;
use std::sync::{
    Mutex,
    atomic::{AtomicUsize, Ordering},
};

fn activity() -> Activity {
    Activity::new(
        "@owner:server".into(),
        "!activity".into(),
        "活动".into(),
        1,
        19,
        21,
    )
    .unwrap()
}
fn grant(actor: &str) -> Grant {
    let a = Authority::default();
    a.set_account(Some(actor));
    a.grant("buwei", &["participate"], 100, 10000).unwrap()
}
fn journal() -> (std::path::PathBuf, Journal) {
    let path = std::env::temp_dir().join(format!("participant-{}.db", new_id()));
    let j = Journal::open(&path).unwrap();
    (path, j)
}
struct Server {
    activity: Activity,
    actor: String,
    calls: AtomicUsize,
    events: Mutex<BTreeMap<String, Evidence>>,
    fault: u8,
}
impl Server {
    fn new(fault: u8) -> Self {
        Self {
            activity: activity(),
            actor: "@p:server".into(),
            calls: AtomicUsize::new(0),
            events: Mutex::new(BTreeMap::new()),
            fault,
        }
    }
}
impl Adapter for Server {
    fn validate(&self, action: &Action, revision: u64) -> action_receipts::Result<()> {
        if revision != self.activity.revision || action.target != self.activity.room {
            return Err(Error::Conflict);
        }
        Intent::parse(action)
            .and_then(|i| i.validate(&self.activity, &self.actor, 101))
            .map_err(|_| Error::Conflict)
    }
    fn dispatch(&self, op: &Operation) -> Dispatch {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if self.fault == 2 {
            return Dispatch::Uncertain("disconnected before server receives".into());
        }
        let e = Evidence {
            operation_id: op.id.clone(),
            external_id: format!("${}", op.id),
            account: op.account.clone(),
            target: op.action.target.clone(),
            digest: op.digest.clone(),
        };
        self.events.lock().unwrap().insert(op.id.clone(), e.clone());
        if self.fault == 3 {
            panic!("crash after server commit before local receipt");
        }
        if self.fault == 1 {
            Dispatch::Uncertain("lost acknowledgement".into())
        } else {
            Dispatch::Verified(e)
        }
    }
    fn lookup(&self, op: &Operation) -> action_receipts::Result<Option<Evidence>> {
        Ok(self.events.lock().unwrap().get(&op.id).cloned())
    }
}
fn intent() -> Intent {
    Intent::Join {
        preferences: Preferences {
            earliest: 17,
            latest: 23,
            group: 1,
        },
    }
}
#[test]
fn participant_repeat_confirmation_is_one_external_effect() {
    let (_, mut j) = journal();
    let g = grant("@p:server");
    let s = Server::new(0);
    let o = j
        .prepare(&g, intent().action(&s.activity), 1, 100, 120)
        .unwrap();
    j.confirm(&g, &o.id, &s, 101).unwrap();
    j.execute(&g, &o.id, &s, 102).unwrap();
    j.confirm(&g, &o.id, &s, 103).unwrap();
    j.execute(&g, &o.id, &s, 103).unwrap();
    assert_eq!(s.calls.load(Ordering::SeqCst), 1);
}
#[test]
fn participant_lost_ack_restart_recovers_same_id_without_resend() {
    let (path, mut j) = journal();
    let g = grant("@p:server");
    let s = Server::new(1);
    let o = j
        .prepare(&g, intent().action(&s.activity), 1, 100, 120)
        .unwrap();
    j.confirm(&g, &o.id, &s, 101).unwrap();
    assert_eq!(
        j.execute(&g, &o.id, &s, 102).unwrap().status,
        Status::Unknown
    );
    drop(j);
    let mut j = Journal::open(&path).unwrap();
    let fresh = grant("@p:server");
    assert_eq!(j.pending(&fresh, 500).unwrap()[0].id, o.id);
    let r = j.reconcile(&fresh, &o.id, &s, 500).unwrap();
    assert_eq!(r.status, Status::Confirmed);
    assert_eq!(r.id, o.id);
    assert_eq!(s.calls.load(Ordering::SeqCst), 1);
}
#[test]
fn participant_before_send_outage_remains_unknown_and_never_resends() {
    let (_, mut j) = journal();
    let g = grant("@p:server");
    let s = Server::new(2);
    let o = j
        .prepare(&g, intent().action(&s.activity), 1, 100, 120)
        .unwrap();
    j.confirm(&g, &o.id, &s, 101).unwrap();
    j.execute(&g, &o.id, &s, 102).unwrap();
    assert_eq!(
        j.reconcile(&g, &o.id, &s, 500).unwrap().status,
        Status::Unknown
    );
    assert!(j.execute(&g, &o.id, &s, 500).is_err());
    assert_eq!(s.calls.load(Ordering::SeqCst), 1);
    assert!(s.events.lock().unwrap().is_empty());
}
#[test]
fn participant_crash_before_receipt_is_discovered_after_restart() {
    let (path, mut j) = journal();
    let g = grant("@p:server");
    let s = Server::new(3);
    let o = j
        .prepare(&g, intent().action(&s.activity), 1, 100, 120)
        .unwrap();
    j.confirm(&g, &o.id, &s, 101).unwrap();
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(
            || j.execute(&g, &o.id, &s, 102)
        ))
        .is_err()
    );
    drop(j);
    let mut j = Journal::open(&path).unwrap();
    let fresh = grant("@p:server");
    assert_eq!(
        j.pending(&fresh, 500).unwrap()[0].status,
        Status::Dispatching
    );
    assert_eq!(
        j.reconcile(&fresh, &o.id, &s, 500).unwrap().status,
        Status::Confirmed
    );
    assert_eq!(s.events.lock().unwrap().len(), 1);
}
#[test]
fn pending_scan_does_not_lose_old_attempts_and_is_account_scoped() {
    let (_, mut j) = journal();
    let g = grant("@p:server");
    let s = Server::new(1);
    let mut ids = vec![];
    for _ in 0..45 {
        let o = j
            .prepare(&g, intent().action(&s.activity), 1, 100, 120)
            .unwrap();
        j.confirm(&g, &o.id, &s, 101).unwrap();
        j.execute(&g, &o.id, &s, 102).unwrap();
        ids.push(o.id);
    }
    assert_eq!(j.recent(&g, 103).unwrap().len(), 40);
    let pending = j.pending(&g, 103).unwrap();
    assert_eq!(pending.len(), 45);
    assert_eq!(pending[0].id, ids[0]);
    assert!(j.pending(&grant("@other:server"), 103).unwrap().is_empty());
    g.revoke();
    assert!(j.pending(&g, 103).is_err());
}
#[test]
fn new_preference_preview_invalidates_old_confirmation() {
    let (_, mut j) = journal();
    let g = grant("@p:server");
    let s = Server::new(0);
    let old = j
        .prepare(&g, intent().action(&s.activity), 1, 100, 120)
        .unwrap();
    let next = Intent::Join {
        preferences: Preferences {
            earliest: 18,
            latest: 23,
            group: 1,
        },
    };
    j.prepare(&g, next.action(&s.activity), 1, 100, 120)
        .unwrap();
    assert!(j.confirm(&g, &old.id, &s, 101).is_err());
    assert_eq!(s.calls.load(Ordering::SeqCst), 0);
}
#[test]
fn owner_cannot_submit_participant_intent() {
    assert!(
        intent()
            .validate(&activity(), "@owner:server", 100)
            .is_err()
    );
}
#[test]
fn reply_action_id_is_distinct_from_invitation_reference() {
    let (_, mut j) = journal();
    let g = grant("@p:server");
    let a = activity();
    let i = Intent::Reply {
        invitation_id: "invite-123".into(),
        invitation_event: "$invite".into(),
        accept: true,
    };
    let op = j.prepare(&g, i.action(&a), 1, 100, 120).unwrap();
    let wire = i.wire(&op);
    assert_eq!(wire["operation_id"], "invite-123");
    assert_eq!(wire["action_id"], op.id);
    assert_ne!(wire["action_id"], wire["operation_id"]);
}
