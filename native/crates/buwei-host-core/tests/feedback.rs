use buwei_host_core::{assistance::*, intent_feedback::*, proactive::*, *};
fn fixture() -> (std::path::PathBuf, Activity, u64) {
    let root = std::env::temp_dir().join(format!("buwei-feedback-{}", action_receipts::new_id()));
    let mut a = Activity::new(
        "@owner:test".into(),
        "!feedback:test".into(),
        "羽毛球".into(),
        6,
        19,
        21,
    )
    .unwrap();
    a.start = calendar::parse("2026-10-10 19:30").unwrap();
    a.end = a.start + 7200;
    a.metadata = Some(calendar::Metadata {
        activity_id: action_receipts::new_id(),
        template: "badminton".into(),
        location: "测试球馆".into(),
        description: String::new(),
        archived: false,
        metrics: Default::default(),
    });
    let clock = a.start - 3600;
    (root, a, clock)
}
fn input(a: &Activity) -> GoalInput {
    GoalInput {
        title: "本周想打球".into(),
        kind: GoalKind::Participate,
        activity_id: None,
        template: "badminton".into(),
        earliest: a.start,
        latest: a.end,
        group: 2,
        target: None,
        check_at: None,
        recurrence_days: None,
        preparation_hours: 24,
    }
}
#[test]
fn this_occasion_does_not_change_long_term_and_can_undo() {
    let (p, a, n) = fixture();
    let mut s = IntentStore::open(&p, "@member:test").unwrap();
    let mut pref = PersonalPreferences::default();
    pref.group = Some(2);
    s.save_preferences(pref.clone(), n).unwrap();
    let g = s.save_goal(None, input(&a), &[a.clone()], n).unwrap();
    let before = s.load().unwrap().preferences;
    let f = s
        .feedback(
            &g.id,
            "badminton".into(),
            1,
            FeedbackScope::ThisOccasion,
            &[a],
            n,
        )
        .unwrap();
    assert_eq!(s.load().unwrap().goals[0].input.group, 1);
    assert_eq!(s.load().unwrap().preferences, before);
    s.undo_feedback(&f.id, n + 1).unwrap();
    assert_eq!(s.load().unwrap().goals[0].input.group, 2);
    assert!(s.undo_feedback(&f.id, n + 2).is_err());
}
#[test]
fn long_term_requires_explicit_scope_and_old_undo_cannot_overwrite_new_preferences() {
    let (p, a, n) = fixture();
    let mut s = IntentStore::open(&p, "@member:test").unwrap();
    let g = s.save_goal(None, input(&a), &[a.clone()], n).unwrap();
    let f = s
        .feedback(&g.id, "reading".into(), 1, FeedbackScope::LongTerm, &[a], n)
        .unwrap();
    assert_eq!(s.load().unwrap().preferences.group, Some(1));
    let mut pref = s.load().unwrap().preferences;
    pref.group = Some(3);
    s.save_preferences(pref, n + 1).unwrap();
    assert!(s.undo_feedback(&f.id, n + 2).is_err());
    assert_eq!(s.load().unwrap().preferences.group, Some(3));
}
#[test]
fn recurrence_prepares_a_draft_card_from_confirmed_schedule_only() {
    let (p, a, n) = fixture();
    let mut s = IntentStore::open(&p, "@owner:test").unwrap();
    let mut i = input(&a);
    i.kind = GoalKind::Organize;
    i.activity_id = Some(a.metadata.as_ref().unwrap().activity_id.clone());
    i.target = Some(6);
    i.check_at = Some(a.start - 60);
    i.recurrence_days = Some(7);
    s.save_goal(None, i, &[a.clone()], n).unwrap();
    let next = a.start + 7 * 86400;
    let f = VerifiedFacts {
        activity: a.clone(),
        observed_at: n,
        complete: false,
        accessible: true,
        automation_pause: None,
        has_pending_operation: false,
    };
    assert!(
        s.refresh_cards(&[f.clone()], next - 86401)
            .unwrap()
            .is_empty()
    );
    let cards = s.refresh_cards(&[f], next - 86400).unwrap();
    assert_eq!(cards.len(), 1);
    assert_eq!(cards[0].action, SuggestedAction::NextDraft);
    assert!(cards[0].reason.contains(&calendar::display(next)));
    assert!(s.load().unwrap().tasks.is_empty());
}

#[test]
fn recovered_next_activity_cannot_complete_a_changed_goal() {
    let (p, a, n) = fixture();
    let mut s = IntentStore::open(&p, "@owner:test").unwrap();
    let mut i = input(&a);
    i.kind = GoalKind::Organize;
    i.activity_id = Some(a.metadata.as_ref().unwrap().activity_id.clone());
    i.target = Some(6);
    i.check_at = Some(a.start - 60);
    i.recurrence_days = Some(7);
    let g = s.save_goal(None, i.clone(), &[a.clone()], n).unwrap();
    let clock = a.start + 6 * 86400;
    let f = VerifiedFacts {
        activity: a.clone(),
        observed_at: n,
        complete: false,
        accessible: true,
        automation_pause: None,
        has_pending_operation: false,
    };
    let card = s.refresh_cards(&[f], clock).unwrap().remove(0);
    let task = s.start_task(&card.id, &card.fingerprint, clock).unwrap();
    let mut next = a.clone();
    next.room = "!next:test".into();
    next.metadata.as_mut().unwrap().activity_id = action_receipts::new_id();
    next.start += 7 * 86400;
    next.end += 7 * 86400;
    // The independently created activity is valid, but no longer satisfies the
    // goal revision that originally authorized this preparatory task.
    s.set_goal_status(&g.id, GoalStatus::Paused, clock).unwrap();
    assert!(s.record_next_activity(&task.id, &next, clock).is_err());
    assert_ne!(
        s.load().unwrap().tasks[0].status,
        buwei_host_core::assistance_tasks::TaskStatus::Completed
    );
}
#[test]
fn a_verified_next_activity_does_not_prepare_the_same_period_again() {
    let (p, a, n) = fixture();
    let mut s = IntentStore::open(&p, "@owner:test").unwrap();
    let mut i = input(&a);
    i.kind = GoalKind::Organize;
    i.activity_id = Some(a.metadata.as_ref().unwrap().activity_id.clone());
    i.target = Some(6);
    i.check_at = Some(a.start - 60);
    i.recurrence_days = Some(7);
    s.save_goal(None, i, &[a.clone()], n).unwrap();
    let clock = a.start + 6 * 86400;
    let f = VerifiedFacts {
        activity: a.clone(),
        observed_at: n,
        complete: false,
        accessible: true,
        automation_pause: None,
        has_pending_operation: false,
    };
    let card = s.refresh_cards(&[f.clone()], clock).unwrap().remove(0);
    let task = s.start_task(&card.id, &card.fingerprint, clock).unwrap();
    let mut next = a.clone();
    next.room = "!next:test".into();
    next.metadata.as_mut().unwrap().activity_id = action_receipts::new_id();
    next.start += 7 * 86400;
    next.end += 7 * 86400;
    s.record_next_activity(&task.id, &next, clock).unwrap();
    assert!(s.refresh_cards(&[f], clock + 1).unwrap().is_empty());
    let task = &s.load().unwrap().tasks[0];
    assert_eq!(
        task.steps[0].status,
        buwei_host_core::assistance_tasks::TaskStatus::Completed
    );
}
