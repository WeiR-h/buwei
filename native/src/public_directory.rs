//! Explicitly join a shared public directory; activity rooms remain invitation-only.
use super::*;
pub(super) struct DirectoryPreview {
    pub nonce: String,
    pub account: String,
    pub alias: String,
    pub room: Option<String>,
    pub expires: u64,
    pub grant: Grant,
    pub summary: String,
}
#[cfg(feature = "full-host")]
impl Controller {
    fn public_alias(&self) -> Result<String> {
        let server = self
            .actor()
            .split_once(':')
            .ok_or("账号服务器不可用")?
            .1
            .to_owned();
        #[cfg(feature = "acceptance")]
        if let Ok(alias) = std::env::var("BUWEI_PUBLIC_TEST_ALIAS") {
            if alias.starts_with("#buwei-test-") && alias.ends_with(&format!(":{server}")) {
                return Ok(alias);
            }
            return Err("验收公开大厅别名不合法".into());
        }
        Ok(format!("#buwei-events:{server}"))
    }
    fn resolve_public(&self, alias: &str) -> Result<Option<String>> {
        let alias = alias.parse().map_err(|_| "公开大厅别名不合法")?;
        let result = self.rt.block_on(async {
            self.active()
                .send(matrix_sdk::ruma::api::client::alias::get_alias::v3::Request::new(alias))
                .with_request_config(
                    RequestConfig::new()
                        .disable_retry()
                        .timeout(Duration::from_secs(10)),
                )
                .await
        });
        match result {
            Ok(r) => Ok(Some(r.room_id.to_string())),
            Err(e)
                if e.client_api_error_kind().is_some_and(|kind| {
                    matches!(kind, matrix_sdk::ruma::api::error::ErrorKind::NotFound)
                }) =>
            {
                Ok(None)
            }
            Err(_) => Err("公开大厅定位暂不可核验，请稍后重试；未创建房间".into()),
        }
    }
    pub(super) fn public_directory_room(&self) -> Result<String> {
        let saved: Value = serde_json::from_slice(
            &std::fs::read(self.data.join("public-directory.json"))
                .map_err(|_| "请先进入公开活动大厅")?,
        )
        .map_err(|_| "公开大厅记录不合法")?;
        let alias = self.public_alias()?;
        let room = saved["room"].as_str().ok_or("公开大厅记录不合法")?;
        if saved["alias"] != alias || self.resolve_public(&alias)?.as_deref() != Some(room) {
            return Err("公开大厅来源已变化，请重新确认进入".into());
        }
        self.verify_directory(room)?;
        Ok(room.to_owned())
    }
    pub(super) fn verify_directory(&self, room: &str) -> Result<()> {
        let states =
            community_access::community_states(&self.rt, self.active(), room, &self.actor())?;
        buwei_host_core::public_directory::verify(&states)?;
        Ok(())
    }
    pub(super) fn preview_public_directory(&mut self) -> Result<String> {
        let grant = self.g("read")?;
        if let Ok(room) = self.public_directory_room() {
            self.watch(&room)?;
            self.read_community(&room)?;
            self.public_mode = true;
            self.public_directory_preview = None;
            return Ok("已打开本人已加入并核验的公开活动大厅。".into());
        }
        let alias = self.public_alias()?;
        let room = self.resolve_public(&alias)?;
        let summary = format!(
            "本人账号：{}\n公开大厅：{}\n{}\n进入后，其他用户能看到你的大厅成员身份；公开卡片的时间、地点、组织者和加入申请对大厅成员可见。活动报名与私人偏好仍分别处理。",
            self.actor(),
            alias,
            if room.is_some() {
                "确认后加入并核验公开大厅。"
            } else {
                "当前服务器尚无该大厅；确认后创建可公开加入的大厅。"
            }
        );
        self.public_directory_preview = Some(DirectoryPreview {
            nonce: new_id(),
            account: self.actor(),
            alias,
            room,
            expires: now() + 120,
            grant,
            summary,
        });
        Ok("请核对公开范围后确认进入大厅。".into())
    }
    pub(super) fn confirm_public_directory(&mut self, nonce: &str) -> Result<String> {
        self.g("participate")?;
        let p = self
            .public_directory_preview
            .as_ref()
            .ok_or("请先核对公开大厅范围")?;
        p.grant
            .check("read", now())
            .map_err(|_| "公开大厅预览授权已失效")?;
        if p.nonce != nonce || p.account != self.actor() || p.expires <= now() {
            return Err("公开大厅预览已失效".into());
        }
        let alias = p.alias.clone();
        let expected = p.room.clone();
        let found = self.resolve_public(&alias)?;
        if expected.is_some() && found != expected {
            return Err("公开大厅定位已变化，请重新核对".into());
        }
        let room = if let Some(room) = found {
            room
        } else {
            let pending = self.data.join("pending-public-directory.json");
            if pending.exists() {
                return Err("原公开大厅创建仍待核实，未重复创建；刷新预览沿原别名核实。".into());
            }
            let setup = new_id();
            std::fs::write(
                &pending,
                json!({"account":self.actor(),"alias":alias,"setup_id":setup,"created_at":now()})
                    .to_string(),
            )
            .map_err(|_| "公开大厅创建记录不可保存")?;
            let mut request = create_room::v3::Request::new();
            request.name = Some("补位 · 公开活动大厅".into());
            request.room_alias_name = Some(
                alias
                    .split_once(':')
                    .ok_or("别名不合法")?
                    .0
                    .trim_start_matches('#')
                    .into(),
            );
            request.preset = Some(create_room::v3::RoomPreset::PublicChat);
            request.visibility = matrix_sdk::ruma::api::client::room::Visibility::Public;
            request.topic =
                Some("所有补位用户可查看活动并以本人身份申请；活动房间与报名名额分别核验。".into());
            for state in [
                json!({"type":"org.buwei.directory","state_key":"","content":{"protocol":1,"kind":"public","creator":self.actor(),"setup_id":setup}}),
                json!({"type":"m.room.history_visibility","state_key":"","content":{"history_visibility":"shared"}}),
            ] {
                request.initial_state.push(Raw::from_json(
                    serde_json::value::to_raw_value(&state).map_err(|_| "大厅声明不可用")?,
                ));
            }
            self.rt
                .block_on(async {
                    self.active()
                        .send(request)
                        .with_request_config(
                            RequestConfig::new()
                                .disable_retry()
                                .timeout(Duration::from_secs(15)),
                        )
                        .await
                })
                .map_err(|_| "公开大厅创建结果待核实，沿原别名恢复；不会自动重建")?
                .room_id
                .to_string()
        };
        let room_id: OwnedRoomId = room.parse().map_err(|_| "公开大厅房间不合法")?;
        self.rt
            .block_on(self.active().join_room_by_id(&room_id))
            .map_err(|_| "大厅加入结果待核实，原定位保留")?;
        self.verify_directory(&room)?;
        std::fs::write(
            self.data.join("public-directory.json"),
            json!({"room":room,"alias":alias}).to_string(),
        )
        .map_err(|_| "公开大厅定位不可保存")?;
        self.watch(&room)?;
        self.read_community(&room)?;
        self.public_mode = true;
        self.public_directory_preview = None;
        Ok("已核验并进入公开活动大厅，可挑选活动；加入活动房间与报名分别确认。".into())
    }
}
