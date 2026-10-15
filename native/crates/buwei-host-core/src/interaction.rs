//! Account roles and registration defaults depend on this activity, not a launcher.
use crate::{Activity, Preferences, Result, assistance::PersonalPreferences};

pub fn is_organizer(activity: &Activity, actor: &str) -> bool {
    activity.owner == actor
}

/// UI completion follows the requested plan and its verified activity, not an unrelated update.
pub fn creation_is_complete(
    pending_plan: Option<&str>,
    plan: Option<&crate::planning::ActivityPlanDraft>,
    activity_id: &str,
) -> bool {
    let (Some(pending), Some(plan)) = (pending_plan, plan) else { return false; };
    !activity_id.is_empty()
        && pending == plan.id
        && matches!(plan.status, crate::planning::PlanStatus::Created | crate::planning::PlanStatus::Published)
        && plan.activity_id.as_deref() == Some(activity_id)
}

/// Presentation hints only. The host still validates each prepared action.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ParticipantControls {
    pub register: bool,
    pub reply: bool,
    pub cancel: bool,
}
pub fn participant_controls(activity: &Activity, actor: &str, now: u64) -> ParticipantControls {
    if is_organizer(activity, actor) || !crate::account_valid(actor) {
        return ParticipantControls::default();
    }
    let person = activity.people.iter().find(|p| p.account == actor);
    let reply = activity.pending_invitation_for(actor, now).is_ok();
    let reserved = activity.invitations.iter().any(|i| i.recipient == actor
        && i.reply == crate::Reply::Pending && i.delivery != crate::Delivery::Rejected);
    let open = now < activity.start && !activity.metadata.as_ref().is_some_and(|m| m.archived);
    ParticipantControls {
        register: open && !reserved && person.is_none_or(|p| p.status != crate::PersonStatus::Confirmed),
        reply,
        cancel: person.is_some_and(|p| matches!(p.status, crate::PersonStatus::Waiting | crate::PersonStatus::Confirmed)),
    }
}

/// Only complete, explicit whole-event phrases bypass a model request. Added
/// constraints or ambiguous amounts fall back to the existing dialogue.
pub fn explicit_whole(text: &str, activity: &Activity, actor: &str, preferences: Option<&PersonalPreferences>) -> Result<Option<(Preferences, &'static str)>> {
    let text=text.trim().trim_end_matches(['。','！','!']).replace('，',",");
    let (phrase,amount)=text.split_once(',').map(|(a,b)|(a.trim(),Some(b.trim()))).unwrap_or((text.as_str(),None));
    if !matches!(phrase,"参加整场"|"我参加整场"|"我想参加整场"|"我能全程参加"|"全程参加") {return Ok(None);}
    let group=if let Some(amount)=amount {
        let amounts=[("1人",1),("一个人",1),("2人",2),("两个人",2),("二人",2),("3人",3),("三个人",3),("4人",4),("四个人",4),("5人",5),("五个人",5),("6人",6),("六个人",6),("7人",7),("七个人",7),("8人",8),("八个人",8)];
        let Some((_,group))=amounts.iter().find(|(s,_)|*s==amount) else {return Ok(None);};Some(*group)
    }else{None};
    let (mut result,source)=whole_activity(activity,actor,preferences)?;
    if let Some(group)=group {result.group=group;}
    Ok(Some((result,if group.is_some(){"同行人数来自本次本人原话，未修改长期偏好"}else{source})))
}

pub fn whole_activity(
    activity: &Activity,
    actor: &str,
    preferences: Option<&PersonalPreferences>,
) -> Result<(Preferences, &'static str)> {
    activity.validate()?;
    if !crate::account_valid(actor) || is_organizer(activity, actor) {
        return Err("本场组织者请使用组织流程；报名必须使用成员本人身份".into());
    }
    let group = preferences
        .filter(|p| p.confirmed_at > 0)
        .and_then(|p| p.group)
        .unwrap_or(1);
    let p = Preferences {
        earliest: activity.start,
        latest: activity.end,
        group,
    };
    p.validate()?;
    Ok((
        p,
        if preferences.is_some_and(|p| p.confirmed_at > 0 && p.group.is_some()) {
            "同行人数来自本人确认的偏好，请核对"
        } else {
            "同行人数暂填1人，请核对"
        },
    ))
}
