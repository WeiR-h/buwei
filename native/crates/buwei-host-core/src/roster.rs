//! An organizer-only export of a verified snapshot, never personal intentions.
use crate::{Activity, Delivery, PersonStatus, Reply, Result, calendar};
pub fn status(a: &Activity, account: &str) -> &'static str {
    if let Some(i) = a.invitations.iter().rev().find(|i| {
        i.recipient == account && i.reply == Reply::Pending && i.delivery != Delivery::Rejected
    }) {
        return match i.delivery {
            Delivery::Unknown | Delivery::Pending => "邀请待核实",
            Delivery::Delivered => "名额已保留，等待本人回复",
            Delivery::Rejected => unreachable!(),
        };
    }
    match a
        .people
        .iter()
        .find(|p| p.account == account)
        .map(|p| &p.status)
    {
        Some(PersonStatus::Waiting) => "候补中",
        Some(PersonStatus::Confirmed) => "报名已确认",
        Some(PersonStatus::Declined) => "已拒绝",
        Some(PersonStatus::Cancelled) => "已取消",
        Some(PersonStatus::Expired) => "邀请已过期",
        None => "尚未报名",
    }
}
fn cell(s: &str) -> String {
    let s = s.replace('\r', " ").replace('\n', " ");
    let value = if s.trim_start().starts_with(['=', '+', '-', '@']) {
        format!("'{s}")
    } else {
        s
    };
    format!("\"{}\"", value.replace('"', "\"\""))
}
pub fn csv(a: &Activity, actor: &str, exported_at: u64, verified_at: u64) -> Result<Vec<u8>> {
    a.validate()?;
    if actor != a.owner
        || verified_at == 0
        || verified_at > exported_at
        || exported_at.saturating_sub(verified_at) > 120
    {
        return Err("请由组织者先同步并核验本场名单".into());
    }
    let id = a
        .metadata
        .as_ref()
        .ok_or("历史活动请从旧版导出")?
        .activity_id
        .clone();
    let campus = a.metadata.as_ref().and_then(|m| m.campus.as_ref());
    let mut out="\u{feff}活动编号,活动名称,导出北京时间,核验北京时间,报名序号,显示名,Matrix身份,同行人数,最早可用,最晚可用,报名状态,活动类型".to_owned();
    if campus.is_some() { out.push_str(",学校校区,面向人群,校园主题,报名截止北京时间,每份报名人数,音乐节用途"); }
    out.push_str("\r\n");
    for p in a.ordered_people() {
        let mut fields = vec![
            id.clone(),
            a.title.clone(),
            calendar::display(exported_at),
            calendar::display(verified_at),
            p.joined.to_string(),
            p.name.clone(),
            p.account.clone(),
            p.preferences.group.to_string(),
            calendar::display(p.preferences.earliest),
            calendar::display(p.preferences.latest),
            status(a, &p.account).into(),
            crate::activity_types::name(&a.metadata.as_ref().unwrap().template).into(),
        ];
        if let Some(c) = campus { fields.extend([c.campus.clone(), c.audience.clone(), c.topic.clone(), c.signup_deadline.clone(), c.team_size.map(|n| n.to_string()).unwrap_or_default(), c.role.map(|r| r.name().to_string()).unwrap_or_default()]); }
        out.push_str(&fields.iter().map(|s| cell(s)).collect::<Vec<_>>().join(","));
        out.push_str("\r\n");
    }
    Ok(out.into_bytes())
}
pub fn visible(a: &Activity, actor: &str) -> String {
    let owner = actor == a.owner;
    a.ordered_people()
        .iter()
        .filter(|p| owner || p.account == actor)
        .map(|p| {
            format!(
                "报名序号 {} · {} · {} 人 · {}\n账号：{}\n{} 至 {}",
                p.joined,
                p.name,
                p.preferences.group,
                status(a, &p.account),
                p.account,
                calendar::display(p.preferences.earliest),
                calendar::display(p.preferences.latest)
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}
