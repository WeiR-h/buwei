//! Rules are bound to native consent; guest inputs cannot issue this authority.
use crate::{Activity, Result};
use action_receipts::{Action, Grant};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    pub invitation_minutes: u8,
    pub quiet_start: u8,
    pub quiet_end: u8,
    pub max_invitations: u8,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            invitation_minutes: 10,
            quiet_start: 8,
            quiet_end: 22,
            max_invitations: 30,
        }
    }
}
impl Settings {
    pub fn validate(&self) -> Result<()> {
        if !(5..=60).contains(&self.invitation_minutes)
            || self.quiet_start >= self.quiet_end
            || self.quiet_end > 24
            || !(1..=30).contains(&self.max_invitations)
        {
            return Err("邀请期限为 5–60 分钟，发送时段为 0–24 点，上限为 1–30 次".into());
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    pub policy_id: String,
    pub account: String,
    pub activity_id: String,
    pub room: String,
    pub rules_revision: u64,
    pub expires_at: u64,
    pub settings: Settings,
}
impl Policy {
    pub fn issue(
        grant: &Grant,
        activity: &Activity,
        settings: Settings,
        clock: u64,
        expires_at: u64,
    ) -> Result<Self> {
        grant
            .check("invite", clock)
            .map_err(|_| "自动补位授权已失效")?;
        settings.validate()?;
        let meta = activity.metadata.as_ref().ok_or("请先创建带日期的新活动")?;
        if grant.account() != activity.owner
            || expires_at <= clock
            || expires_at > clock + 3600
            || activity.start <= clock
            || meta.archived
        {
            return Err("自动补位的账号、活动或期限不合法".into());
        }
        Ok(Self {
            policy_id: action_receipts::new_id(),
            account: activity.owner.clone(),
            activity_id: meta.activity_id.clone(),
            room: activity.room.clone(),
            rules_revision: 1,
            expires_at,
            settings,
        })
    }
    pub fn digest(&self) -> String {
        hex::encode(Sha256::digest(
            serde_json::to_vec(self).expect("policy serializes"),
        ))
    }
    pub fn check(&self, grant: &Grant, activity: &Activity, clock: u64) -> Result<()> {
        grant
            .check("invite", clock)
            .map_err(|_| "自动补位授权已失效")?;
        self.settings.validate()?;
        if clock >= self.expires_at
            || self.account != grant.account()
            || self.account != activity.owner
            || self.room != activity.room
            || activity
                .metadata
                .as_ref()
                .is_none_or(|m| m.activity_id != self.activity_id || m.archived)
            || clock >= activity.start
            || activity.paused
        {
            return Err("自动补位已暂停，请核对活动与授权")?;
        }
        let hour = ((clock / 3600) + 8) % 24;
        if hour < self.settings.quiet_start as u64 || hour >= self.settings.quiet_end as u64 {
            return Err("当前不在已授权的发送时段".into());
        }
        Ok(())
    }
    pub fn action(&self, grant: &Grant, activity: &Activity, clock: u64) -> Result<Action> {
        self.check(grant, activity, clock)?;
        let who = activity.candidate().ok_or("当前没有可邀请的候补")?;
        let until = (clock + self.settings.invitation_minutes as u64 * 60).min(activity.start);
        Ok(Action {
            permission: "invite".into(),
            target: activity.room.clone(),
            summary: format!(
                "按已确认规则邀请候补；完整同行人数保留；截止 {}",
                crate::calendar::display(until)
            ),
            payload: serde_json::json!({"person":who,"until":until}),
        })
    }
}
