use buwei_host_core::{planning::*, roster, *};
fn clock() -> u64 {
    calendar::parse("2026-10-12 12:00").unwrap()
}
fn basics() -> ActivityBasics {
    ActivityBasics {
        title: "读书会".into(),
        template: "reading".into(),
        start: "2026-10-13 19:30".into(),
        end: "2026-10-13 21:30".into(),
        location: "社区阅览室".into(),
        capacity: 6,
        description: String::new(),
    }
}
fn proposal(field: &str, value: &str, source: &str) -> PlanProposal {
    PlanProposal {
        updates: vec![FieldUpdate {
            field: field.into(),
            value: value.into(),
            source: source.into(),
        }],
        ..Default::default()
    }
}
#[test]
fn correction_preserves_unmentioned_basics_and_reopens_confirmation() {
    let mut d = ActivityPlanDraft::new("@owner:test", "明天办读书会", clock()).unwrap();
    d.confirm_basics(basics(), clock()).unwrap();
    let old = d.creation_digest(clock()).unwrap();
    d.merge(
        "这次到十点结束",
        proposal("end", "2026-10-13 22:00", "到十点结束"),
        clock(),
    )
    .unwrap();
    assert_eq!(d.basics.capacity, 6);
    assert_eq!(d.basics.start, basics().start);
    assert_eq!(d.basics.location, basics().location);
    assert!(d.creation_digest(clock()).is_err());
    d.confirm_basics(d.basics.clone(), clock()).unwrap();
    assert_ne!(old, d.creation_digest(clock()).unwrap());
}
#[test]
fn unknown_required_fields_are_questions_and_suggestions_do_not_create() {
    let mut d = ActivityPlanDraft::new("@owner:test", "办场读书会", clock()).unwrap();
    d.merge(
        "办场读书会",
        PlanProposal {
            updates: vec![
                FieldUpdate {
                    field: "title".into(),
                    value: "读书会".into(),
                    source: "读书会".into(),
                },
                FieldUpdate {
                    field: "template".into(),
                    value: "reading".into(),
                    source: "读书会".into(),
                },
            ],
            agenda: vec!["交流读书笔记".into()],
            reminders: vec!["提前一天确认人数".into()],
            ..Default::default()
        },
        clock(),
    )
    .unwrap();
    assert_eq!(d.basics.questions().len(), 3);
    assert!(d.creation_digest(clock()).is_err());
    assert_eq!(d.status, PlanStatus::Draft);
    assert!(d.activity_id.is_none());
}
#[test]
fn invalid_proposal_is_atomic_and_has_no_identity_or_permissions() {
    let mut d = ActivityPlanDraft::new("@owner:test", "六人", clock()).unwrap();
    let old = d.clone();
    assert!(
        d.merge("六人", proposal("capacity", "31", "六人"), clock())
            .is_err()
    );
    assert_eq!(old, d);
    assert!(
        d.merge("六人", proposal("owner", "@evil:test", "六人"), clock())
            .is_err()
    );
    assert_eq!(old, d);
    assert!(
        d.merge("六人", proposal("capacity", "6", "八人"), clock())
            .is_err()
    );
    assert_eq!(old, d);
}
#[test]
fn plan_store_isolates_accounts_and_refuses_stale_revision() {
    let path = std::env::temp_dir().join(format!("buwei-plans-{}.db", action_receipts::new_id()));
    let a = PlanStore::open(&path, "@a:test").unwrap();
    let b = PlanStore::open(&path, "@b:test").unwrap();
    let mut d = ActivityPlanDraft::new("@a:test", "准备活动", clock()).unwrap();
    a.save(&mut d, None).unwrap();
    assert!(b.load(&d.id).is_err());
    assert!(b.list().unwrap().is_empty());
    assert!(b.save(&mut d, Some(1)).is_err());
    let mut old = d.clone();
    a.save(&mut d, Some(1)).unwrap();
    assert!(a.save(&mut old, Some(1)).is_err());
    a.record_call("call1", &d.id).unwrap();
    assert_eq!(a.completed_call("call1").unwrap().unwrap().id, d.id);
    assert!(b.completed_call("call1").unwrap().is_none());
    drop(a);
    drop(b);
    let _ = std::fs::remove_file(path);
}
fn activity() -> Activity {
    let mut a = Activity::new(
        "@owner:test".into(),
        "!room:test".into(),
        "读书会".into(),
        6,
        19,
        21,
    )
    .unwrap();
    let b = basics();
    a.start = calendar::parse(&b.start).unwrap();
    a.end = calendar::parse(&b.end).unwrap();
    a.metadata = Some(calendar::Metadata {
        activity_id: "a".repeat(32),
        template: b.template,
        location: b.location,
        description: String::new(),
        archived: false,
        metrics: Default::default(),
    });
    a.add_person(
        "@a:test".into(),
        " =HYPERLINK(\"bad\")".into(),
        Preferences {
            earliest: a.start,
            latest: a.end,
            group: 2,
        },
        true,
    )
    .unwrap();
    a.add_person(
        "@b:test".into(),
        "成员乙".into(),
        Preferences {
            earliest: a.start,
            latest: a.end,
            group: 1,
        },
        true,
    )
    .unwrap();
    a
}
#[test]
fn roster_is_owner_only_fresh_and_escaped_for_spreadsheets() {
    let a = activity();
    assert!(roster::csv(&a, "@a:test", clock(), clock()).is_err());
    assert!(roster::csv(&a, &a.owner, clock(), clock() - 121).is_err());
    let bytes = roster::csv(&a, &a.owner, clock(), clock()).unwrap();
    assert!(bytes.starts_with(&[239, 187, 191]));
    let text = String::from_utf8(bytes).unwrap();
    assert!(text.contains("' =HYPERLINK("));
    assert!(text.contains("\"'@a:test\""));
    assert_eq!(text.lines().count(), 3);
    let member = roster::visible(&a, "@a:test");
    assert!(member.contains("@a:test"));
    assert!(!member.contains("@b:test"));
    assert!(!member.contains("成员乙"));
}
#[test]
fn capacity_and_cross_midnight_validation_remain_deterministic() {
    let mut b = basics();
    b.start = "2026-10-13 23:30".into();
    b.end = "2026-10-14 01:00".into();
    assert!(b.ready(clock()).is_ok());
    b.capacity = 31;
    assert!(b.ready(clock()).is_err());
    b.capacity = 6;
    b.end = b.start.clone();
    assert!(b.ready(clock()).is_err());
}
