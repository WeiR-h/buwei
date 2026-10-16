use action_receipts::{Operation, Status};
use buwei_host_core::{
    assistance::{GoalInput, GoalKind},
    goal_progress::registration_progress,
    *,
};
fn fixture() -> (Activity, GoalInput, u64) {
    let start = calendar::parse("2026-10-13 19:30").unwrap();
    let mut a = Activity::new(
        "@owner:test".into(),
        "!reading:test".into(),
        "读书会".into(),
        1,
        19,
        21,
    )
    .unwrap();
    a.start = start;
    a.end = start + 7200;
    a.metadata = Some(calendar::Metadata {
        activity_id: "a".repeat(32),
        template: "reading".into(),
        location: "测试地点".into(),
        description: String::new(),
        campus: None,
        archived: false,
        metrics: Default::default(),
    });
    let goal = GoalInput {
        title: "参加读书会".into(),
        kind: GoalKind::Participate,
        activity_id: None,
        template: "reading".into(),
        earliest: a.start,
        latest: a.end,
        group: 1,
        target: None,
        check_at: None,
        recurrence_days: None,
        preparation_hours: 24,
        availability: None,
    };
    (a, goal, start - 3600)
}
fn operation(a: &Activity, goal: &GoalInput, clock: u64, status: Status) -> Operation {
    let action = participation::Intent::Join {
        preferences: Preferences {
            earliest: goal.earliest,
            latest: goal.latest,
            group: goal.group,
        },
    }
    .action(a);
    serde_json::from_value(serde_json::json!({"id":"original-operation","app":"buwei","account":"@member:test","action":action,"revision":a.revision,"digest":"test-only","created_at":clock,"expires_at":clock+120,"status":status,"receipt":null,"authority_id":"fixture","generation":0,"instance":"fixture"})).unwrap()
}
fn enroll(a: &mut Activity, end: u64) {
    a.join_own(
        "@member:test".into(),
        "测试成员".into(),
        Preferences {
            earliest: a.start,
            latest: end,
            group: 1,
        },
    )
    .unwrap();
}
#[test]
fn saving_a_goal_does_not_claim_the_registration_was_updated() {
    let (mut a, g, n) = fixture();
    let end = a.end - 3600;
    enroll(&mut a, end);
    let before = serde_json::to_value(&a).unwrap();
    let text = registration_progress(&g, &a, "@member:test", &[], n);
    assert!(text.contains("候补仍为") && text.contains("20:30") && text.contains("另行预览"));
    assert_eq!(serde_json::to_value(&a).unwrap(), before);
}
#[test]
fn preparation_expiry_and_delivery_are_distinct_from_final_verification() {
    let (a, g, n) = fixture();
    let op = operation(&a, &g, n, Status::Prepared);
    assert!(registration_progress(&g, &a, "@member:test", &[op.clone()], n).contains("尚未发送"));
    assert!(
        registration_progress(&g, &a, "@member:test", &[op.clone()], n + 120).contains("已过期")
    );
    let mut sent = op;
    sent.status = Status::Confirmed;
    assert!(registration_progress(&g, &a, "@member:test", &[sent], n).contains("等待组织者核验"));
}
#[test]
fn unknown_result_keeps_original_operation_and_blocks_new_update_guidance() {
    let (a, g, n) = fixture();
    for state in [Status::Unknown, Status::Dispatching] {
        let text = registration_progress(&g, &a, "@member:test", &[operation(&a, &g, n, state)], n);
        assert!(text.contains("original-operation") && text.contains("先恢复回执"));
        assert!(!text.contains("另行预览"));
    }
}
#[test]
fn other_accounts_and_activities_do_not_affect_this_progress() {
    let (a, g, n) = fixture();
    let mut other_account = operation(&a, &g, n, Status::Unknown);
    other_account.account = "@other:test".into();
    let mut other_room = operation(&a, &g, n, Status::Unknown);
    other_room.action.target = "!other:test".into();
    let text = registration_progress(&g, &a, "@member:test", &[other_account, other_room], n);
    assert!(text.contains("尚无本人报名"));
    assert!(!text.contains("original-operation"));
}
#[test]
fn queue_verification_invitation_and_acceptance_use_separate_evidence() {
    let (mut a, g, n) = fixture();
    let end = a.end;
    enroll(&mut a, end);
    assert!(registration_progress(&g, &a, "@member:test", &[], n).contains("继续按原排位"));
    a.invitations.push(Invitation {
        operation_id: "invite".into(),
        recipient: "@member:test".into(),
        room: a.room.clone(),
        digest: "test".into(),
        until: n + 300,
        delivery: Delivery::Unknown,
        server_event: None,
        reply: Reply::Pending,
    });
    assert!(registration_progress(&g, &a, "@member:test", &[], n).contains("送达仍在核验"));
    a.invitations[0].delivery = Delivery::Delivered;
    assert!(registration_progress(&g, &a, "@member:test", &[], n).contains("确认接受或拒绝"));
    assert!(registration_progress(&g, &a, "@member:test", &[], n + 301).contains("请勿使用旧邀请"));
    a.people[0].status = PersonStatus::Confirmed;
    a.invitations[0].reply = Reply::Accepted;
    assert!(registration_progress(&g, &a, "@member:test", &[], n).contains("报名已确认"));
    assert!(registration_progress(&g, &a, "@member:test", &[], n).contains("不表示实际到场"));
}
