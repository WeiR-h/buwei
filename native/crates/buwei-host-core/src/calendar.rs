//! Explicit Beijing dates. Legacy hour-only records are never assigned a date.
use crate::Result;
use serde::{Deserialize, Serialize};
pub const MIN_TIME: u64 = 1_577_836_800;
pub const MAX_TIME: u64 = 4_102_444_800;
fn leap(y: u32) -> bool {
    y % 4 == 0 && (y % 100 != 0 || y % 400 == 0)
}
fn month_days(y: u32, m: u32) -> u32 {
    match m {
        2 => {
            if leap(y) {
                29
            } else {
                28
            }
        }
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}
pub fn parse(text: &str) -> Result<u64> {
    let text = text.trim().replace('T', " ");
    if text.len() != 16
        || !text.is_ascii()
        || &text[4..5] != "-"
        || &text[7..8] != "-"
        || &text[10..11] != " "
        || &text[13..14] != ":"
    {
        return Err("请填写日期和时间，例如 2026-10-10 19:30".into());
    }
    let number = |a, b| {
        text[a..b]
            .parse::<u32>()
            .map_err(|_| "日期或时间不合法".to_string())
    };
    let (y, m, d, h, n) = (
        number(0, 4)?,
        number(5, 7)?,
        number(8, 10)?,
        number(11, 13)?,
        number(14, 16)?,
    );
    if !(2020..2100).contains(&y)
        || !(1..=12).contains(&m)
        || d == 0
        || d > month_days(y, m)
        || h > 23
        || n > 59
    {
        return Err("日期或时间不合法".into());
    }
    let mut days = 0u64;
    for year in 1970..y {
        days += if leap(year) { 366 } else { 365 };
    }
    for month in 1..m {
        days += month_days(y, month) as u64;
    }
    days += (d - 1) as u64;
    Ok(days * 86400 + h as u64 * 3600 + n as u64 * 60 - 8 * 3600)
}
pub fn display(time: u64) -> String {
    if time <= 24 {
        return format!("{time}:00");
    }
    if !(MIN_TIME..MAX_TIME).contains(&time) {
        return "时间待核对".into();
    }
    let local = time + 8 * 3600;
    let mut days = local / 86400;
    let mut y = 1970;
    loop {
        let count = if leap(y) { 366 } else { 365 };
        if days < count {
            break;
        }
        days -= count;
        y += 1;
    }
    let mut m = 1;
    while days >= month_days(y, m) as u64 {
        days -= month_days(y, m) as u64;
        m += 1;
    }
    format!(
        "{y:04}-{m:02}-{:02} {:02}:{:02}",
        days + 1,
        (local / 3600) % 24,
        (local / 60) % 60
    )
}
pub fn interval(start: u64, end: u64) -> bool {
    start >= MIN_TIME
        && end < MAX_TIME
        && start < end
        && end - start <= 7 * 86400
        && start % 60 == 0
        && end % 60 == 0
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Metadata {
    pub activity_id: String,
    pub template: String,
    pub location: String,
    pub description: String,
    #[serde(default)]
    pub archived: bool,
    #[serde(default)]
    pub metrics: Metrics,
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Metrics {
    pub cancelled_people: u32,
    pub successful_invitations: u32,
    pub response_seconds: u64,
    pub timed_responses: u32,
    pub invitation_times: std::collections::BTreeMap<String, u64>,
}
impl Metadata {
    pub fn validate(&self) -> Result<()> {
        if self.metrics.invitation_times.len() > 1000
            || self.metrics.successful_invitations > 1000
            || self.metrics.timed_responses > self.metrics.successful_invitations
            || self.metrics.invitation_times.iter().any(|(id, time)| {
                id.len() != 32
                    || !id.bytes().all(|b| b.is_ascii_hexdigit())
                    || *time < MIN_TIME
                    || *time >= MAX_TIME
            })
        {
            return Err("活动统计记录不合法".into());
        }
        if self.activity_id.len() != 32
            || !self.activity_id.bytes().all(|b| b.is_ascii_hexdigit())
            || !matches!(
                self.template.as_str(),
                "badminton" | "boardgame" | "reading" | "custom"
            )
            || self.location.trim().is_empty()
            || self.location.chars().count() > 120
            || self.description.chars().count() > 1200
            || self.location.chars().any(char::is_control)
        {
            return Err("活动地点、模板或说明不合法".into());
        }
        Ok(())
    }
}
pub fn template(kind: &str) -> (&'static str, u8, &'static str) {
    match kind {
        "badminton" => ("羽毛球活动", 6, "请提前到场，自备球拍。"),
        "boardgame" => ("桌游活动", 6, "请在报名时填写同行人数。"),
        "reading" => ("读书会", 12, "请准备一句想分享的阅读感受。"),
        _ => ("社群活动", 6, ""),
    }
}
