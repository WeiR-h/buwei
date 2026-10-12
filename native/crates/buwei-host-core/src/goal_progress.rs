//! Explain the boundary between an account-local goal and a verified registration.
use crate::{
    Activity, Delivery, PersonStatus, Preferences, Reply,
    assistance::{GoalInput, GoalKind},
};
use action_receipts::{Operation, Status};

pub fn registration_progress(
    goal: &GoalInput,
    activity: &Activity,
    account: &str,
    operations: &[Operation],
    clock: u64,
) -> String {
    if goal.kind != GoalKind::Participate
        || goal.activity_id.as_ref().is_some_and(|id| {
            activity
                .metadata
                .as_ref()
                .is_none_or(|m| &m.activity_id != id)
        })
    {
        return "请先选择本场活动对应的参与目标".into();
    }
    let own = |op: &&Operation| op.account == account && op.action.target == activity.room;
    if let Some(op) = operations
        .iter()
        .filter(own)
        .find(|op| matches!(op.status, Status::Dispatching | Status::Unknown))
    {
        return format!(
            "原操作 {} 的结果待核实；请先恢复回执，不会重复发送。",
            op.id
        );
    }
    let expected = Preferences {
        earliest: goal.earliest,
        latest: goal.latest,
        group: goal.group,
    };
    let join = operations.iter().filter(own).find(|op| {
        op.action.permission == "participate"
            && serde_json::from_value::<crate::participation::Intent>(op.action.payload.clone())
                .is_ok_and(|intent| {
                    intent
                        == crate::participation::Intent::Join {
                            preferences: expected.clone(),
                        }
                })
            && !matches!(
                op.status,
                Status::Cancelled | Status::Expired | Status::Failed
            )
    });
    if let Some(op) = join {
        if matches!(op.status, Status::Prepared | Status::Queued) {
            return if clock >= op.expires_at {
                "报名更新预览已过期，尚未发送；请重新预览并确认。".into()
            } else {
                "目标已保存，报名更新尚未发送；请核对本场活动并确认页面内的操作预览。".into()
            };
        }
        if op.status == Status::Confirmed && activity.revision <= op.revision {
            return "报名更新已送达服务器，等待组织者核验；最终候补与名额结果将在同步后显示。"
                .into();
        }
    }
    let Some(person) = activity.people.iter().find(|p| p.account == account) else {
        return "目标已保存；本场尚无本人报名，请在活动详情预览本人报名。".into();
    };
    match person.status {
        PersonStatus::Confirmed => "组织者已核验：本人报名已确认；此结果不表示实际到场。".into(),
        PersonStatus::Cancelled | PersonStatus::Declined | PersonStatus::Expired => {
            "本场本人报名已取消、拒绝或过期；目标仍保留，重新报名需在活动详情另行确认。".into()
        }
        PersonStatus::Waiting => {
            if let Some(invite) = activity
                .invitations
                .iter()
                .find(|i| i.recipient == account && i.reply == Reply::Pending)
            {
                if clock >= invite.until {
                    return "邀请已到回复期限，等待组织者核验与释放结果；请勿使用旧邀请接受。"
                        .into();
                }
                return if invite.delivery == Delivery::Delivered {
                    "组织者已核验候补，邀请已送达；请本人在活动详情确认接受或拒绝。".into()
                } else {
                    "本场有名额保留，邀请送达仍在核验；先核实原邀请，再决定下一步。".into()
                };
            }
            if person.preferences == expected {
                "组织者已核验：本场候补与当前目标一致，继续按原排位等待；无需重复更新。".into()
            } else {
                format!(
                    "目标已保存；组织者当前核验的候补仍为 {}–{}、{} 人。请另行预览本场候补更新，确认后才会发送。",
                    crate::calendar::display(person.preferences.earliest),
                    crate::calendar::display(person.preferences.latest),
                    person.preferences.group
                )
            }
        }
    }
}
