use action_receipts::{Action, Authority, Evidence, Journal, Receipt, Status};
use buwei_host_core::{assistance::*, assistance_tasks::*, proactive::*, *};
use serde_json::json;
fn setup(actor: &str) -> (IntentStore, Activity, u64) {
    let root = std::env::temp_dir().join(format!("buwei-task-{}", action_receipts::new_id()));
    let mut a = Activity::new(
        "@owner:test".into(),
        "!test:test".into(),
        "合成活动".into(),
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
        location: "合成场地".into(),
        description: String::new(),
        archived: false,
        metrics: Default::default(),
    });
    let now = a.start - 3600;
    let mut s = IntentStore::open(&root, actor).unwrap();
    s.save_goal(
        None,
        GoalInput {
            title: "本人的目标".into(),
            kind: if actor == a.owner {
                GoalKind::Organize
            } else {
                GoalKind::Participate
            },
            activity_id: if actor == a.owner {
                Some(a.metadata.as_ref().unwrap().activity_id.clone())
            } else {
                None
            },
            template: "badminton".into(),
            earliest: a.start,
            latest: a.end,
            group: 1,
            target: if actor == a.owner { Some(6) } else { None },
            check_at: if actor == a.owner { Some(now) } else { None },
            recurrence_days: None,
            preparation_hours: 24,
        },
        &[a.clone()],
        now,
    )
    .unwrap();
    (s, a, now)
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
fn preview(
    actor: &str,
    target: &str,
    permission: &str,
    payload: serde_json::Value,
    now: u64,
) -> action_receipts::Operation {
    let authority = Authority::default();
    authority.set_account(Some(actor));
    let g = authority.grant("buwei", &[permission], now, 3600).unwrap();
    let path = std::env::temp_dir().join(format!(
        "buwei-task-journal-{}.db",
        action_receipts::new_id()
    ));
    Journal::open(path)
        .unwrap()
        .prepare(
            &g,
            Action {
                permission: permission.into(),
                target: target.into(),
                summary: "合成操作".into(),
                payload,
            },
            0,
            now,
            120,
        )
        .unwrap()
}
#[test]
fn original_operation_recovers_without_a_duplicate_task_or_false_completion() {
    let (mut s, mut a, now) = setup("@member:test");
    let facts = [fact(a.clone(), now)];
    let card = s.refresh_cards(&facts, now).unwrap().remove(0);
    let task = s.start_task(&card.id, &card.fingerprint, now).unwrap();
    let mut op = preview(
        "@member:test",
        &a.room,
        "participate",
        json!({"kind":"join","preferences":{"earliest":a.start,"latest":a.end,"group":1}}),
        now,
    );
    // Journal persisted before the task update: follow_tasks recovers linkage.
    op.status = Status::Unknown;
    s.follow_tasks(&facts, &[op.clone()], now + 1).unwrap();
    let saved = s.load().unwrap().tasks.remove(0);
    assert_eq!(saved.status, TaskStatus::Reconciling);
    assert_eq!(saved.steps[1].operation_id.as_deref(), Some(op.id.as_str()));
    assert_eq!(
        s.start_task(&card.id, &card.fingerprint, now + 1)
            .unwrap()
            .id,
        task.id
    );
    op.status = Status::Confirmed;
    s.follow_tasks(&facts, &[op.clone()], now + 2).unwrap();
    assert_eq!(s.load().unwrap().tasks[0].status, TaskStatus::Reconciling);
    op.receipt = Some(Receipt {
        status: Status::Confirmed,
        evidence: Some(Evidence {
            operation_id: op.id.clone(),
            external_id: "$synthetic-event".into(),
            account: op.account.clone(),
            target: op.action.target.clone(),
            digest: op.digest.clone(),
        }),
        message: "合成回执".into(),
    });
    s.follow_tasks(&facts, &[op.clone()], now + 3).unwrap();
    assert_eq!(s.load().unwrap().tasks[0].status, TaskStatus::WaitingReply);
    a.join_own(
        "@member:test".into(),
        "合成成员".into(),
        Preferences {
            earliest: a.start,
            latest: a.end,
            group: 1,
        },
    )
    .unwrap();
    a.people[0].status = PersonStatus::Confirmed;
    s.follow_tasks(&[fact(a, now + 4)], &[op], now + 4).unwrap();
    assert_eq!(s.load().unwrap().tasks[0].status, TaskStatus::Completed);
}
#[test]
fn different_accounts_articles_and_old_operations_do_not_advance_a_task() {
    let (mut s, a, now) = setup("@member:test");
    let card = s
        .refresh_cards(&[fact(a.clone(), now)], now)
        .unwrap()
        .remove(0);
    let t = s.start_task(&card.id, &card.fingerprint, now).unwrap();
    for op in [
        preview(
            "@other:test",
            &a.room,
            "participate",
            json!({"kind":"join"}),
            now,
        ),
        preview("@member:test", &a.room, "publish", json!({}), now),
        preview(
            "@member:test",
            &a.room,
            "participate",
            json!({"kind":"join"}),
            now - 1,
        ),
    ] {
        assert!(s.link_operation(&t.id, &op, now).is_err());
    }
    let mut op = preview(
        "@member:test",
        &a.room,
        "participate",
        json!({"kind":"join"}),
        now,
    );
    op.status = Status::Failed;
    s.follow_tasks(&[fact(a, now)], &[op], now).unwrap();
    assert_eq!(s.load().unwrap().tasks[0].status, TaskStatus::Paused);
}
#[test]
fn a_previous_decline_cannot_complete_a_new_registration_task() {
    let (mut s, mut a, now) = setup("@member:test");
    a.join_own(
        "@member:test".into(),
        "合成成员".into(),
        Preferences {
            earliest: a.start,
            latest: a.end,
            group: 1,
        },
    )
    .unwrap();
    a.people[0].status = PersonStatus::Declined;
    let card = s
        .refresh_cards(&[fact(a.clone(), now)], now)
        .unwrap()
        .into_iter()
        .find(|c| c.action == SuggestedAction::Register)
        .unwrap();
    s.start_task(&card.id, &card.fingerprint, now).unwrap();
    let mut op = preview(
        "@member:test",
        &a.room,
        "participate",
        json!({"kind":"join"}),
        now,
    );
    op.status = Status::Confirmed;
    op.receipt = Some(Receipt {
        status: Status::Confirmed,
        evidence: Some(Evidence {
            operation_id: op.id.clone(),
            external_id: "$new-registration".into(),
            account: op.account.clone(),
            target: op.action.target.clone(),
            digest: op.digest.clone(),
        }),
        message: "合成回执".into(),
    });
    s.follow_tasks(&[fact(a.clone(), now + 1)], &[op.clone()], now + 1)
        .unwrap();
    assert_eq!(s.load().unwrap().tasks[0].status, TaskStatus::WaitingReply);
    a.join_own(
        "@member:test".into(),
        "合成成员".into(),
        Preferences {
            earliest: a.start,
            latest: a.end,
            group: 1,
        },
    )
    .unwrap();
    a.people[0].status = PersonStatus::Confirmed;
    s.follow_tasks(&[fact(a, now + 2)], &[op], now + 2).unwrap();
    assert_eq!(s.load().unwrap().tasks[0].status, TaskStatus::Completed);
}
#[test]
fn expired_registration_completes_only_after_a_verified_current_result() {
    let (mut s, mut a, now) = setup("@member:test");
    let card = s
        .refresh_cards(&[fact(a.clone(), now)], now)
        .unwrap()
        .into_iter()
        .find(|c| c.action == SuggestedAction::Register)
        .unwrap();
    let task = s.start_task(&card.id, &card.fingerprint, now).unwrap();
    let mut op = preview(
        "@member:test",
        &a.room,
        "participate",
        json!({"kind":"join"}),
        now,
    );
    op.status = Status::Confirmed;
    op.receipt = Some(Receipt {
        status: Status::Confirmed,
        evidence: Some(Evidence {
            operation_id: op.id.clone(),
            external_id: "$current-registration".into(),
            account: op.account.clone(),
            target: op.action.target.clone(),
            digest: op.digest.clone(),
        }),
        message: "合成回执".into(),
    });
    a.join_own(
        "@member:test".into(),
        "合成成员".into(),
        Preferences {
            earliest: a.start,
            latest: a.end,
            group: 1,
        },
    )
    .unwrap();
    s.follow_tasks(&[fact(a.clone(), now + 1)], &[op.clone()], now + 1)
        .unwrap();
    assert_eq!(s.load().unwrap().tasks[0].status, TaskStatus::WaitingReply);

    a.people[0].status = PersonStatus::Expired;
    let mut incomplete = fact(a.clone(), now + 2);
    incomplete.complete = false;
    s.follow_tasks(&[incomplete], &[op.clone()], now + 2)
        .unwrap();
    assert_eq!(s.load().unwrap().tasks[0].status, TaskStatus::WaitingReply);
    s.follow_tasks(&[fact(a, now + 3)], &[op.clone()], now + 3)
        .unwrap();
    let finished = s.load().unwrap().tasks.remove(0);
    assert_eq!(finished.id, task.id);
    assert_eq!(finished.status, TaskStatus::Completed);
    assert_eq!(
        finished
            .steps
            .iter()
            .filter(|step| { step.operation_id.as_deref() == Some(op.id.as_str()) })
            .count(),
        1
    );
    assert!(finished.steps.iter().any(|step| {
        step.label == "业务结果已核验" && step.status == TaskStatus::Completed
    }));
}

