//! Activity cards carry pointers and display data, never credentials or consent.
use super::*;
use serde::{Deserialize, Serialize};
pub(crate) const MSGTYPE: &str = "org.buwei.activity.card";
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct Card {
    pub protocol: u8,
    pub activity_id: String,
    pub room: String,
    pub title: String,
    pub start: u64,
    pub end: u64,
    pub location: String,
    pub free: usize,
}
impl Card {
    pub fn from_activity(a: &Activity) -> Result<Self> {
        let m = a.metadata.as_ref().ok_or("历史活动请使用原入口")?;
        a.validate()?;
        Ok(Self {
            protocol: 2,
            activity_id: m.activity_id.clone(),
            room: a.room.clone(),
            title: a.title.clone(),
            start: a.start,
            end: a.end,
            location: m.location.clone(),
            free: a.free(),
        })
    }
    pub fn body(&self) -> String {
        format!(
            "[补位活动] {}\n{} 至 {} · 北京时间\n地点：{}\n当前可用 {} 个名额\n在补位中查看并报名。\n客户端下载：https://github.com/WeiR-h/buwei/releases/latest",
            self.title,
            buwei_host_core::calendar::display(self.start),
            buwei_host_core::calendar::display(self.end),
            self.location,
            self.free
        )
    }
}
pub(crate) fn content(op: &Operation) -> Value {
    json!({"msgtype":MSGTYPE,"body":op.action.payload["body"],"buwei":op.action.payload["card"],"org.buwei.action":{"operation_id":op.id,"digest":op.digest,"action":op.action}})
}
pub(crate) struct CardAdapter {
    pub runtime: Arc<Runtime>,
    pub client: Client,
    pub state: PathBuf,
}
impl CardAdapter {
    fn a(&self) -> action_receipts::Result<Activity> {
        Store::open(&self.state)
            .and_then(|s| s.load())
            .map_err(|_| Error::Integrity)?
            .ok_or(Error::Integrity)
    }
    fn membership(&self, room: &OwnedRoomId, recipient: &str) -> Result<bool> {
        let response = self
            .runtime
            .block_on(async {
                self.client
                    .send(
                        matrix_sdk::ruma::api::client::state::get_state_events::v3::Request::new(
                            room.clone(),
                        ),
                    )
                    .await
            })
            .map_err(|_| "活动成员状态待核实")?;
        Ok(response.room_state.iter().any(|raw| {
            serde_json::from_str::<Value>(raw.json().get()).is_ok_and(|v| {
                v["type"] == "m.room.member"
                    && v["state_key"] == recipient
                    && matches!(v["content"]["membership"].as_str(), Some("join" | "invite"))
            })
        }))
    }
}
impl Adapter for CardAdapter {
    fn validate(&self, action: &Action, revision: u64) -> action_receipts::Result<()> {
        let a = self.a()?;
        let recipient = action.payload["recipient"]
            .as_str()
            .ok_or(Error::Integrity)?;
        if action.permission != "share_card"
            || account(&self.client) != a.owner
            || a.revision != revision
            || Card::from_activity(&a).map_err(|_| Error::Integrity)?
                != serde_json::from_value::<Card>(action.payload["card"].clone())
                    .map_err(|_| Error::Integrity)?
            || action.payload["body"]
                != Card::from_activity(&a)
                    .map_err(|_| Error::Integrity)?
                    .body()
            || !self.client.rooms().iter().any(|r| {
                r.room_id().as_str() == action.target
                    && r.direct_targets().iter().any(|id| id.as_str() == recipient)
            })
        {
            return Err(Error::Conflict);
        }
        Ok(())
    }
    fn dispatch(&self, op: &Operation) -> Dispatch {
        let result = (|| {
            self.validate(&op.action, op.revision)
                .map_err(|_| "活动分享预览已变化")?;
            #[cfg(feature = "full-host")]
            if rinx_bridge::official_mode() {
                rinx_bridge::ensure_current(&self.client)?;
            }
            let a = self.a().map_err(|_| "活动不可核实")?;
            let activity_room =
                OwnedRoomId::try_from(a.room.as_str()).map_err(|_| "活动房间不合法")?;
            let recipient = op.action.payload["recipient"]
                .as_str()
                .ok_or("接收者缺失")?;
            if !self.membership(&activity_room, recipient)? {
                let user = matrix_sdk::ruma::OwnedUserId::try_from(recipient)
                    .map_err(|_| "接收者不合法")?;
                let request=matrix_sdk::ruma::api::client::membership::invite_user::v3::Request::new(activity_room.clone(),matrix_sdk::ruma::api::client::membership::invite_user::v3::InvitationRecipient::UserId(matrix_sdk::ruma::api::client::membership::invite_user::v3::InviteUserId::new(user)));
                self.runtime
                    .block_on(async {
                        self.client
                            .send(request)
                            .with_request_config(
                                RequestConfig::new()
                                    .disable_retry()
                                    .timeout(Duration::from_secs(10)),
                            )
                            .await
                    })
                    .map_err(|_| "成员邀请结果待核实")?;
                if !self.membership(&activity_room, recipient)? {
                    return Err("成员邀请尚未核实".into());
                }
            }
            let room =
                OwnedRoomId::try_from(op.action.target.as_str()).map_err(|_| "分享房间不合法")?;
            channel::publish(&self.runtime, &self.client, &room, op, "m.room.message")
        })();
        match result {
            Ok(e) => Dispatch::Verified(e),
            Err(e) => Dispatch::Uncertain(e),
        }
    }
    fn lookup(&self, op: &Operation) -> action_receipts::Result<Option<Evidence>> {
        let room =
            OwnedRoomId::try_from(op.action.target.as_str()).map_err(|_| Error::Integrity)?;
        channel::lookup(&self.runtime, &self.client, &room, op, "m.room.message")
    }
}
