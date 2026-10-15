use buwei_host_core::{
    Activity, Preferences, activity_types as types,
    assistance::*,
    calendar,
    discovery::*,
    goal_correction::{FieldChange, corrected_input},
    planning::*,
    proactive::*,
};
use serde_json::json;

fn clock() -> u64 {
    calendar::parse("2026-10-16 12:00").unwrap()
}
fn activity(kind: &str) -> Activity {
    let t = types::get(kind).unwrap();
    let mut a = Activity::new(
        "@owner:test".into(),
        "!activity:test".into(),
        t.title.into(),
        2,
        19,
        21,
    )
    .unwrap();
    a.start = calendar::parse("2026-10-17 19:30").unwrap();
    a.end = calendar::parse("2026-10-17 21:30").unwrap();
    a.metadata = Some(calendar::Metadata {
        activity_id: action_receipts::new_id(),
        template: kind.into(),
        location: "测试场地".into(),
        description: t.description.into(),
        archived: false,
        metrics: Default::default(),
    });
    a
}
fn goal(a: &Activity) -> GoalInput {
    GoalInput {
        title: "本人参加目标".into(),
        kind: GoalKind::Participate,
        activity_id: None,
        template: a.metadata.as_ref().unwrap().template.clone(),
        earliest: a.start,
        latest: a.end,
        group: 2,
        target: None,
        check_at: None,
        recurrence_days: None,
        preparation_hours: 24,
        availability: None,
    }
}
fn facts(a: Activity) -> VerifiedFacts {
    VerifiedFacts {
        activity: a,
        observed_at: clock(),
        complete: true,
        accessible: true,
        automation_pause: None,
        has_pending_operation: false,
    }
}
struct Temp(std::path::PathBuf);
impl Temp {
    fn new() -> Self {
        Self(std::env::temp_dir().join(format!("buwei-types-{}", action_receipts::new_id())))
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn aliases_are_explicit_unambiguous_and_old_identifiers_remain_stable() {
    let mut aliases = std::collections::BTreeMap::new();
    for t in types::ALL {
        assert!((1..=30).contains(&t.capacity));
        assert!(types::valid(t.id));
        assert_eq!(types::parse(t.name).unwrap(), t.id);
        for alias in t.aliases {
            if let Some(previous) = aliases.insert(alias.to_ascii_lowercase(), t.id) {
                assert_eq!(previous, t.id);
            }
            assert_eq!(
                types::parse(&format!("  {}  ", alias.to_ascii_uppercase())).unwrap(),
                t.id
            );
        }
    }
    for (label, id) in [
        ("羽毛球", "badminton"),
        ("桌游", "boardgame"),
        ("读书会", "reading"),
        ("自定义", "custom"),
        ("不限", "any"),
    ] {
        assert_eq!(types::parse(label).unwrap(), id);
    }
    for unknown in [
        "",
        "打球",
        "delete",
        "run; send",
        "未来协议活动",
        "readingness",
    ] {
        assert!(types::parse(unknown).is_err());
    }
    assert!(!types::valid("any"));
}

#[test]
fn type_search_finds_aliases_and_groups_without_changing_selection() {
    for query in [
        "夜跑", "户外", "学习", "篮球", "YOGA", "公益", "聚餐", "亲子",
    ] {
        assert!(!types::search(query).is_empty(), "{query}");
    }
    assert!(
        types::search("户外")
            .iter()
            .all(|t| t.category == "户外出行")
    );
    assert!(types::search("没有这种类型").is_empty());
    assert_eq!(types::search("").len(), types::ALL.len());
}

#[test]
fn templates_stay_local_unconfirmed_and_require_date_location_and_review() {
    for t in types::ALL {
        let mut draft = ActivityPlanDraft::from_template("@owner:test", t.id, clock()).unwrap();
        assert_eq!(draft.status, PlanStatus::Draft);
        assert!(
            draft.activity_id.is_none()
                && draft.setup_id.is_none()
                && draft.publication_operations.is_empty()
        );
        assert!(!draft.agenda.is_empty() && !draft.preparation.is_empty());
        assert_eq!(draft.basics.questions().len(), 2); // start/end and location, not an invented date or venue
        assert!(draft.creation_digest(clock()).is_err());
        draft.basics.start = "2026-10-17 19:30".into();
        draft.basics.end = "2026-10-17 21:30".into();
        draft.basics.location = "本人核对的场地".into();
        assert!(draft.creation_digest(clock()).is_err()); // filling fields alone is not confirmation
        draft.confirm_basics(draft.basics.clone(), clock()).unwrap();
        assert!(draft.creation_digest(clock()).is_ok());
    }
    assert!(ActivityPlanDraft::from_template("@owner:test", "any", clock()).is_err());
    assert!(ActivityPlanDraft::from_template("@owner:test", "unknown", clock()).is_err());
    assert!(ActivityPlanDraft::from_template("not-an-account", "running", clock()).is_err());
}

#[test]
fn template_drafts_survive_restart_remain_account_private_and_keep_original_drafts() {
    let temp = Temp::new();
    std::fs::create_dir_all(&temp.0).unwrap();
    let path = temp.0.join("plans.db");
    let store = PlanStore::open(&path, "@owner:test").unwrap();
    let mut first = ActivityPlanDraft::from_template("@owner:test", "hiking", clock()).unwrap();
    store.save(&mut first, None).unwrap();
    let mut second = ActivityPlanDraft::from_template("@owner:test", "dining", clock()).unwrap();
    store.save(&mut second, None).unwrap();
    drop(store);
    let store = PlanStore::open(&path, "@owner:test").unwrap();
    assert_eq!(store.load(&first.id).unwrap().basics.template, "hiking");
    assert_eq!(
        store.load(&second.id).unwrap().preparation,
        second.preparation
    );
    assert!(
        PlanStore::open(&path, "@other:test")
            .unwrap()
            .load(&first.id)
            .is_err()
    );
}

#[test]
fn every_type_publishes_and_filters_in_both_public_and_community_scopes() {
    for t in types::ALL {
        let a = activity(t.id);
        a.validate().unwrap();
        for audience in [Audience::Community, Audience::Public] {
            let mut card = Announcement::from_activity(&a, false).unwrap();
            card.audience = audience;
            if audience == Audience::Public {
                card.protocol = 2;
            }
            let mut event = json!({"event_id":"$card","type":"m.room.message","sender":a.owner,
                "origin_server_ts":clock()*1000,"content":{"org.buwei.community":card}});
            let listing = Listing::parse(&event, "!community:test", clock()).unwrap();
            let filter = Filter {
                template: t.id.into(),
                date: "2026-10-17".into(),
                available_only: true,
            };
            filter.validate().unwrap();
            assert!(filter.matches(&listing, clock()));
            let other = if t.id == "reading" {
                "running"
            } else {
                "reading"
            };
            assert!(
                !Filter {
                    template: other.into(),
                    ..filter.clone()
                }
                .matches(&listing, clock())
            );
            assert!(
                Filter {
                    template: "any".into(),
                    ..filter.clone()
                }
                .matches(&listing, clock())
            );
            assert!(!filter.matches(&listing, a.start));
            event["sender"] = "@forged:test".into();
            assert!(Listing::parse(&event, "!community:test", clock()).is_err());
        }
    }
}

#[test]
fn unknown_and_wildcard_activity_codes_fail_closed_in_plans_metadata_and_cards() {
    for bad in ["any", "not-supported", "RUNNING", "跑步"] {
        let mut a = activity("running");
        a.metadata.as_mut().unwrap().template = bad.into();
        assert!(a.validate().is_err());
        let mut basics = ActivityPlanDraft::from_template("@owner:test", "running", clock())
            .unwrap()
            .basics;
        basics.template = bad.into();
        assert!(basics.validate_draft().is_err());
        let mut card = Announcement::from_activity(&activity("running"), false).unwrap();
        card.template = bad.into();
        assert!(card.validate().is_err());
    }
}

#[test]
fn recommendation_matches_each_type_without_cross_type_leakage_or_actions() {
    for t in types::ALL {
        let temp = Temp::new();
        let a = activity(t.id);
        let mut store = IntentStore::open(&temp.0, "@member:test").unwrap();
        store
            .save_goal(None, goal(&a), &[a.clone()], clock())
            .unwrap();
        let before = serde_json::to_value(&a).unwrap();
        let cards = evaluate(
            "@member:test",
            &store.load().unwrap(),
            &[facts(a.clone())],
            clock(),
        );
        assert_eq!(
            cards
                .iter()
                .filter(|c| c.kind == CardKind::Opportunity)
                .count(),
            1,
            "{}",
            t.id
        );
        let mut other = a.clone();
        other.metadata.as_mut().unwrap().template = if t.id == "running" {
            "reading"
        } else {
            "running"
        }
        .into();
        assert!(
            !evaluate(
                "@member:test",
                &store.load().unwrap(),
                &[facts(other)],
                clock()
            )
            .iter()
            .any(|c| c.kind == CardKind::Opportunity)
        );
        assert!(
            evaluate(
                "@other:test",
                &store.load().unwrap(),
                &[facts(a.clone())],
                clock()
            )
            .is_empty()
        );
        assert_eq!(serde_json::to_value(&a).unwrap(), before);
    }
}

#[test]
fn recommendations_honor_full_duration_and_whole_party_for_every_type() {
    for t in types::ALL {
        let temp = Temp::new();
        let a = activity(t.id);
        let mut store = IntentStore::open(&temp.0, "@member:test").unwrap();
        store
            .save_goal(None, goal(&a), &[a.clone()], clock())
            .unwrap();
        let state = store.load().unwrap();
        let mut no_capacity = a.clone();
        no_capacity.capacity = 1;
        let mut too_long = a.clone();
        too_long.end += 60;
        for unavailable in [
            facts(no_capacity),
            facts(too_long),
            VerifiedFacts {
                complete: false,
                ..facts(a.clone())
            },
        ] {
            assert!(
                !evaluate("@member:test", &state, &[unavailable], clock())
                    .iter()
                    .any(|c| c.kind == CardKind::Opportunity)
            );
        }
        let mut queue = a.clone();
        queue.capacity = 1;
        queue
            .join_own(
                "@pair:test".into(),
                "同行组".into(),
                Preferences {
                    earliest: a.start,
                    latest: a.end,
                    group: 2,
                },
            )
            .unwrap();
        queue
            .join_own(
                "@single:test".into(),
                "单人".into(),
                Preferences {
                    earliest: a.start,
                    latest: a.end,
                    group: 1,
                },
            )
            .unwrap();
        assert_eq!(queue.candidate(), Some("@single:test"));
        assert_eq!(queue.people[0].preferences.group, 2); // never split or remove the original group
        assert_eq!(queue.people[0].joined, 1);
    }
}

#[test]
fn correcting_type_requires_evidence_for_that_type_and_preserves_other_fields() {
    let before = goal(&activity("reading"));
    for t in types::ALL {
        let text = format!("这次改为{}", t.name);
        let change = FieldChange {
            field: "template".into(),
            value: t.id.into(),
            evidence: t.name.into(),
        };
        let after = corrected_input(&before, &text, &[change]).unwrap();
        assert_eq!(after.template, t.id);
        assert_eq!(after.earliest, before.earliest);
        assert_eq!(after.latest, before.latest);
        assert_eq!(after.group, before.group);
        assert!(
            PersonalPreferences {
                template: Some(t.id.into()),
                ..Default::default()
            }
            .validate()
            .is_ok()
        );
    }
    for (value, evidence) in [
        ("reading", "篮球"),
        ("running", "prerunning"),
        ("unknown", "unknown"),
    ] {
        let change = FieldChange {
            field: "template".into(),
            value: value.into(),
            evidence: evidence.into(),
        };
        assert!(corrected_input(&before, evidence, &[change]).is_err());
    }
}

#[test]
fn plan_type_changes_cannot_relabel_a_different_request() {
    let mut draft = ActivityPlanDraft::from_template("@owner:test", "reading", clock()).unwrap();
    let before = serde_json::to_value(&draft).unwrap();
    let proposal = PlanProposal {
        updates: vec![FieldUpdate {
            field: "template".into(),
            value: "dining".into(),
            source: "篮球".into(),
        }],
        ..Default::default()
    };
    assert!(draft.merge("改为篮球", proposal, clock()).is_err());
    assert_eq!(serde_json::to_value(&draft).unwrap(), before);
}

#[test]
fn creation_page_waits_for_its_own_verified_activity_through_intermediate_updates() {
    use buwei_host_core::interaction::creation_is_complete;
    let mut draft = ActivityPlanDraft::from_template("@owner:test", "dining", clock()).unwrap();
    let plan_id = draft.id.clone();
    let activity_id = action_receipts::new_id();
    assert!(!creation_is_complete(
        Some(&plan_id),
        Some(&draft),
        &activity_id
    ));
    draft.status = PlanStatus::Creating;
    draft.setup_id = Some(activity_id.clone());
    assert!(!creation_is_complete(
        Some(&plan_id),
        Some(&draft),
        &activity_id
    ));
    draft.status = PlanStatus::Created;
    draft.activity_id = Some(activity_id.clone());
    assert!(creation_is_complete(
        Some(&plan_id),
        Some(&draft),
        &activity_id
    ));
    assert!(!creation_is_complete(
        Some("old-plan"),
        Some(&draft),
        &activity_id
    ));
    assert!(!creation_is_complete(
        Some(&plan_id),
        Some(&draft),
        "other-activity"
    ));
    assert!(!creation_is_complete(None, Some(&draft), &activity_id));
    assert!(!creation_is_complete(Some(&plan_id), None, &activity_id));
    draft.status = PlanStatus::Published;
    assert!(creation_is_complete(
        Some(&plan_id),
        Some(&draft),
        &activity_id
    ));
}

#[test]
fn verified_type_is_available_in_answers_and_organizer_csv_without_private_preferences() {
    for t in types::ALL {
        let mut a = activity(t.id);
        a.join_own(
            "@member:test".into(),
            "=公式样本".into(),
            Preferences {
                earliest: a.start,
                latest: a.end,
                group: 2,
            },
        )
        .unwrap();
        let answer = buwei_host_core::facts::render(&a, &["activity_type".into()]).unwrap();
        assert!(answer.contains(t.name) && answer.contains("依据：activity_type"));
        assert!(buwei_host_core::roster::csv(&a, "@member:test", clock(), clock()).is_err());
        let csv = String::from_utf8(
            buwei_host_core::roster::csv(&a, &a.owner, clock(), clock()).unwrap(),
        )
        .unwrap();
        assert!(csv.starts_with('\u{feff}'));
        assert!(csv.lines().next().unwrap().ends_with("报名状态,活动类型"));
        assert!(csv.contains(&format!(",\"{}\"\r\n", t.name)));
        assert!(csv.contains("\"'=公式样本\""));
        assert!(!csv.contains("private_preferences"));
    }
}