#[test]
fn unsent_confirmation_checks_persisted_and_not_yet_linked_task_goals() {
    for linked in [false, true] {
        for change in [
            "edit",
            "pause_goal",
            "complete_goal",
            "delete_goal",
            "pause_task",
            "feedback",
            "undo_feedback",
        ] {
            let (mut s, a, now) = setup("@member:test");
            let card = s
                .refresh_cards(&[fact(a.clone(), now)], now)
                .unwrap()
                .remove(0);
            let task = s.start_task(&card.id, &card.fingerprint, now).unwrap();
            let mut op = preview(
                "@member:test",
                &a.room,
                "participate",
                json!({"kind":"join"}),
                now,
            );
            let active = if linked { None } else { Some(task.id.as_str()) };
            if linked {
                s.link_operation(&task.id, &op, now).unwrap();
            }
            assert!(s.check_operation_confirmation(&op, active).is_ok());
            let goal_id = task.goal_id.as_deref().unwrap();
            match change {
                "edit" => {
                    let mut input = s.load().unwrap().goals[0].input.clone();
                    input.group = 2;
                    s.save_goal(Some(goal_id), input, &[a.clone()], now + 1)
                        .unwrap();
                }
                "pause_goal" => s
                    .set_goal_status(goal_id, GoalStatus::Paused, now + 1)
                    .unwrap(),
                "complete_goal" => s
                    .set_goal_status(goal_id, GoalStatus::Completed, now + 1)
                    .unwrap(),
                "delete_goal" => s.delete_goal(goal_id).unwrap(),
                "pause_task" => s.pause_task(&task.id, now + 1).unwrap(),
                "feedback" | "undo_feedback" => {
                    let feedback = s
                        .feedback(
                            goal_id,
                            "badminton".into(),
                            2,
                            buwei_host_core::intent_feedback::FeedbackScope::ThisOccasion,
                            &[a.clone()],
                            now + 1,
                        )
                        .unwrap();
                    if change == "undo_feedback" {
                        s.undo_feedback(&feedback.id, now + 2).unwrap();
                    }
                }
                _ => unreachable!(),
            }
            for status in [Status::Prepared, Status::Queued] {
                op.status = status;
                assert!(
                    s.check_operation_confirmation(&op, active).is_err(),
                    "{change}, linked={linked}, status={status:?}"
                );
            }
            // Stopping a task cannot discard an uncertain or verified old send.
            for status in [Status::Dispatching, Status::Unknown, Status::Confirmed] {
                op.status = status;
                assert!(s.check_operation_confirmation(&op, active).is_ok());
            }
        }
    }
}

