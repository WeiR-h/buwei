//! Account roles and registration defaults depend on this activity, not a launcher.
use crate::{Activity, Preferences, Result, assistance::PersonalPreferences};

pub fn is_organizer(activity: &Activity, actor: &str) -> bool {
    activity.owner == actor
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
