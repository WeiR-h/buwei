//! Typed intents share the receipt journal; identities come from the host.
use crate::{Activity, Preferences, Result};
use action_receipts::{Action, Operation};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Intent {
    Join {
        preferences: Preferences,
    },
    Reply {
        invitation_id: String,
        invitation_event: String,
        accept: bool,
    },
    Cancel,
}
impl Intent {
    pub fn validate(&self, activity: &Activity, actor: &str, clock: u64) -> Result<()> {
        if activity.metadata.as_ref().is_some_and(|m| m.archived)
            || activity.metadata.is_some() && clock >= activity.end
        {
            return Err("活动已结束，报名入口已关闭".into());
        }
        if actor == activity.owner {
            return Err("组织者不能代替参与者操作".into());
        }
        match self {
            Self::Join { preferences } => {
                if activity.metadata.is_some() && clock >= activity.start {
                    return Err("活动已开始，停止新增报名".into());
                }
                activity
                    .clone()
                    .join_own(actor.into(), "参与者".into(), preferences.clone())
            }
            Self::Reply {
                invitation_id,
                invitation_event,
                ..
            } => {
                let i = activity.pending_invitation_for(actor, clock)?;
                if &i.operation_id != invitation_id
                    || i.server_event.as_deref() != Some(invitation_event.as_str())
                {
                    return Err("本人邀请已变化，请重新预览".into());
                }
                Ok(())
            }
            Self::Cancel => activity.clone().cancel_own(actor),
        }
    }
    pub fn event_type(&self) -> &'static str {
        match self {
            Self::Join { .. } => "org.buwei.join",
            Self::Reply { .. } => "org.buwei.reply",
            Self::Cancel => "org.buwei.cancel",
        }
    }
    pub fn action(&self, activity: &Activity) -> Action {
        let summary = match self {
            Self::Join { preferences: p } => format!(
                "登记本人候补：{} 至 {}，{} 人",
                crate::calendar::display(p.earliest),
                crate::calendar::display(p.latest),
                p.group
            ),
            Self::Reply { accept, .. } => if *accept {
                "本人接受这次邀请"
            } else {
                "本人拒绝这次邀请"
            }
            .into(),
            Self::Cancel => "取消本人的报名及整组席位".into(),
        };
        Action {
            permission: "participate".into(),
            target: activity.room.clone(),
            summary,
            payload: json!(self),
        }
    }
    pub fn parse(action: &Action) -> Result<Self> {
        if action.permission != "participate" {
            return Err("参与者操作权限不匹配".into());
        }
        serde_json::from_value(action.payload.clone())
            .map_err(|_| "参与者操作格式不符合约定".into())
    }
    pub fn wire(&self, op: &Operation) -> Value {
        let mut v = match self {
            Self::Join { preferences } => json!({"preferences":preferences}),
            // Keep the original invitation reference for existing rooms. The
            // participant's own transaction has a distinct action_id.
            Self::Reply {
                invitation_id,
                invitation_event,
                accept,
            } => {
                json!({"operation_id":invitation_id,"accept":accept,"m.relates_to":{"m.in_reply_to":{"event_id":invitation_event}}})
            }
            Self::Cancel => json!({"activity":op.action.target}),
        };
        v["action_id"] = json!(op.id);
        v["action_digest"] = json!(op.digest);
        v
    }
}
