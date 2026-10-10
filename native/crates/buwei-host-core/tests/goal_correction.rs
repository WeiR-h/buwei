use buwei_host_core::{assistance::*, goal_correction::*, intent_feedback::*, proactive::*, *};
fn at(s: &str) -> u64 {
    calendar::parse(s).unwrap()
}
fn fixture() -> (std::path::PathBuf, Activity, u64, GoalInput) {
    let root = std::env::temp_dir().join(format!("buwei-correction-{}", action_receipts::new_id()));
    let mut a = Activity::new(
        "@owner:test".into(),
        "!reading:test".into(),
        "读书会".into(),
        2,
        19,
        21,
    )
    .unwrap();
    a.start = at("2026-10-11 19:00");
    a.end = at("2026-10-11 21:00");
    a.metadata = Some(calendar::Metadata {
        activity_id: "c".repeat(32),
        template: "reading".into(),
        location: "测试地点".into(),
        description: String::new(),
        archived: false,
        metrics: Default::default(),
    });
    let goal = GoalInput {
        title: "周末读书会".into(),
        kind: GoalKind::Participate,
        activity_id: None,
        template: "reading".into(),
        earliest: a.start,
        latest: at("2026-10-11 20:00"),
        group: 1,
        target: None,
        check_at: None,
        recurrence_days: None,
        preparation_hours: 24,
        availability: None,
    };
    (root, a, at("2026-10-10 12:00"), goal)
}
fn change() -> FieldChange {
    FieldChange {
        field: "latest".into(),
        value: "2026-10-11 21:00".into(),
        evidence: "能待到九点".into(),
    }
}
#[test]
fn correction_is_sparse_private_durable_and_requires_a_separate_registration() {
    let (root, mut a, n, input) = fixture();
    let mut s = IntentStore::open(&root, "@member:test").unwrap();
    a.join_own(
        "@member:test".into(),
        "测试成员".into(),
        Preferences {
            earliest: input.earliest,
            latest: input.latest,
            group: 1,
        },
    )
    .unwrap();
    let old = serde_json::to_value(&a).unwrap();
    let g = s.save_goal(None, input.clone(), &[a.clone()], n).unwrap();
    let p = s
        .prepare_correction(
            &g.id,
            "这次能待到九点，还是我一个人",
            vec![change()],
            vec![],
            &[a.clone()],
            n,
        )
        .unwrap();
    assert_eq!(s.load().unwrap().goals[0].input, input);
    drop(s);
    let mut s = IntentStore::open(&root, "@member:test").unwrap();
    let changed = s
        .confirm_correction(&p.id, FeedbackScope::ThisOccasion, &[a.clone()], n + 1)
        .unwrap();
    assert_eq!(changed.id, g.id);
    assert_eq!(changed.revision, g.revision + 1);
    assert_eq!(changed.input.latest, a.end);
    assert_eq!(changed.input.earliest, input.earliest);
    assert_eq!(changed.input.template, input.template);
    assert_eq!(changed.input.group, input.group);
    assert_eq!(serde_json::to_value(&a).unwrap(), old);
    assert_eq!(
        s.load().unwrap().preferences,
        PersonalPreferences::default()
    );
    assert!(
        s.confirm_correction(&p.id, FeedbackScope::ThisOccasion, &[a.clone()], n + 2)
            .is_err()
    );
    let other = IntentStore::open(&root, "@other:test").unwrap();
    assert!(other.load().unwrap().corrections.is_empty());
    let task = s
        .prepare_registration_update(&g.id, &a, &[], n + 3)
        .unwrap();
    assert_eq!(task.registration_update_sequence, Some(a.people[0].joined));
    let prior = a.people[0].joined;
    a.join_own(
        "@member:test".into(),
        "测试成员".into(),
        Preferences {
            earliest: changed.input.earliest,
            latest: changed.input.latest,
            group: changed.input.group,
        },
    )
    .unwrap();
    assert_eq!(a.people[0].joined, prior);
    assert_eq!(a.people[0].preferences.latest, a.end);
}
#[test]
fn expired_changed_missing_or_cross_account_confirmation_is_rejected() {
    let (root, a, n, input) = fixture();
    let mut s = IntentStore::open(&root, "@member:test").unwrap();
    let g = s.save_goal(None, input.clone(), &[a.clone()], n).unwrap();
    let p = s
        .prepare_correction(&g.id, "能待到九点", vec![change()], vec![], &[a.clone()], n)
        .unwrap();
    assert!(
        s.confirm_correction(&p.id, FeedbackScope::ThisOccasion, &[a.clone()], n + 600)
            .is_err()
    );
    let p = s
        .prepare_correction(
            &g.id,
            "能待到九点",
            vec![change()],
            vec![],
            &[a.clone()],
            n + 1,
        )
        .unwrap();
    let mut edited = input;
    edited.group = 2;
    s.save_goal(Some(&g.id), edited, &[a.clone()], n + 2)
        .unwrap();
    assert!(
        s.confirm_correction(&p.id, FeedbackScope::ThisOccasion, &[a.clone()], n + 3)
            .is_err()
    );
    let p = s
        .prepare_correction(
            &g.id,
            "能待到九点",
            vec![],
            vec!["九点是上午还是晚上？".into()],
            &[a.clone()],
            n + 4,
        )
        .unwrap();
    assert!(
        s.confirm_correction(&p.id, FeedbackScope::ThisOccasion, &[a.clone()], n + 5)
            .is_err()
    );
    assert!(
        IntentStore::open(&root, "@other:test")
            .unwrap()
            .confirm_correction(&p.id, FeedbackScope::ThisOccasion, &[a], n + 5)
            .is_err()
    );
}
#[test]
fn explicit_long_term_updates_only_changed_fields_and_is_reversible() {
    let (root, a, n, input) = fixture();
    let mut s = IntentStore::open(&root, "@member:test").unwrap();
    let preferences = PersonalPreferences {
        template: Some("reading".into()),
        group: Some(2),
        weekdays: vec![5],
        earliest_minute: Some(19 * 60),
        latest_minute: Some(20 * 60),
        ..Default::default()
    };
    s.save_preferences(preferences, n).unwrap();
    let before = s.load().unwrap().preferences;
    let g = s.save_goal(None, input.clone(), &[a.clone()], n).unwrap();
    let p = s
        .prepare_correction(&g.id, "能待到九点", vec![change()], vec![], &[a.clone()], n)
        .unwrap();
    s.confirm_correction(&p.id, FeedbackScope::LongTerm, &[a], n + 1)
        .unwrap();
    let state = s.load().unwrap();
    assert_eq!(state.preferences.group, Some(2));
    assert_eq!(state.preferences.template, before.template);
    assert_eq!(state.preferences.weekdays, vec![5]);
    assert_eq!(state.preferences.earliest_minute, Some(19 * 60));
    assert_eq!(state.preferences.latest_minute, Some(21 * 60));
    let f = state.feedback.last().unwrap();
    s.undo_feedback(&f.id, n + 2).unwrap();
    assert_eq!(s.load().unwrap().preferences, before);
    assert_eq!(s.load().unwrap().goals[0].input, input);
}
#[test]
fn hostile_fields_duplicate_changes_and_unsupported_evidence_fail_closed() {
    let (_, _, _, input) = fixture();
    let mut c = change();
    c.field = "execute".into();
    assert!(corrected_input(&input, "能待到九点", &[c]).is_err());
    let mut c = change();
    c.evidence = "不存在的原话".into();
    assert!(corrected_input(&input, "能待到九点", &[c]).is_err());
    assert!(corrected_input(&input, "能待到九点", &[change(), change()]).is_err());
    let c = FieldChange {
        field: "group".into(),
        value: "3".into(),
        evidence: "能待到九点".into(),
    };
    assert!(corrected_input(&input, "能待到九点", &[c]).is_err());
    assert!(serde_json::from_value::<FieldChange>(serde_json::json!({"field":"group","value":"1","evidence":"一个人","account":"@other:test"})).is_err());
}
#[test]
fn registration_update_continues_original_task_and_rejects_a_reserved_place() {
    let (root, mut a, n, mut input) = fixture();
    input.latest = a.end;
    let mut s = IntentStore::open(&root, "@member:test").unwrap();
    let g = s.save_goal(None, input.clone(), &[a.clone()], n).unwrap();
    let fact = VerifiedFacts {
        activity: a.clone(),
        observed_at: n,
        complete: true,
        accessible: true,
        has_pending_operation: false,
        automation_pause: None,
    };
    s.refresh_cards(&[fact], n).unwrap();
    let card = s
        .visible_cards(n)
        .unwrap()
        .into_iter()
        .find(|c| c.kind == CardKind::Opportunity)
        .unwrap();
    let original = s.start_task(&card.id, &card.fingerprint, n).unwrap();
    a.join_own(
        "@member:test".into(),
        "测试成员".into(),
        Preferences {
            earliest: a.start,
            latest: a.end - 3600,
            group: 1,
        },
    )
    .unwrap();
    let continued = s
        .prepare_registration_update(&g.id, &a, &[], n + 2)
        .unwrap();
    assert_eq!(continued.id, original.id);
    assert_eq!(
        continued.registration_update_sequence,
        Some(a.people[0].joined)
    );
    let mut reserved = a.clone();
    reserved.invitations.push(Invitation {
        operation_id: "invite".into(),
        recipient: "@member:test".into(),
        room: a.room.clone(),
        digest: "d".repeat(64),
        until: n + 60,
        delivery: Delivery::Delivered,
        server_event: Some("$server:test".into()),
        reply: Reply::Pending,
    });
    assert!(
        s.prepare_registration_update(&g.id, &reserved, &[], n + 3)
            .is_err()
    );
    a.people[0].status = PersonStatus::Confirmed;
    assert!(
        s.prepare_registration_update(&g.id, &a, &[], n + 3)
            .is_err()
    );
}
