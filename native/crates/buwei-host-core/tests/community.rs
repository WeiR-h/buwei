use action_receipts::{Action, Authority, Journal, Status, new_id};
use buwei_host_core::{
    Activity, PersonStatus, Preferences, Store,
    automation::{Policy, Settings},
    calendar::{self, Metadata},
    catalog::Catalog,
    facts,
    participation::Intent,
};
use serde_json::json;

fn clock() -> u64 {
    calendar::parse("2026-10-10 10:00").unwrap()
}
fn activity(capacity: u8) -> Activity {
    let mut a = Activity::new(
        "@owner:server".into(),
        format!("!{}", new_id()),
        "羽毛球".into(),
        capacity,
        19,
        21,
    )
    .unwrap();
    a.start = calendar::parse("2026-10-10 23:30").unwrap();
    a.end = calendar::parse("2026-10-11 01:15").unwrap();
    a.metadata = Some(Metadata {
        activity_id: new_id(),
        template: "badminton".into(),
        location: "示例球馆".into(),
        description: "自备球拍".into(),
        archived: false,
        metrics: Default::default(),
    });
    a.validate().unwrap();
    a
}
fn p(a: &Activity, n: u8) -> Preferences {
    Preferences {
        earliest: a.start,
        latest: a.end,
        group: n,
    }
}
fn add(a: &mut Activity, who: &str, n: u8) {
    let preferences = p(a, n);
    a.join_own(format!("@{who}:server"), who.into(), preferences)
        .unwrap();
}
fn grant(a: &Activity) -> action_receipts::Grant {
    let auth = Authority::default();
    auth.set_account(Some(&a.owner));
    auth.grant("buwei", &["create", "invite", "read"], clock(), 3600)
        .unwrap()
}
fn reserve(a: &mut Activity) -> String {
    let g = grant(a);
    let mut j = Journal::open(":memory:").unwrap();
    let mut op = j
        .prepare(
            &g,
            Action {
                permission: "invite".into(),
                target: a.room.clone(),
                summary: "整组邀请".into(),
                payload: json!({"person":a.candidate().unwrap(),"until":clock()+600}),
            },
            a.revision,
            clock(),
            120,
        )
        .unwrap();
    op.status = Status::Dispatching;
    a.reserve(&op, clock()).unwrap();
    a.record_delivery(&op.id, &op.account, &op.action.target, "$event", &op.digest)
        .unwrap();
    op.id
}
#[test]
fn beijing_minutes_dates_roundtrip() {
    for value in ["2026-10-10 19:30", "2026-10-11 00:05", "2024-02-29 08:01"] {
        assert_eq!(calendar::display(calendar::parse(value).unwrap()), value);
    }
    assert_eq!(calendar::parse("2026-10-10 10:00").unwrap() % 86400, 7200);
}
#[test]
fn malformed_dates_are_rejected() {
    for s in [
        "2026-02-29 19:30",
        "2026-13-10 19:30",
        "2026-10-10 24:01",
        "2026-10-10 19:60",
        "19:30",
        "2026-10-10 7:30",
    ] {
        assert!(calendar::parse(s).is_err(), "{s}");
    }
}
#[test]
fn cross_midnight_is_explicit() {
    let a = activity(8);
    assert_eq!(a.end - a.start, 105 * 60);
    let mut bad = a.clone();
    bad.end = bad.start + 49 * 3600;
    assert!(bad.validate().is_err());
}
#[test]
fn legacy_hours_are_not_assigned_today() {
    let a = Activity::new("@o:s".into(), "!r".into(), "历史".into(), 1, 19, 21).unwrap();
    let wire = serde_json::to_value(&a).unwrap();
    assert!(wire.get("metadata").is_none());
    assert_eq!(calendar::display(a.start), "19:00");
}
#[test]
fn dated_activity_rejects_hour_only_signup() {
    let mut a = activity(8);
    assert!(
        a.join_own(
            "@p:s".into(),
            "p".into(),
            Preferences {
                earliest: 17,
                latest: 23,
                group: 2
            }
        )
        .is_err()
    );
    assert!(a.people.is_empty());
}
#[test]
fn group_reservation_and_acceptance_are_whole() {
    let mut a = activity(8);
    add(&mut a, "first", 8);
    let id = reserve(&mut a);
    assert_eq!(a.held(), 8);
    assert_eq!(a.free(), 0);
    a.receive_reply(
        &id,
        "@first:server",
        &a.room.clone(),
        "$event",
        true,
        clock() + 5,
        clock() + 6,
    )
    .unwrap();
    assert_eq!(a.confirmed(), 8);
    assert_eq!(a.held(), 0);
    a.cancel_own("@first:server").unwrap();
    assert_eq!(a.free(), 8);
}
#[test]
fn large_group_is_skipped_without_losing_position() {
    let mut a = activity(2);
    add(&mut a, "first", 3);
    add(&mut a, "second", 2);
    assert_eq!(a.candidate(), Some("@second:server"));
    let first = a.people[0].joined;
    let id = reserve(&mut a);
    a.receive_reply(
        &id,
        "@second:server",
        &a.room.clone(),
        "$event",
        false,
        clock() + 5,
        clock() + 6,
    )
    .unwrap();
    a.capacity = 3;
    assert_eq!(a.candidate(), Some("@first:server"));
    assert_eq!(a.people[0].joined, first);
}
#[test]
fn groups_have_one_to_eight_people() {
    let a = activity(30);
    for n in [0, 9, 31] {
        assert!(p(&a, n).validate().is_err());
    }
    for n in 1..=8 {
        p(&a, n).validate().unwrap();
    }
}
#[test]
fn thirty_signup_limit_and_seats_hold() {
    let mut a = activity(30);
    for n in 0..30 {
        add(&mut a, &format!("p{n}"), 1);
    }
    let preferences = p(&a, 1);
    assert!(
        a.join_own("@extra:s".into(), "extra".into(), preferences)
            .is_err()
    );
    a.people
        .iter_mut()
        .for_each(|p| p.status = PersonStatus::Confirmed);
    a.validate().unwrap();
    assert_eq!(a.confirmed(), 30);
    assert_eq!(a.candidate(), None);
}
#[test]
fn cancelled_registration_frees_a_registration_slot_without_losing_history() {
    let mut a = activity(30);
    for n in 0..30 {
        add(&mut a, &format!("p{n}"), 1);
    }
    a.cancel_own("@p0:server").unwrap();
    add(&mut a, "new", 1);
    assert_eq!(a.effective_registrations(), 30);
    assert_eq!(a.people.len(), 31);
    assert!(
        a.join_own("@p0:server".into(), "p0".into(), p(&a, 1))
            .is_err()
    );
    assert_eq!(a.people[0].status, PersonStatus::Cancelled);
    a.validate().unwrap();
}
#[test]
fn waiting_person_can_cancel_and_rejoin_at_tail() {
    let mut a = activity(8);
    add(&mut a, "first", 2);
    add(&mut a, "second", 1);
    a.cancel_own("@first:server").unwrap();
    add(&mut a, "first", 2);
    assert_eq!(a.candidate(), Some("@second:server"));
}
#[test]
fn unknown_group_is_not_expired_or_reallocated() {
    let mut a = activity(8);
    add(&mut a, "first", 8);
    let g = grant(&a);
    let mut j = Journal::open(":memory:").unwrap();
    let mut op = j
        .prepare(
            &g,
            Action {
                permission: "invite".into(),
                target: a.room.clone(),
                summary: "邀请".into(),
                payload: json!({"person":a.candidate(),"until":clock()+600}),
            },
            a.revision,
            clock(),
            120,
        )
        .unwrap();
    op.status = Status::Dispatching;
    a.reserve(&op, clock()).unwrap();
    a.mark_unknown(&op.id).unwrap();
    assert_eq!(a.expire(clock() + 1000), 0);
    assert_eq!(a.held(), 8);
    assert_eq!(a.candidate(), None);
}
#[test]
fn new_signup_stops_at_start_but_cancellation_is_possible() {
    let mut a = activity(8);
    add(&mut a, "first", 2);
    a.people[0].status = PersonStatus::Confirmed;
    assert!(
        Intent::Join {
            preferences: p(&a, 1)
        }
        .validate(&a, "@new:s", a.start)
        .is_err()
    );
    assert!(
        Intent::Cancel
            .validate(&a, "@first:server", a.start)
            .is_ok()
    );
    assert!(Intent::Cancel.validate(&a, "@first:server", a.end).is_err());
}
#[test]
fn policy_binds_actor_room_dates_quiet_hours_and_expiry() {
    let mut a = activity(8);
    add(&mut a, "first", 2);
    let g = grant(&a);
    let policy = Policy::issue(&g, &a, Settings::default(), clock(), clock() + 3600).unwrap();
    policy.check(&g, &a, clock()).unwrap();
    assert!(policy.check(&g, &a, clock() + 3600).is_err());
    let mut quiet = policy.clone();
    quiet.settings.quiet_start = 11;
    assert!(quiet.check(&g, &a, clock()).is_err());
    let mut other = a.clone();
    other.room = "!other".into();
    assert!(policy.check(&g, &other, clock()).is_err());
    g.revoke();
    assert!(policy.check(&g, &a, clock()).is_err());
}
#[test]
fn automation_quota_is_durable_and_deduplicated() {
    let path = std::env::temp_dir().join(format!("quota-{}.db", new_id()));
    let id = new_id();
    let op = new_id();
    let digest = "a".repeat(64);
    let mut s = Store::open(&path).unwrap();
    s.audit_policy(&id, &op, &digest, 1).unwrap();
    s.audit_policy(&id, &op, &digest, 1).unwrap();
    drop(s);
    let mut s = Store::open(path).unwrap();
    assert!(s.audit_policy(&id, &new_id(), &digest, 1).is_err());
    assert!(s.audit_policy(&id, &op, &"b".repeat(64), 1).is_err());
}
#[test]
fn five_activity_states_and_cursors_are_isolated() {
    let root = std::env::temp_dir().join(format!("catalog-{}", new_id()));
    let mut c = Catalog::open(&root).unwrap();
    let mut list = vec![];
    for _ in 0..5 {
        let a = activity(8);
        let path = c.path(&a.metadata.as_ref().unwrap().activity_id).unwrap();
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut s = Store::open(path).unwrap();
        s.cache_verified_snapshot(&a).unwrap();
        s.save_sync_checkpoint(&a.room, "$cursor").unwrap();
        c.register(&a, clock()).unwrap();
        list.push(a);
    }
    assert_eq!(c.list().unwrap().len(), 5);
    c.register(&list[0], clock()).unwrap();
    assert!(c.ensure_capacity(clock()).is_err());
    let a = activity(8);
    let path = c.path(&a.metadata.as_ref().unwrap().activity_id).unwrap();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    Store::open(path)
        .unwrap()
        .cache_verified_snapshot(&a)
        .unwrap();
    assert!(c.register(&a, clock()).is_err());
    let first = &list[0];
    let mut s = Store::open(
        c.path(&first.metadata.as_ref().unwrap().activity_id)
            .unwrap(),
    )
    .unwrap();
    s.update(first.revision, |a| {
        a.metadata.as_mut().unwrap().archived = true;
        a.revision += 1;
        Ok(())
    })
    .unwrap();
    c.register(&a, clock()).unwrap();
    let second = &list[1];
    let other = Store::open(
        c.path(&second.metadata.as_ref().unwrap().activity_id)
            .unwrap(),
    )
    .unwrap();
    assert_eq!(other.load().unwrap().unwrap().revision, 1);
    assert!(other.sync_checkpoint(&first.room).unwrap().is_none());
}
#[test]
fn catalog_corruption_cannot_silently_free_activity_capacity() {
    let root = std::env::temp_dir().join(format!("catalog-missing-{}", new_id()));
    let mut catalog = Catalog::open(&root).unwrap();
    let a = activity(8);
    let path = catalog.path(&a.metadata.as_ref().unwrap().activity_id).unwrap();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    Store::open(&path).unwrap().cache_verified_snapshot(&a).unwrap();
    catalog.register(&a, clock()).unwrap();
    std::fs::rename(&path, path.with_extension("backup")).unwrap();
    assert!(catalog.list().is_err());
    assert!(catalog.ensure_capacity(clock()).is_err());
    assert!(!path.exists());
}
#[test]
fn questions_and_notes_cannot_invent_unrecorded_facts() {
    let a = activity(8);
    let valid = facts::render(&a, &["time".into(), "location".into()]).unwrap();
    assert!(valid.contains("〔依据：time〕"));
    assert!(facts::render(&a, &["atmosphere".into()]).is_err());
    assert!(facts::render(&a, &["time".into(), "time".into()]).is_err());
}
#[test]
fn personal_facts_only_describe_the_bound_account() {
    let mut a = activity(8);
    add(&mut a, "first", 2);
    add(&mut a, "second", 1);
    let first = facts::render_for(&a, "@first:server", &["my_signup".into()]).unwrap();
    let second = facts::render_for(&a, "@second:server", &["my_signup".into()]).unwrap();
    assert!(first.contains("2 人") && first.contains("第 1 位"));
    assert!(second.contains("1 人") && second.contains("第 2 位"));
    assert!(!first.contains('@') && !first.contains("second"));
    assert!(
        facts::render_for(&a, "@unknown:s", &["my_signup".into()])
            .unwrap()
            .contains("尚未报名")
    );
}
#[test]
fn recap_counts_verified_replies_and_whole_groups_once() {
    let mut a = activity(8);
    add(&mut a, "first", 2);
    let id = reserve(&mut a);
    a.record_invitation_time(&id, clock() + 1).unwrap();
    let revision = a.revision;
    a.record_invitation_time(&id, clock() + 1).unwrap();
    assert_eq!(a.revision, revision);
    assert!(a.record_invitation_time(&id, clock() + 2).is_err());
    a.receive_reply(
        &id,
        "@first:server",
        &a.room.clone(),
        "$event",
        true,
        clock() + 7,
        clock() + 8,
    )
    .unwrap();
    assert!(
        a.receive_reply(
            &id,
            "@first:server",
            &a.room.clone(),
            "$event",
            true,
            clock() + 7,
            clock() + 8
        )
        .is_err()
    );
    a.cancel_own("@first:server").unwrap();
    let m = &a.metadata.as_ref().unwrap().metrics;
    assert_eq!(m.successful_invitations, 1);
    assert_eq!(m.cancelled_people, 2);
    assert_eq!(m.response_seconds, 6);
    assert_eq!(m.timed_responses, 1);
}