#[test]
fn a_fresh_manual_confirmation_remains_available_and_account_scoped() {
    let (mut s, a, now) = setup("@member:test");
    let goal_id = s.load().unwrap().goals[0].id.clone();
    s.set_goal_status(&goal_id, GoalStatus::Paused, now + 1)
        .unwrap();
    let mut manual = preview(
        "@member:test",
        &a.room,
        "participate",
        json!({"kind":"join"}),
        now + 2,
    );
    assert!(s.check_operation_confirmation(&manual, None).is_ok());
    manual.account = "@other:test".into();
    assert!(s.check_operation_confirmation(&manual, None).is_err());
}

#[test]
fn persisted_preview_binding_blocks_confirmation_if_journal_cancellation_fails() {
    for pause_task in [false, true] {
        let (mut store, activity, clock) = setup("@member:test");
        let card = store
            .refresh_cards(&[fact(activity.clone(), clock)], clock)
            .unwrap()
            .remove(0);
        let task = store
            .start_task(&card.id, &card.fingerprint, clock)
            .unwrap();
        let mut operation = preview(
            "@member:test",
            &activity.room,
            "participate",
            json!({"kind":"join"}),
            clock,
        );
        let unrelated = preview(
            "@member:test",
            "!other:test",
            "participate",
            json!({"kind":"join"}),
            clock,
        );
        store
            .retain_task_previews(&task.id, &[operation.clone(), unrelated], clock)
            .unwrap();
        if pause_task {
            store.pause_task(&task.id, clock + 1).unwrap();
        } else {
            store
                .set_goal_status(
                    task.goal_id.as_deref().unwrap(),
                    GoalStatus::Paused,
                    clock + 1,
                )
                .unwrap();
        }
        // Model a failed journal cancellation: the original preview stays
        // Prepared even after the window's active task context is cleared.
        assert!(
            store
                .check_operation_confirmation(&operation, None)
                .is_err()
        );
        let retained = store.load().unwrap().tasks.remove(0);
        let references = retained
            .steps
            .iter()
            .filter_map(|step| step.operation_id.as_deref())
            .collect::<Vec<_>>();
        assert_eq!(references, [operation.id.as_str()]);
        assert_eq!(retained.id, task.id);
        for status in [Status::Dispatching, Status::Unknown, Status::Confirmed] {
            operation.status = status;
            assert!(store.check_operation_confirmation(&operation, None).is_ok());
        }
    }
}

