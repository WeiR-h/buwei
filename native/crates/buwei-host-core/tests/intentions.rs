use buwei_host_core::{
    Activity,
    assistance::*,
    calendar::{self, Metadata},
};
const OWNER: &str = "@organizer:matrix.rinx.chat";
const MEMBER: &str = "@member:matrix.rinx.chat";
fn root() -> std::path::PathBuf {
    std::env::temp_dir().join(format!("buwei-intention-{}", action_receipts::new_id()))
}
fn activity() -> Activity {
    let mut a = Activity::new(
        OWNER.into(),
        "!intentions:matrix.rinx.chat".into(),
        "测试活动".into(),
        6,
        19,
        21,
    )
    .unwrap();
    a.start = calendar::parse("2026-10-10 19:30").unwrap();
    a.end = a.start + 7200;
    a.metadata = Some(Metadata {
        activity_id: "a".repeat(32),
        template: "badminton".into(),
        location: "测试场馆".into(),
        description: String::new(),
        archived: false,
        metrics: Default::default(),
    });
    a
}
fn goal(a: &Activity, kind: GoalKind) -> GoalInput {
    GoalInput {
        title: "本周活动目标".into(),
        kind,
        activity_id: Some("a".repeat(32)),
        template: "badminton".into(),
        earliest: a.start,
        latest: a.end,
        group: 2,
        target: if kind == GoalKind::Organize {
            Some(6)
        } else {
            None
        },
        check_at: if kind == GoalKind::Organize {
            Some(a.start - 3600)
        } else {
            None
        },
        recurrence_days: None,
        preparation_hours: 24,
        availability: None,
    }
}
#[test]
fn goal_validation_binds_owner_capacity_and_date() {
    let a = activity();
    let g = goal(&a, GoalKind::Organize);
    assert!(g.validate(OWNER, &[a.clone()], a.start - 7200).is_ok());
    assert!(g.validate(MEMBER, &[a.clone()], a.start - 7200).is_err());
    let mut g = g;
    g.target = Some(7);
    assert!(g.validate(OWNER, &[a.clone()], a.start - 7200).is_err());
    g.target = Some(6);
    g.check_at = None;
    assert!(g.validate(OWNER, &[a.clone()], a.start - 7200).is_err());
    g.check_at = Some(a.start);
    g.latest = g.earliest;
    assert!(g.validate(OWNER, &[a.clone()], a.start - 7200).is_err());
}
#[test]
fn accounts_are_isolated_and_reopening_preserves_revision() {
    let p = root();
    let a = activity();
    let clock = a.start - 7200;
    let mut s = IntentStore::open(&p, OWNER).unwrap();
    let g = s
        .save_goal(None, goal(&a, GoalKind::Organize), &[a.clone()], clock)
        .unwrap();
    drop(s);
    let mut s = IntentStore::open(&p, OWNER).unwrap();
    let mut input = g.input.clone();
    input.title = "修改目标".into();
    let newer = s
        .save_goal(Some(&g.id), input, &[a.clone()], clock + 1)
        .unwrap();
    assert_eq!(newer.revision, 2);
    assert!(
        IntentStore::open(&p, MEMBER)
            .unwrap()
            .load()
            .unwrap()
            .goals
            .is_empty()
    );
    s.set_goal_status(&g.id, GoalStatus::Paused, clock + 2)
        .unwrap();
    assert_eq!(s.load().unwrap().goals[0].status, GoalStatus::Paused);
    s.delete_goal(&g.id).unwrap();
    assert!(s.load().unwrap().goals.is_empty());
}
#[test]
fn field_sources_distinguish_confirmed_preferences_from_this_occasion() {
    let p = root();
    let a = activity();
    let clock = a.start - 7200;
    let mut s = IntentStore::open(&p, MEMBER).unwrap();
    let mut pref = PersonalPreferences::default();
    pref.template = Some("badminton".into());
    pref.group = Some(2);
    s.save_preferences(pref, clock).unwrap();
    let mut i = goal(&a, GoalKind::Participate);
    let g = s.save_goal(None, i.clone(), &[a.clone()], clock).unwrap();
    assert_eq!(
        g.field_sources["template"],
        FieldSource::ConfirmedPreference
    );
    assert_eq!(g.field_sources["group"], FieldSource::ConfirmedPreference);
    assert_eq!(g.field_sources["earliest"], FieldSource::SelfEntered);
    i.group = 1;
    let g = s.save_goal(Some(&g.id), i, &[a], clock + 1).unwrap();
    assert_eq!(g.field_sources["group"], FieldSource::SelfEntered);
    assert_eq!(s.load().unwrap().preferences.group, Some(2));
}
#[test]
fn preferences_require_explicit_values_and_support_cross_midnight() {
    let mut p = PersonalPreferences::default();
    assert!(p.validate().is_ok());
    p.group = Some(9);
    assert!(p.validate().is_err());
    p.group = Some(2);
    p.weekdays = vec![1, 1];
    assert!(p.validate().is_err());
    p.weekdays = vec![5, 6];
    p.earliest_minute = Some(1380);
    p.latest_minute = Some(120);
    assert!(p.validate().is_ok());
    p.latest_minute = None;
    assert!(p.validate().is_err());
}
#[test]
fn confirmed_model_fields_keep_their_source_and_do_not_replace_preferences() {
    let p = root();
    let a = activity();
    let clock = a.start - 7200;
    let mut s = IntentStore::open(&p, MEMBER).unwrap();
    let mut pref = PersonalPreferences::default();
    pref.template = Some("badminton".into());
    pref.group = Some(2);
    s.save_preferences(pref, clock).unwrap();
    let g = s
        .save_goal(None, goal(&a, GoalKind::Participate), &[a], clock)
        .unwrap();
    let g = s
        .confirm_suggestion_sources(&g.id, &["title".into(), "template".into(), "group".into()])
        .unwrap();
    assert_eq!(g.field_sources["title"], FieldSource::ConfirmedSuggestion);
    assert_eq!(
        g.field_sources["template"],
        FieldSource::ConfirmedPreference
    );
    assert_eq!(g.field_sources["group"], FieldSource::ConfirmedPreference);
    assert_eq!(g.field_sources["latest"], FieldSource::SelfEntered);
    assert!(
        s.confirm_suggestion_sources(&g.id, &["execute".into()])
            .is_err()
    );
    assert_eq!(s.load().unwrap().preferences.group, Some(2));
}
#[test]
fn quiet_hours_use_beijing_and_preferences_can_reset() {
    let p = root();
    let mut s = IntentStore::open(&p, MEMBER).unwrap();
    let mut pref = PersonalPreferences::default();
    pref.group = Some(2);
    s.save_preferences(pref, 1).unwrap();
    assert!(
        s.load()
            .unwrap()
            .preferences
            .quiet(calendar::parse("2026-10-10 23:00").unwrap())
    );
    assert!(
        !s.load()
            .unwrap()
            .preferences
            .quiet(calendar::parse("2026-10-10 09:00").unwrap())
    );
    s.reset_preferences().unwrap();
    assert_eq!(s.load().unwrap().preferences.group, None);
}
