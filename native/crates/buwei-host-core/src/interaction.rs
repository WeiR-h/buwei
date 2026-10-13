//! Account roles and registration defaults depend on this activity, not a launcher.
use crate::{Activity, Preferences, Result, assistance::PersonalPreferences};

pub fn is_organizer(activity: &Activity, actor: &str) -> bool {
    activity.owner == actor
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