#[test]
fn invitation_task_rejects_a_reply_to_a_different_invitation() {
    let (mut s, mut a, now) = setup("@member:test");
    a.join_own(
        "@member:test".into(),
        "合成成员".into(),
        Preferences {
            earliest: a.start,
            latest: a.end,
            group: 1,
        },
    )
    .unwrap();
    let invitation = "a".repeat(32);
    a.invitations.push(Invitation {
        operation_id: invitation.clone(),
        recipient: "@member:test".into(),
        room: a.room.clone(),
        digest: "b".repeat(64),
        until: now + 300,
        delivery: Delivery::Delivered,
        server_event: Some("$invitation".into()),
        reply: Reply::Pending,
    });
    let card = s
        .refresh_cards(&[fact(a.clone(), now)], now)
        .unwrap()
        .into_iter()
        .find(|c| c.kind == CardKind::Invitation)
        .unwrap();
    let task = s.start_task(&card.id, &card.fingerprint, now).unwrap();
    let wrong = preview(
        "@member:test",
        &a.room,
        "participate",
        json!({"kind":"reply","invitation_id":"c".repeat(32),"invitation_event":"$other","accept":true}),
        now,
    );
    assert!(s.link_operation(&task.id, &wrong, now).is_err());
    let right = preview(
        "@member:test",
        &a.room,
        "participate",
        json!({"kind":"reply","invitation_id":invitation,"invitation_event":"$invitation","accept":true}),
        now,
    );
    s.link_operation(&task.id, &right, now).unwrap();
}
#[test]
fn conflict_resolution_requires_the_same_registration_and_new_verified_state() {
    let (mut s, mut a, now) = setup("@member:test");
    a.join_own(
        "@member:test".into(),
        "合成成员".into(),
        Preferences {
            earliest: a.start,
            latest: a.end,
            group: 1,
        },
    )
    .unwrap();
    a.people[0].status = PersonStatus::Confirmed;
    let mut other = a.clone();
    other.room = "!other:test".into();
    other.metadata.as_mut().unwrap().activity_id = action_receipts::new_id();
    let card = s
        .refresh_cards(&[fact(a.clone(), now), fact(other, now)], now)
        .unwrap()
        .into_iter()
        .find(|c| c.action == SuggestedAction::ReviewConflict)
        .unwrap();
    s.start_task(&card.id, &card.fingerprint, now).unwrap();
    let mut op = preview(
        "@member:test",
        &a.room,
        "participate",
        json!({"kind":"cancel"}),
        now,
    );
    op.status = Status::Confirmed;
    op.receipt = Some(Receipt {
        status: Status::Confirmed,
        evidence: Some(Evidence {
            operation_id: op.id.clone(),
            external_id: "$cancel".into(),
            account: op.account.clone(),
            target: op.action.target.clone(),
            digest: op.digest.clone(),
        }),
        message: "合成回执".into(),
    });
    a.people[0].status = PersonStatus::Cancelled;
    a.revision = op.revision;
    s.follow_tasks(&[fact(a.clone(), now + 1)], &[op.clone()], now + 1)
        .unwrap();
    assert_eq!(s.load().unwrap().tasks[0].status, TaskStatus::WaitingReply);
    a.revision += 1;
    a.people[0].joined += 1;
    s.follow_tasks(&[fact(a.clone(), now + 2)], &[op.clone()], now + 2)
        .unwrap();
    assert_eq!(s.load().unwrap().tasks[0].status, TaskStatus::WaitingReply);
    a.people[0].joined -= 1;
    s.follow_tasks(&[fact(a, now + 3)], &[op], now + 3).unwrap();
    assert_eq!(s.load().unwrap().tasks[0].status, TaskStatus::Completed);
}
#[test]
fn a_shared_card_targets_the_contact_room_and_binds_the_activity_pointer() {
    let (mut s, a, now) = setup("@owner:test");
    let card = s
        .refresh_cards(&[fact(a.clone(), now)], now)
        .unwrap()
        .remove(0);
    let t = s.start_task(&card.id, &card.fingerprint, now).unwrap();
    let op = preview(
        "@owner:test",
        "!contact:test",
        "share_card",
        json!({"card":{"room":a.room,"activity_id":a.metadata.as_ref().unwrap().activity_id}}),
        now,
    );
    s.link_operation(&t.id, &op, now).unwrap();
    let mut other = op;
    other.action.payload["card"]["activity_id"] = json!("other");
    assert!(s.link_operation(&t.id, &other, now).is_err());
}
#[test]
fn automatic_invitation_can_complete_the_goal_without_claiming_a_share_was_sent() {
    let (mut s, mut a, now) = setup("@owner:test");
    let card = s
        .refresh_cards(&[fact(a.clone(), now)], now)
        .unwrap()
        .remove(0);
    s.start_task(&card.id, &card.fingerprint, now).unwrap();
    let mut op = preview("@owner:test", &a.room, "invite", json!({}), now);
    op.status = Status::Confirmed;
    op.receipt = Some(Receipt {
        status: Status::Confirmed,
        evidence: Some(Evidence {
            operation_id: op.id.clone(),
            external_id: "$synthetic-invitation".into(),
            account: op.account.clone(),
            target: op.action.target.clone(),
            digest: op.digest.clone(),
        }),
        message: "合成回执".into(),
    });
    a.join_own(
        "@member:test".into(),
        "合成成员".into(),
        Preferences {
            earliest: a.start,
            latest: a.end,
            group: 6,
        },
    )
    .unwrap();
    a.people[0].status = PersonStatus::Confirmed;
    s.follow_tasks(&[fact(a, now + 1)], &[op], now + 1).unwrap();
    let task = s.load().unwrap().tasks.remove(0);
    assert_eq!(task.status, TaskStatus::Completed);
    assert_eq!(task.steps[0].status, TaskStatus::Paused);
    assert!(
        task.steps[0]
            .evidence
            .as_ref()
            .unwrap()
            .contains("本步尚未执行")
    );
    assert!(task_text(&task).contains("本人选择接收者并确认分享 · 已暂停"));
}
#[test]
fn incomplete_sync_still_reports_its_pause_and_expired_previews_pause_tasks() {
    let (mut s, a, now) = setup("@owner:test");
    let mut f = fact(a, now);
    f.complete = false;
    f.automation_pause = Some("同步尚不完整".into());
    f.has_pending_operation = true;
    let cards = s.refresh_cards(&[f.clone()], now).unwrap();
    assert_eq!(cards.len(), 1);
    assert_eq!(cards[0].action, SuggestedAction::Reconcile);
    f.accessible = false;
    assert!(s.refresh_cards(&[f], now).unwrap().is_empty());
    let (mut s, a, now) = setup("@member:test");
    let card = s
        .refresh_cards(&[fact(a.clone(), now)], now)
        .unwrap()
        .remove(0);
    s.start_task(&card.id, &card.fingerprint, now).unwrap();
    let op = preview(
        "@member:test",
        &a.room,
        "participate",
        json!({"kind":"join"}),
        now,
    );
    s.follow_tasks(&[fact(a, now + 121)], &[op], now + 121)
        .unwrap();
    assert_eq!(s.load().unwrap().tasks[0].status, TaskStatus::Paused);
}
#[test]
fn explicit_snooze_allows_a_later_reminder_but_repeated_facts_do_not() {
    let (mut s, a, now) = setup("@member:test");
    let card = s
        .refresh_cards(&[fact(a.clone(), now)], now)
        .unwrap()
        .remove(0);
    assert_eq!(s.claim_notifications(now).unwrap().len(), 1);
    assert!(s.claim_notifications(now).unwrap().is_empty());
    s.control_card(&card.id, Some(now + 60), false, now)
        .unwrap();
    assert!(s.claim_notifications(now + 59).unwrap().is_empty());
    s.refresh_cards(&[fact(a.clone(), now + 60)], now + 60)
        .unwrap();
    assert_eq!(s.claim_notifications(now + 60).unwrap().len(), 1);
    assert!(s.claim_notifications(now + 60).unwrap().is_empty());
    s.control_card(&card.id, None, true, now + 60).unwrap();
    assert!(s.visible_cards(now + 60).unwrap().is_empty());
    s.reset_preferences().unwrap();
    // An ignored instance stays ignored; future changes to this kind may return.
    let mut changed = a;
    changed.revision += 1;
    s.refresh_cards(&[fact(changed, now + 61)], now + 61)
        .unwrap();
    assert_eq!(s.visible_cards(now + 61).unwrap().len(), 1);
}
#[test]
fn paused_tasks_still_show_a_verified_old_receipt_without_resuming_actions() {
    let (mut s, a, now) = setup("@member:test");
    let facts = [fact(a.clone(), now)];
    let card = s.refresh_cards(&facts, now).unwrap().remove(0);
    let t = s.start_task(&card.id, &card.fingerprint, now).unwrap();
    let mut op = preview(
        "@member:test",
        &a.room,
        "participate",
        json!({"kind":"join"}),
        now,
    );
    op.status = Status::Unknown;
    s.link_operation(&t.id, &op, now).unwrap();
    s.pause_task(&t.id, now + 1).unwrap();
    op.status = Status::Confirmed;
    op.receipt = Some(Receipt {
        status: Status::Confirmed,
        evidence: Some(Evidence {
            operation_id: op.id.clone(),
            external_id: "$synthetic".into(),
            account: op.account.clone(),
            target: op.action.target.clone(),
            digest: op.digest.clone(),
        }),
        message: "合成回执".into(),
    });
    s.follow_tasks(&facts, &[op], now + 2).unwrap();
    let task = s.load().unwrap().tasks.remove(0);
    assert_eq!(task.status, TaskStatus::Paused);
    assert_eq!(task.steps[1].status, TaskStatus::Completed);
}
