use buwei_host_core::{assistance::*, proactive::*, *};
fn activity(n: u8, kind: &str) -> Activity {
    let mut a = Activity::new(
        "@owner:test".into(),
        format!("!room{n}:test"),
        "周末活动".into(),
        n,
        19,
        21,
    )
    .unwrap();
    a.start = calendar::parse("2026-10-10 19:30").unwrap();
    a.end = a.start + 7200;
    a.metadata = Some(calendar::Metadata {
        activity_id: action_receipts::new_id(),
        template: kind.into(),
        location: "测试场馆".into(),
        description: String::new(),
        archived: false,
        metrics: Default::default(),
    });
    a
}
fn root() -> std::path::PathBuf {
    std::env::temp_dir().join(format!("buwei-proactive-{}", action_receipts::new_id()))
}
fn fact(a: Activity, now: u64) -> VerifiedFacts {
    VerifiedFacts {
        activity: a,
        observed_at: now,
        complete: true,
        accessible: true,
        automation_pause: None,
        has_pending_operation: false,
    }
}
fn goal(a: &Activity, kind: GoalKind, clock: u64) -> GoalInput {
    GoalInput {
        title: "周末想办或参加活动".into(),
        kind,
        activity_id: if kind == GoalKind::Organize {
            Some(a.metadata.as_ref().unwrap().activity_id.clone())
        } else {
            None
        },
        template: a.metadata.as_ref().unwrap().template.clone(),
        earliest: a.start,
        latest: a.end,
        group: 1,
        target: if kind == GoalKind::Organize {
            Some(a.capacity)
        } else {
            None
        },
        check_at: if kind == GoalKind::Organize {
            Some(clock)
        } else {
            None
        },
        recurrence_days: None,
        preparation_hours: 24,
        availability: None,
    }
}
#[test]
fn independent_organizer_fifty_cases() {
    let mut checked = 0;
    for capacity in [1, 2, 6, 8, 30] {
        for case in 0..10 {
            let a = activity(capacity, ["badminton", "boardgame", "reading"][case % 3]);
            let now = a.start - 3600;
            let p = root();
            let mut s = IntentStore::open(&p, "@owner:test").unwrap();
            let mut input = goal(&a, GoalKind::Organize, now);
            if case == 1 {
                input.check_at = Some(now + 60);
            }
            let g = s.save_goal(None, input, &[a.clone()], now).unwrap();
            let mut f = fact(a.clone(), now);
            match case {
                2 => s.set_goal_status(&g.id, GoalStatus::Paused, now).unwrap(),
                3 => {
                    f.complete = false;
                }
                4 => {
                    f.observed_at = now - 121;
                }
                5 => {
                    f.accessible = false;
                }
                6 => {
                    f.activity.end = now;
                }
                7 => {
                    f.activity.metadata.as_mut().unwrap().archived = true;
                }
                8 => {
                    s.delete_goal(&g.id).unwrap();
                }
                9 => {
                    f.observed_at = now + 1;
                }
                _ => {}
            }
            let cards = s.refresh_cards(&[f], now).unwrap();
            assert_eq!(
                cards.iter().any(|c| c.kind == CardKind::Shortfall),
                case == 0,
                "organizer capacity {capacity}, case {case}"
            );
            checked += 1;
        }
    }
    assert_eq!(checked, 50);
}
#[test]
fn weekly_constraints_filter_opportunities_and_legacy_goals_keep_their_meaning() {
    let clock = calendar::parse("2026-10-09 12:00").unwrap();
    for (begin, end, expected) in [
        ("2026-10-10 19:30", "2026-10-10 21:30", true),
        ("2026-10-11 19:00", "2026-10-11 22:00", true),
        ("2026-10-10 09:00", "2026-10-10 11:00", false),
        ("2026-10-12 19:00", "2026-10-12 21:00", false),
        ("2026-10-11 21:00", "2026-10-11 22:01", false),
    ] {
        let mut a = activity(2, "reading");
        a.start = calendar::parse(begin).unwrap();
        a.end = calendar::parse(end).unwrap();
        let dir = root();
        let mut s = IntentStore::open(&dir, "@member:test").unwrap();
        let mut input = goal(&a, GoalKind::Participate, clock);
        input.earliest = calendar::parse("2026-10-10 00:00").unwrap();
        input.latest = calendar::parse("2026-10-13 00:00").unwrap();
        let old = s
            .save_goal(None, input.clone(), &[a.clone()], clock)
            .unwrap();
        assert!(
            s.refresh_cards(&[fact(a.clone(), clock)], clock)
                .unwrap()
                .iter()
                .any(|c| c.kind == CardKind::Opportunity)
        );
        input.availability = Some(AvailabilitySnapshot {
            weekdays: vec![5, 6],
            earliest_minute: 19 * 60,
            latest_minute: 22 * 60,
            confirmed_at: clock,
        });
        s.save_goal(Some(&old.id), input, &[a.clone()], clock)
            .unwrap();
        assert_eq!(
            s.refresh_cards(&[fact(a, clock)], clock)
                .unwrap()
                .iter()
                .any(|c| c.kind == CardKind::Opportunity),
            expected,
            "{begin}"
        );
        drop(s);
        assert!(dir.starts_with(std::env::temp_dir()));
        std::fs::remove_dir_all(dir).unwrap();
    }
}
#[test]
fn reading_shortfall_explains_only_verified_registration_constraints() {
    let mut a = activity(2, "reading");
    let clock = a.start - 3600;
    a.join_own(
        "@member:test".into(),
        "报名成员".into(),
        Preferences {
            earliest: a.start,
            latest: a.end - 3600,
            group: 1,
        },
    )
    .unwrap();
    let dir = root();
    let mut store = IntentStore::open(&dir, "@owner:test").unwrap();
    store
        .save_goal(
            None,
            goal(&a, GoalKind::Organize, clock),
            &[a.clone()],
            clock,
        )
        .unwrap();
    let cards = store.refresh_cards(&[fact(a, clock)], clock).unwrap();
    let card = cards
        .iter()
        .find(|c| c.kind == CardKind::Shortfall)
        .unwrap();
    assert!(card.reason.contains("目标 2 人"));
    assert!(card.reason.contains("不覆盖活动"));
    assert!(!card.reason.contains("@member"));
    drop(store);
    assert!(dir.starts_with(std::env::temp_dir()));
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn independent_member_fifty_cases() {
    let mut checked = 0;
    for group in [1, 2, 4, 7, 8] {
        for case in 0..10 {
            let a = activity(8, ["badminton", "boardgame", "reading"][case % 3]);
            let now = a.start - 3600;
            let p = root();
            let mut s = IntentStore::open(&p, "@member:test").unwrap();
            let mut input = goal(&a, GoalKind::Participate, now);
            input.group = group;
            match case {
                1 => input.template = "custom".into(),
                2 => input.earliest += 60,
                3 => input.latest -= 60,
                _ => {}
            }
            let g = s.save_goal(None, input, &[a.clone()], now).unwrap();
            let mut f = fact(a.clone(), now);
            match case {
                4 => s.set_goal_status(&g.id, GoalStatus::Paused, now).unwrap(),
                5 => f.complete = false,
                6 => f.observed_at = now - 121,
                7 => f.activity.capacity = group - 1,
                8 => f.accessible = false,
                9 => {
                    f.activity
                        .join_own(
                            "@member:test".into(),
                            "本人".into(),
                            Preferences {
                                earliest: a.start,
                                latest: a.end,
                                group,
                            },
                        )
                        .unwrap();
                }
                _ => {}
            }
            let cards = s.refresh_cards(&[f], now).unwrap();
            assert_eq!(
                cards.iter().any(|c| c.kind == CardKind::Opportunity),
                case == 0,
                "member group {group}, case {case}"
            );
            checked += 1;
        }
    }
    assert_eq!(checked, 50);
}
#[test]
fn cards_deduplicate_snooze_quiet_hours_and_daily_cap_persist() {
    let p = root();
    let mut s = IntentStore::open(&p, "@member:test").unwrap();
    let a = activity(8, "badminton");
    let now = calendar::parse("2026-10-10 09:00").unwrap();
    s.save_goal(
        None,
        goal(&a, GoalKind::Participate, now),
        &[a.clone()],
        now,
    )
    .unwrap();
    let facts = (0..5)
        .map(|_| {
            let mut a = a.clone();
            a.metadata.as_mut().unwrap().activity_id = action_receipts::new_id();
            fact(a, now)
        })
        .collect::<Vec<_>>();
    assert_eq!(s.refresh_cards(&facts, now).unwrap().len(), 5);
    assert_eq!(s.claim_notifications(now).unwrap().len(), 3);
    assert!(s.claim_notifications(now).unwrap().is_empty());
    drop(s);
    let mut s = IntentStore::open(&p, "@member:test").unwrap();
    assert!(s.claim_notifications(now).unwrap().is_empty());
    let c = s.visible_cards(now).unwrap()[0].clone();
    s.control_card(&c.id, Some(now + 3600), false, now).unwrap();
    assert_eq!(s.visible_cards(now).unwrap().len(), 4);
    assert!(s.visible_cards(now + 601).unwrap().is_empty());
    assert!(s.start_task(&c.id, &c.fingerprint, now + 601).is_err());
    let mut prefs = s.load().unwrap().preferences;
    prefs.quiet_start = 8;
    prefs.quiet_end = 12;
    s.save_preferences(prefs, now).unwrap();
    assert!(s.claim_notifications(now).unwrap().is_empty());
}
#[test]
fn changed_goal_pauses_task_and_other_accounts_cannot_resume() {
    let p = root();
    let mut s = IntentStore::open(&p, "@member:test").unwrap();
    let a = activity(8, "badminton");
    let now = a.start - 3600;
    let input = goal(&a, GoalKind::Participate, now);
    let g = s.save_goal(None, input.clone(), &[a.clone()], now).unwrap();
    let facts = [fact(a.clone(), now)];
    let cards = s.refresh_cards(&facts, now).unwrap();
    let c = &cards[0];
    let task = s.start_task(&c.id, &c.fingerprint, now).unwrap();
    assert_eq!(
        s.start_task(&c.id, &c.fingerprint, now).unwrap().id,
        task.id
    );
    s.save_goal(Some(&g.id), input, &[a], now + 1).unwrap();
    s.follow_tasks(&facts, &[], now + 1).unwrap();
    assert_eq!(
        s.load().unwrap().tasks[0].status,
        assistance_tasks::TaskStatus::Paused
    );
    assert!(
        IntentStore::open(&p, "@other:test")
            .unwrap()
            .load()
            .unwrap()
            .tasks
            .is_empty()
    );
}
