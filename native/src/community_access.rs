//! Community entry is separate from registration and seat allocation.
use super::*;
use buwei_host_core::discovery::{
    AccessPolicy, Announcement, DiscoveryStore, EntryRequest, Filter, Listing,
};
use serde::Deserialize;
#[derive(Clone, Deserialize)]
#[serde(tag = "command", content = "value", deny_unknown_fields)]
pub(crate) enum DiscoveryCommand {
    PreviewPublicDirectory,
    ConfirmPublicDirectory(String),
    CommunityMode,
    PreviewPublicPublish { withdrawn: bool },
    LoadCommunities,
    Watch(String),
    Refresh,
    Filter(Filter),
    PreviewPublish { community: String, withdrawn: bool },
    ConfirmPublish(String),
    PreviewAccess(String),
    ConfirmAccess(String),
    PauseAccess,
    PreviewEntry(String),
    ConfirmEntry(String),
    Reconcile,
}
pub(super) struct AccessPreview {
    pub nonce: String,
    pub listing: Listing,
    pub expires: u64,
}
impl Controller {
    pub(super) fn discovery_store(&self) -> Result<DiscoveryStore> {
        DiscoveryStore::open(self.data.join("community-discovery.db"), &self.actor())
    }
    fn community_journal(&self) -> action_receipts::Result<Journal> {
        Journal::open(self.data.join("community-operations.db"))
    }
    fn cg(&self, permission: &str) -> Result<Grant> {
        let g = self.community_grant.as_ref().ok_or("请先授权当前账号")?;
        g.check(permission, now()).map_err(|_| "社群授权已失效")?;
        if g.account() != self.actor() {
            return Err("社群账号已变化".into());
        }
        Ok(g.clone())
    }
    pub(super) fn watch(&mut self, room: &str) -> Result<()> {
        self.g("read")?;
        #[cfg(feature = "full-host")]
        community_states(&self.rt, self.active(), room, &self.actor())?;
        if !self.watched_communities.iter().any(|r| r == room) {
            if self.watched_communities.len() >= 5 {
                return Err("本轮最多接入五个社群，请先使用已有社群".into());
            }
            self.watched_communities.push(room.into());
            std::fs::write(
                self.data.join("watched-communities.json"),
                serde_json::to_vec(&self.watched_communities).map_err(|_| "社群选择不可保存")?,
            )
            .map_err(|_| "社群选择不可保存")?;
        }
        Ok(())
    }
    pub(super) fn discovery_view(&self, v: &mut View) {
        v.public_mode = self.public_mode;
        v.public_room=std::fs::read(self.data.join("public-directory.json")).ok().and_then(|b|serde_json::from_slice::<Value>(&b).ok()).and_then(|v|v["room"].as_str().map(str::to_owned));
        v.public_directory_nonce = self.public_directory_preview.as_ref().filter(|p| p.account == self.actor() && p.expires > now() && p.grant.check("read",now()).is_ok()).map(|p| p.nonce.clone());
        v.public_directory_preview = self.public_directory_preview.as_ref().map(|p| p.summary.clone()).unwrap_or_default();
        v.communities = self.communities.clone();
        v.community_listings = self
            .discovery_store()
            .and_then(|s| s.list())
            .unwrap_or_default()
            .into_iter()
            .filter(|l| {
                self.watched_communities.contains(&l.community)
                    && self.discovery_filter.matches(l, now())
                    && (l.announcement.audience == buwei_host_core::discovery::Audience::Public) == self.public_mode
            })
            .collect();
        v.community_status = self.community_status.clone();
        v.community_operation = self.community_current.clone();
        v.community_preview = self
            .community_current
            .as_ref()
            .map(|op| {
                format!(
                    "本人账号：{}\n{}\n{}\n状态：{}",
                    op.account,
                    op.action.summary,
                    op.action.payload["body"]
                        .as_str()
                        .unwrap_or("申请只用于取得活动房间访问，不报名或占位"),
                    match op.status {
                        Status::Prepared => "待本人确认",
                        Status::Confirmed => "服务端已核验",
                        Status::Unknown | Status::Dispatching => "结果待核实，沿原编号恢复",
                        _ => "请重新准备",
                    }
                )
            })
            .unwrap_or_default();
        v.access_nonce = self
            .access_preview
            .as_ref()
            .filter(|p| p.expires > now())
            .map(|p| p.nonce.clone());
        v.access_preview=self.access_preview.as_ref().map(|p|format!("账号：{}\n活动：{}\n社群：{}\n只处理该社群当前成员本人提出的加入申请；最多30次房间邀请；不替成员报名或占位。到期不超过本次一小时授权及活动开始。",self.actor(),p.listing.announcement.title,p.listing.community)).unwrap_or_default();
        v.access_status = self
            .access_policies
            .values()
            .map(|p| {
                format!(
                    "{} · {} · 到期 {} · 已处理 {} 次",
                    p.activity_id,
                    if p.expires > now() {
                        "社群加入规则已启用"
                    } else {
                        "社群加入规则已暂停"
                    },
                    buwei_host_core::calendar::display(p.expires),
                    p.used.len()
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        let activities = buwei_host_core::catalog::Catalog::open(&self.data)
            .and_then(|s| s.list())
            .unwrap_or_default();
        v.entry_progress = self
            .community_journal()
            .ok()
            .and_then(|j| {
                self.community_grant
                    .as_ref()
                    .and_then(|g| j.recent(g, now()).ok())
            })
            .unwrap_or_default()
            .iter()
            .filter(|op| op.action.permission == "request_entry")
            .take(5)
            .map(|op| {
                let room = op.action.payload["listing"]["announcement"]["activity_room"]
                    .as_str()
                    .unwrap_or_default();
                let status = if op.status != Status::Confirmed {
                    "加入申请待核实；先查询原回执".into()
                } else if let Some(a) = activities.iter().find(|a| a.room == room) {
                    if a.people.iter().any(|p| p.account == self.actor()) {
                        buwei_host_core::roster::status(a, &self.actor()).to_owned()
                    } else {
                        "活动房间已核验；等待本人报名".into()
                    }
                } else {
                    "加入申请已送达；等待组织者邀请，再确认加入并报名".into()
                };
                format!("{} · {}", op.action.summary, status)
            })
            .collect::<Vec<_>>()
            .join("\n");
    }
}
#[cfg(not(feature = "full-host"))]
impl Controller {
    pub(super) fn apply_discovery(&mut self, _: DiscoveryCommand) -> Result<String> {
        Err("社群活动需要完整官方宿主".into())
    }
}
#[cfg(feature = "full-host")]
impl Controller {
    pub(super) fn apply_discovery(&mut self, cmd: DiscoveryCommand) -> Result<String> {
        self.g("read")?;
        match cmd {
            DiscoveryCommand::PreviewPublicDirectory => self.preview_public_directory(),
            DiscoveryCommand::ConfirmPublicDirectory(id) => self.confirm_public_directory(&id),
            DiscoveryCommand::CommunityMode => { self.public_mode=false; self.community_current=None; Ok("已切换到本人选择的社群活动。".into()) },
            DiscoveryCommand::PreviewPublicPublish { withdrawn } => {
                let community = self.public_directory_room()?;
                self.preview_listing(community, withdrawn, buwei_host_core::discovery::Audience::Public)
            }
            DiscoveryCommand::LoadCommunities => {
                self.communities = self
                    .active()
                    .joined_rooms()
                    .into_iter()
                    .filter(|r| r.direct_targets().is_empty())
                    .map(|r| {
                        (
                            r.room_id().to_string(),
                            r.name().unwrap_or_else(|| r.room_id().to_string()),
                        )
                    })
                    .collect();
                Ok("请选择本人已加入的社群。只有所选社群会读取活动卡片与加入申请。".into())
            }
            DiscoveryCommand::Watch(room) => {
                self.public_mode = false;
                self.community_current = None;
                self.access_preview = None;
                self.watch(&room)?;
                self.read_community(&room)?;
                Ok("社群已接入，可在列表中挑选活动。".into())
            }
            DiscoveryCommand::Refresh => {
                let groups = self.watched_communities.clone();
                for room in groups {
                    self.read_community(&room)?;
                }
                self.process_entries()?;
                Ok("所选社群的活动和加入申请已核验。".into())
            }
            DiscoveryCommand::Filter(f) => {
                f.validate()?;
                self.discovery_filter = f;
                Ok("活动筛选已更新。".into())
            }
            DiscoveryCommand::PreviewPublish {
                community,
                withdrawn,
            } => self.preview_listing(community, withdrawn, buwei_host_core::discovery::Audience::Community),
            DiscoveryCommand::ConfirmPublish(id) => {
                let op = self.execute_community_message(&id, "publish_community")?;
                if op.status == Status::Confirmed {
                    let raw = event(
                        &self.rt,
                        self.active(),
                        &op.action.target.parse().map_err(|_| "社群对象不合法")?,
                        op.receipt
                            .as_ref()
                            .ok_or("发布回执缺失")?
                            .evidence
                            .as_ref()
                            .ok_or("发布证据缺失")?
                            .external_id
                            .parse()
                            .map_err(|_| "发布事件不合法")?,
                    )?;
                    self.discovery_store()?
                        .observe(&op.action.target, &[raw], now())?;
                    self.link_plan_publication(&op)?;
                }
                Ok("社群发布记录已保存。自动处理加入申请需另行核对规则。".into())
            }
            DiscoveryCommand::PreviewAccess(key) => {
                let l = self.discovery_store()?.get(&key)?;
                community_states(&self.rt, self.active(), &l.community, &self.actor())?;
                self.check_owned_listing(&l)?;
                self.access_preview = Some(AccessPreview {
                    nonce: new_id(),
                    listing: l,
                    expires: now() + 120,
                });
                Ok("请核对社群加入规则，然后确认启用。".into())
            }
            DiscoveryCommand::ConfirmAccess(id) => {
                let p = self.access_preview.as_ref().ok_or("请先查看社群加入规则")?;
                if id != p.nonce || now() >= p.expires {
                    return Err("社群规则预览已失效".into());
                }
                self.check_owned_listing(&p.listing)?;
                let g = self.cg("admit_entry")?;
                let policy = AccessPolicy::issue(&g, &p.listing, now(), self.authorized_until)?;
                let key = p.listing.key();
                self.access_policies.insert(key.clone(), policy);
                self.access_permits.insert(
                    key.clone(),
                    self.automation_interlock.issue(&format!("community:{key}")),
                );
                self.access_preview = None;
                self.process_entries()?;
                Ok("该活动的社群加入规则已启用；成员仍亲自确认报名。".into())
            }
            DiscoveryCommand::PauseAccess => {
                self.access_policies.clear();
                for (key, _) in self.access_permits.drain() {
                    self.automation_interlock
                        .invalidate(&format!("community:{key}"));
                }
                self.access_preview = None;
                Ok("社群自动邀请已暂停，已发出的邀请继续核实。".into())
            }
            DiscoveryCommand::PreviewEntry(key) => {
                let g = self.cg("request_entry")?;
                let l = self.discovery_store()?.get(&key)?;
                let states=community_states(&self.rt, self.active(), &l.community, &self.actor())?;
                check_publication_scope(l.announcement.audience,&states)?;
                if !l.can_request(now()) {
                    return Err("本场已开始或已收起，请另选活动".into());
                }
                if let Some(a) = buwei_host_core::catalog::Catalog::open(&self.data)?
                    .list()?
                    .into_iter()
                    .find(|a| a.room == l.announcement.activity_room)
                {
                    if a.owner != l.announcement.organizer {
                        return Err("社群卡片与已核验组织者不一致".into());
                    }
                    self.select_activity(l.announcement.activity_id.clone())?;
                    self.community_current = None;
                    return Ok("已打开本人接入的活动，可查看详情并确认报名。".into());
                }
                let raw = event(
                    &self.rt,
                    self.active(),
                    &l.community.parse().map_err(|_| "社群不合法")?,
                    l.event_id.parse().map_err(|_| "卡片事件不合法")?,
                )?;
                if Listing::parse(&raw, &l.community, now())? != l {
                    return Err("卡片已变化，请刷新列表".into());
                }
                let action = Action {
                    permission: "request_entry".into(),
                    target: l.community.clone(),
                    summary: format!("申请查看并加入：{}", l.announcement.title),
                    payload: json!({"listing":l}),
                };
                self.prepare_community_action(&g, action, l.announcement.revision)?;
                Ok("核对本人的活动加入申请。此步不报名、不保留名额。".into())
            }
            DiscoveryCommand::ConfirmEntry(id) => {
                let op = self.execute_community_message(&id, "request_entry")?;
                if op.status == Status::Confirmed {
                    let l: Listing = serde_json::from_value(op.action.payload["listing"].clone())
                        .map_err(|_| "原申请信息不合法")?;
                    self.expected_card_owner = Some(l.announcement.organizer);
                    self.incoming_card =
                        Some((l.announcement.activity_room, l.announcement.activity_id));
                }
                Ok("加入申请已保存；收到房间邀请后确认加入，再确认本人报名。".into())
            }
            DiscoveryCommand::Reconcile => {
                self.reconcile_community()?;
                Ok("社群操作已沿原编号核实；尚不明确的记录保留。".into())
            }
        }
    }
    fn preview_listing(&mut self, community: String, withdrawn: bool, audience: buwei_host_core::discovery::Audience) -> Result<String> {
        let g=self.cg("publish_community")?;
        self.watch(&community)?;
        let states=community_states(&self.rt,self.active(),&community,&self.actor())?;
        check_publication_scope(audience,&states)?;
        self.sync_activity()?;
        let a=self.activity()?;
        if a.owner!=self.actor(){return Err("只有本场组织者可以发布活动".into());}
        let mut announcement=Announcement::from_activity(&a,withdrawn)?;
        if audience==buwei_host_core::discovery::Audience::Public {announcement.protocol=2;announcement.audience=audience;}
        let card=super::super::activity_card::Card::from_activity(&a)?;
        let scope=if audience==buwei_host_core::discovery::Audience::Public {"所有补位用户可进入公开大厅查看与申请"}else{"仅所选社群当前成员可见与申请"};
        let body=format!("{}\n可见范围：{}。{}",card.body(),scope,if withdrawn{"活动已收起，不接受新申请"}else{"申请只用于加入活动房间，报名与接受均由本人确认"});
        let action=Action{permission:"publish_community".into(),target:community,summary:format!("{}{}：{}",if withdrawn{"收起"}else{"发布"},if audience==buwei_host_core::discovery::Audience::Public{"公开活动"}else{"社群活动"},a.title),payload:json!({"announcement":announcement,"card":card,"body":body})};
        self.prepare_community_action(&g,action,a.revision)?;
        Ok("核对本场活动和公开 / 社群范围后确认发布。".into())
    }
    fn check_owned_listing(&self, l: &Listing) -> Result<()> {
        let states=community_states(&self.rt,self.active(),&l.community,&self.actor())?;
        check_publication_scope(l.announcement.audience,&states)?;
        let a = buwei_host_core::catalog::Catalog::open(&self.data)?
            .list()?
            .into_iter()
            .find(|a| a.room == l.announcement.activity_room)
            .ok_or("本账号没有这场组织者资料")?;
        if a.owner != self.actor()
            || a.metadata.as_ref().is_none_or(|m| {
                m.activity_id != l.announcement.activity_id
                    || m.archived
                    || m.template != l.announcement.template
                    || m.location != l.announcement.location
            })
            || a.start != l.announcement.start
            || a.end != l.announcement.end
            || a.capacity != l.announcement.capacity
            || a.start <= now()
            || l.announcement.organizer != self.actor()
            || l.announcement.withdrawn
        {
            return Err("活动来源、本人身份或期限不匹配".into());
        }
        Ok(())
    }
    fn prepare_community_action(&mut self, g: &Grant, action: Action, revision: u64) -> Result<()> {
        let mut j = self.community_journal().map_err(|_| "社群回执不可打开")?;
        if j.pending(g, now())
            .map_err(|_| "社群待核实操作不可读取")?
            .iter()
            .any(|o| o.action.target == action.target)
        {
            return Err("该社群仍有原操作待核实，请先恢复回执".into());
        }
        if let Some(op) = j
            .recent(g, now())
            .map_err(|_| "社群回执不可读取")?
            .into_iter()
            .find(|o| {
                o.action == action
                    && o.revision == revision
                    && (o.status == Status::Confirmed || Journal::preview_current(g, o, now()))
            })
        {
            self.community_current = Some(op);
            return Ok(());
        }
        self.community_current = Some(
            j.prepare(g, action, revision, now(), 120)
                .map_err(|_| "社群预览不可保存")?,
        );
        Ok(())
    }
    fn execute_community_message(&mut self, id: &str, permission: &str) -> Result<Operation> {
        let g = self.cg(permission)?;
        let op = self
            .community_current
            .as_ref()
            .filter(|o| o.id == id && o.action.permission == permission)
            .ok_or("请先预览确切社群操作")?
            .clone();
        let adapter = CommunityMessageAdapter {
            runtime: self.rt.clone(),
            client: self.active().clone(),
            data: self.data.clone(),
        };
        let mut j = self.community_journal().map_err(|_| "社群回执不可打开")?;
        if op.status == Status::Prepared {
            j.confirm(&g, &op.id, &adapter, now())
                .map_err(|_| "社群预览已失效")?;
        }
        let op = j
            .execute(&g, &op.id, &adapter, now())
            .map_err(|_| "社群发送需沿原编号恢复")?;
        self.community_current = Some(op.clone());
        Ok(op)
    }
    pub(super) fn read_community(&mut self, room: &str) -> Result<()> {
        self.g("read")?;
        community_states(&self.rt, self.active(), room, &self.actor())?;
        let mut s = self.discovery_store()?;
        let cursor = s.cursor(room)?;
        let values = official_sync::timeline_since(
            &self.rt,
            self.active(),
            &room.parse().map_err(|_| "社群不合法")?,
            cursor.as_deref(),
        )?;
        s.apply(room, &values, now())?;
        self.community_status = format!(
            "所选社群活动已核验 · {}",
            buwei_host_core::calendar::display(now())
        );
        Ok(())
    }
    pub(super) fn discovery_tick(&mut self) {
        if self.watched_communities.is_empty() || now().saturating_sub(self.last_discovery) < 2 {
            return;
        }
        self.last_discovery = now();
        let room = self.watched_communities
            [self.discovery_rotation % self.watched_communities.len()]
        .clone();
        self.discovery_rotation += 1;
        if let Err(e) = self
            .read_community(&room)
            .and_then(|_| self.reconcile_community())
            .and_then(|_| self.process_entries())
        {
            self.community_status = format!("社群处理暂停：{e}；原申请和回执保留。");
        }
        // Restore the original card after restart; entry still requires consent.
        if self.incoming_card.is_none() {
            if let (Ok(j), Some(g)) = (self.community_journal(), self.community_grant.as_ref()) {
                if let Ok(ops) = j.recent(g, now()) {
                    if let Some(op) = ops.iter().find(|o| {
                        o.action.permission == "request_entry" && o.status == Status::Confirmed
                    }) {
                        if let Ok(l) =
                            serde_json::from_value::<Listing>(op.action.payload["listing"].clone())
                        {
                            if !buwei_host_core::catalog::Catalog::open(&self.data)
                                .and_then(|s| s.list())
                                .unwrap_or_default()
                                .iter()
                                .any(|a| a.room == l.announcement.activity_room)
                            {
                                self.expected_card_owner = Some(l.announcement.organizer);
                                self.incoming_card = Some((
                                    l.announcement.activity_room,
                                    l.announcement.activity_id,
                                ));
                            }
                        }
                    }
                }
            }
        }
    }
    fn process_entries(&mut self) -> Result<()> {
        let g = self.cg("admit_entry")?;
        let store = self.discovery_store()?;
        for (r, status, operation) in store.requests()? {
            let key = format!("{}|{}", r.community, r.activity_id);
            let Some(policy) = self.access_policies.get(&key).cloned() else {
                continue;
            };
            let permit = self
                .access_permits
                .get(&key)
                .cloned()
                .ok_or("社群规则需重新确认")?;
            if !permit.valid() {
                continue;
            }
            let states = community_states(&self.rt, self.active(), &r.community, &self.actor())?;
            let member = states.iter().any(|e| {
                e["type"] == "m.room.member"
                    && e["state_key"] == r.actor
                    && e["content"]["membership"] == "join"
            });
            if policy.check(&g, &r, member, now()).is_err() {
                continue;
            }
            let listing = store.get(&key)?;
            if listing.event_id != policy.announcement_event {
                continue;
            }
            self.check_owned_listing(&listing)?;
            let a = buwei_host_core::catalog::Catalog::open(&self.data)?
                .list()?
                .into_iter()
                .find(|a| a.room == r.activity_room)
                .ok_or("活动资料缺失")?;
            let member_state =
                activity_member_state(&self.rt, self.active(), &r.activity_room, &r.actor)?;
            if operation.is_none()
                && member_state.as_ref().is_some_and(|v| {
                    matches!(v["content"]["membership"].as_str(), Some("join" | "invite"))
                })
            {
                store.set_request(&r.event_id, "invited", None)?;
                continue;
            }
            let mut j = self
                .community_journal()
                .map_err(|_| "加入邀请回执不可打开")?;
            let adapter = AdmissionAdapter {
                runtime: self.rt.clone(),
                client: self.active().clone(),
                state: buwei_host_core::catalog::Catalog::open(&self.data)?.path(&r.activity_id)?,
                grant: g.clone(),
                policy: policy.clone(),
                permit,
            };
            let op = if let Some(id) = operation {
                let old = j
                    .receipt(&g, &id, now())
                    .map_err(|_| "原加入邀请不可恢复")?;
                if matches!(
                    old.status,
                    Status::Dispatching | Status::Unknown | Status::Confirmed
                ) {
                    j.reconcile(&g, &id, &adapter, now())
                        .map_err(|_| "原加入邀请仍待核实")?
                } else if Journal::preview_current(&g, &old, now()) {
                    if old.status == Status::Prepared {
                        j.confirm(&g, &id, &adapter, now())
                            .map_err(|_| "加入邀请确认失效")?;
                    }
                    j.execute(&g, &id, &adapter, now())
                        .map_err(|_| "加入邀请执行失效")?
                } else {
                    store.set_request(&r.event_id, "pending", None)?;
                    continue;
                }
            } else {
                let action = Action {
                    permission: "admit_entry".into(),
                    target: r.activity_room.clone(),
                    summary: format!(
                        "核验社群本人申请后邀请 {} 查看 {}；不占位",
                        r.actor, a.title
                    ),
                    payload: json!({"request":r,"listing":listing,"previous_member_event":member_state.as_ref().and_then(|v|v["event_id"].as_str())}),
                };
                let op = j
                    .prepare(&g, action, a.revision, now(), 120)
                    .map_err(|_| "加入邀请预执行记录不可保存")?;
                store.set_request(&r.event_id, "unknown", Some(&op.id))?;
                j.confirm(&g, &op.id, &adapter, now())
                    .map_err(|_| "加入邀请规则失效")?;
                self.access_policies
                    .get_mut(&key)
                    .ok_or("社群规则已失效")?
                    .used
                    .insert(r.event_id.clone());
                j.execute(&g, &op.id, &adapter, now())
                    .map_err(|_| "加入邀请沿原编号待核实")?
            };
            store.set_request(
                &r.event_id,
                if op.status == Status::Confirmed {
                    "invited"
                } else {
                    "unknown"
                },
                Some(&op.id),
            )?;
            if status == "unknown" && op.status != Status::Confirmed {
                self.community_status = "加入邀请仍待核实，保持原操作编号。".into();
            }
        }
        Ok(())
    }
    fn reconcile_community(&mut self) -> Result<()> {
        let g = self.cg("read")?;
        let mut j = self.community_journal().map_err(|_| "社群回执不可打开")?;
        let adapter = CommunityMessageAdapter {
            runtime: self.rt.clone(),
            client: self.active().clone(),
            data: self.data.clone(),
        };
        for op in j
            .pending(&g, now())
            .map_err(|_| "社群回执不可读取")?
            .into_iter()
            .filter(|o| {
                matches!(
                    o.action.permission.as_str(),
                    "request_entry" | "publish_community"
                )
            })
        {
            let recovered = j
                .reconcile(&g, &op.id, &adapter, now())
                .map_err(|_| "社群回执尚不可核实")?;
            if recovered.status == Status::Confirmed
                && recovered.action.permission == "publish_community"
            {
                self.link_plan_publication(&recovered)?;
            }
            self.community_current = Some(recovered);
        }
        for op in j
            .pending(&g, now())
            .map_err(|_| "加入邀请回执不可读取")?
            .into_iter()
            .filter(|o| o.action.permission == "admit_entry")
        {
            let r: EntryRequest = serde_json::from_value(op.action.payload["request"].clone())
                .map_err(|_| "原申请不合法")?;
            let l: Listing = serde_json::from_value(op.action.payload["listing"].clone())
                .map_err(|_| "原卡片不合法")?;
            let adapter = AdmissionAdapter {
                runtime: self.rt.clone(),
                client: self.active().clone(),
                state: buwei_host_core::catalog::Catalog::open(&self.data)?.path(&r.activity_id)?,
                grant: g.clone(),
                policy: AccessPolicy {
                    account: self.actor(),
                    community: r.community.clone(),
                    activity_id: r.activity_id.clone(),
                    announcement_event: l.event_id,
                    expires: 0,
                    used: Default::default(),
                },
                permit: self.automation_interlock.issue("reconcile-only"),
            };
            let recovered = j
                .reconcile(&g, &op.id, &adapter, now())
                .map_err(|_| "原成员邀请仍待核实")?;
            self.discovery_store()?.set_request(
                &r.event_id,
                if recovered.status == Status::Confirmed {
                    "invited"
                } else {
                    "unknown"
                },
                Some(&op.id),
            )?;
        }
        Ok(())
    }
    fn link_plan_publication(&self, op: &Operation) -> Result<()> {
        let id = op.action.payload["announcement"]["activity_id"]
            .as_str()
            .ok_or("发布活动编号缺失")?;
        let s = self.plan_store()?;
        if let Some(mut p) = s
            .list()?
            .into_iter()
            .find(|p| p.activity_id.as_deref() == Some(id))
        {
            let revision = p.revision;
            if !p.publication_operations.contains(&op.id) {
                p.publication_operations.push(op.id.clone());
                p.status = if op.action.payload["announcement"]["withdrawn"] == true {
                    buwei_host_core::planning::PlanStatus::Created
                } else {
                    buwei_host_core::planning::PlanStatus::Published
                };
                s.save(&mut p, Some(revision))?;
            }
        }
        Ok(())
    }
}

#[cfg(feature = "full-host")]
pub(super) fn community_states(rt: &Runtime, c: &Client, room: &str, actor: &str) -> Result<Vec<Value>> {
    rinx_bridge::ensure_current(c)?;
    let room: OwnedRoomId = room.parse().map_err(|_| "社群房间不合法")?;
    let response = rt
        .block_on(async {
            c.send(matrix_sdk::ruma::api::client::state::get_state_events::v3::Request::new(room))
                .with_request_config(
                    RequestConfig::new()
                        .disable_retry()
                        .timeout(Duration::from_secs(10)),
                )
                .await
        })
        .map_err(|_| "社群状态暂不可核验")?;
    let values: Vec<Value> = response
        .room_state
        .into_iter()
        .map(|r| serde_json::from_str(r.json().get()).map_err(|_| "社群状态不合法"))
        .collect::<std::result::Result<_, _>>()?;
    if !values.iter().any(|v| {
        v["type"] == "m.room.member"
            && v["state_key"] == actor
            && v["content"]["membership"] == "join"
    }) {
        return Err("本人尚未加入所选社群".into());
    }
    if values.iter().any(|v| v["type"] == "m.room.encryption") {
        return Err(
            "该社群的消息已加密，本轮请选普通活动社群；不向加密房间发送明文活动资料".into(),
        );
    }
    Ok(values)
}
#[cfg(feature = "full-host")]
fn check_publication_scope(audience: buwei_host_core::discovery::Audience, states: &[Value]) -> Result<()> {
    if audience==buwei_host_core::discovery::Audience::Public {
        buwei_host_core::public_directory::verify(states)?;
    } else if states.iter().any(|s|s["type"]=="m.room.join_rules" && s["content"]["join_rule"]=="public") {
        return Err("所选房间可公开加入，不能标成仅社群可见；请使用公开活动大厅。".into());
    }
    Ok(())
}
#[cfg(feature = "full-host")]
fn activity_member_state(
    rt: &Runtime,
    c: &Client,
    room: &str,
    actor: &str,
) -> Result<Option<Value>> {
    let room: OwnedRoomId = room.parse().map_err(|_| "活动房间不合法")?;
    let response = rt
        .block_on(async {
            c.send(matrix_sdk::ruma::api::client::state::get_state_events::v3::Request::new(room))
                .with_request_config(
                    RequestConfig::new()
                        .disable_retry()
                        .timeout(Duration::from_secs(10)),
                )
                .await
        })
        .map_err(|_| "活动成员状态待核实")?;
    Ok(response
        .room_state
        .into_iter()
        .filter_map(|r| serde_json::from_str::<Value>(r.json().get()).ok())
        .find(|v| v["type"] == "m.room.member" && v["state_key"] == actor))
}
#[cfg(feature = "full-host")]
struct CommunityMessageAdapter {
    runtime: Arc<Runtime>,
    client: Client,
    data: PathBuf,
}
#[cfg(feature = "full-host")]
impl Adapter for CommunityMessageAdapter {
    fn validate(&self, a: &Action, revision: u64) -> action_receipts::Result<()> {
        community_states(
            &self.runtime,
            &self.client,
            &a.target,
            &account(&self.client),
        )
        .map_err(|_| Error::Integrity)?;
        match a.permission.as_str() {
            "publish_community" => {
                let announcement: Announcement =
                    serde_json::from_value(a.payload["announcement"].clone())
                        .map_err(|_| Error::Integrity)?;
                let states=community_states(&self.runtime,&self.client,&a.target,&account(&self.client)).map_err(|_| Error::Integrity)?;
                check_publication_scope(announcement.audience,&states).map_err(|_| Error::Authorization)?;
                let current = buwei_host_core::catalog::Catalog::open(&self.data)
                    .and_then(|s| s.list())
                    .map_err(|_| Error::Integrity)?
                    .into_iter()
                    .find(|x| x.room == announcement.activity_room)
                    .ok_or(Error::Integrity)?;
                if current.owner != account(&self.client)
                    || current.revision != revision
                    || Announcement::from_activity(&current, announcement.withdrawn).map(|mut value| {if announcement.audience==buwei_host_core::discovery::Audience::Public {value.protocol=2;value.audience=announcement.audience;}value})
                        .map_err(|_| Error::Integrity)?
                        != announcement
                {
                    return Err(Error::Conflict);
                }
            }
            "request_entry" => {
                let l: Listing = serde_json::from_value(a.payload["listing"].clone())
                    .map_err(|_| Error::Integrity)?;
                let states=community_states(&self.runtime,&self.client,&a.target,&account(&self.client)).map_err(|_| Error::Integrity)?;
                check_publication_scope(l.announcement.audience,&states).map_err(|_| Error::Authorization)?;
                let current = DiscoveryStore::open(
                    self.data.join("community-discovery.db"),
                    &account(&self.client),
                )
                .and_then(|s| s.get(&l.key()))
                .map_err(|_| Error::Integrity)?;
                if current != l
                    || a.target != l.community
                    || !l.can_request(now())
                    || l.announcement.revision != revision
                {
                    return Err(Error::Conflict);
                }
            }
            _ => return Err(Error::Integrity),
        }
        Ok(())
    }
    fn dispatch(&self, op: &Operation) -> Dispatch {
        let result = (|| {
            self.validate(&op.action, op.revision)
                .map_err(|_| "社群操作预览失效")?;
            let room = op.action.target.parse().map_err(|_| "社群不合法")?;
            channel::publish(
                &self.runtime,
                &self.client,
                &room,
                op,
                if op.action.permission == "publish_community" {
                    "m.room.message"
                } else {
                    "org.buwei.entry.request"
                },
            )
        })();
        match result {
            Ok(e) => Dispatch::Verified(e),
            Err(e) => Dispatch::Uncertain(e),
        }
    }
    fn lookup(&self, op: &Operation) -> action_receipts::Result<Option<Evidence>> {
        let room = op.action.target.parse().map_err(|_| Error::Integrity)?;
        channel::lookup(
            &self.runtime,
            &self.client,
            &room,
            op,
            if op.action.permission == "publish_community" {
                "m.room.message"
            } else {
                "org.buwei.entry.request"
            },
        )
    }
}
#[cfg(feature = "full-host")]
struct AdmissionAdapter {
    runtime: Arc<Runtime>,
    client: Client,
    state: PathBuf,
    grant: Grant,
    policy: AccessPolicy,
    permit: automation_guard::Permit,
}
#[cfg(feature = "full-host")]
impl AdmissionAdapter {
    fn proof(&self, op: &Operation, v: &Value) -> Result<Evidence> {
        let r: EntryRequest = serde_json::from_value(op.action.payload["request"].clone())
            .map_err(|_| "原申请不合法")?;
        if v["type"] != "m.room.member"
            || v["sender"] != op.account
            || v["state_key"] != r.actor
            || v["content"]["membership"] != "invite"
            || v.get("room_id")
                .is_some_and(|s| s != op.action.target.as_str())
            || v["event_id"] == op.action.payload["previous_member_event"]
            || v["origin_server_ts"]
                .as_u64()
                .is_none_or(|t| t / 1000 < op.created_at.saturating_sub(5))
        {
            return Err("成员邀请的发送者、对象或时间不匹配".into());
        }
        Ok(Evidence {
            operation_id: op.id.clone(),
            external_id: v["event_id"].as_str().ok_or("成员邀请编号缺失")?.into(),
            account: op.account.clone(),
            target: op.action.target.clone(),
            digest: op.digest.clone(),
        })
    }
}
#[cfg(feature = "full-host")]
impl Adapter for AdmissionAdapter {
    fn validate(&self, action: &Action, revision: u64) -> action_receipts::Result<()> {
        let r: EntryRequest = serde_json::from_value(action.payload["request"].clone())
            .map_err(|_| Error::Integrity)?;
        let l: Listing = serde_json::from_value(action.payload["listing"].clone())
            .map_err(|_| Error::Integrity)?;
        let a = Store::open(&self.state)
            .and_then(|s| s.load())
            .map_err(|_| Error::Integrity)?
            .ok_or(Error::Integrity)?;
        let states = community_states(
            &self.runtime,
            &self.client,
            &r.community,
            &account(&self.client),
        )
        .map_err(|_| Error::Integrity)?;
        check_publication_scope(l.announcement.audience,&states).map_err(|_| Error::Authorization)?;
        let member = states.iter().any(|v| {
            v["type"] == "m.room.member"
                && v["state_key"] == r.actor
                && v["content"]["membership"] == "join"
        });
        self.policy
            .check(&self.grant, &r, member, now())
            .map_err(|_| Error::Authorization)?;
        if !self.permit.valid()
            || action.permission != "admit_entry"
            || action.target != a.room
            || r.activity_room != a.room
            || a.owner != account(&self.client)
            || a.revision != revision
            || a.start <= now()
            || a.metadata
                .as_ref()
                .is_none_or(|m| m.archived || m.activity_id != r.activity_id)
            || l.event_id != self.policy.announcement_event
        {
            return Err(Error::Conflict);
        }
        let raw = source_event(
            &self.runtime,
            &self.client,
            &r.community.parse().map_err(|_| Error::Integrity)?,
            r.event_id.parse().map_err(|_| Error::Integrity)?,
        )
        .map_err(|_| Error::Integrity)?;
        if EntryRequest::parse(&raw, &r.community, &l, now()).map_err(|_| Error::Integrity)? != r {
            return Err(Error::Integrity);
        }
        Ok(())
    }
    fn dispatch(&self, op: &Operation) -> Dispatch {
        let result = (|| {
            self.validate(&op.action, op.revision)
                .map_err(|_| "社群加入规则已失效")?;
            #[cfg(feature = "acceptance")]
            acceptance::before_send()?;
            let r: EntryRequest = serde_json::from_value(op.action.payload["request"].clone())
                .map_err(|_| "原申请不合法")?;
            let request=matrix_sdk::ruma::api::client::membership::invite_user::v3::Request::new(r.activity_room.parse().map_err(|_|"活动房间不合法")?,matrix_sdk::ruma::api::client::membership::invite_user::v3::InvitationRecipient::UserId(matrix_sdk::ruma::api::client::membership::invite_user::v3::InviteUserId::new(r.actor.parse().map_err(|_|"加入身份不合法")?)));
            use std::{
                future::{Future, poll_fn},
                task::Poll,
            };
            rinx_bridge::ensure_current(&self.client)?;
            let mut sending = std::pin::pin!(async {
                self.client
                    .send(request)
                    .with_request_config(
                        RequestConfig::new()
                            .disable_retry()
                            .timeout(Duration::from_secs(10)),
                    )
                    .await
            });
            let mut submitted = false;
            self.runtime.block_on(poll_fn(|cx| {
                let mut poll = || {
                    sending
                        .as_mut()
                        .poll(cx)
                        .map(|r| r.map_err(|_| "成员邀请结果待核实，保持原编号".to_owned()))
                };
                if submitted {
                    return poll();
                }
                match self.permit.submit(poll) {
                    Some(result) => {
                        submitted = true;
                        result
                    }
                    None => Poll::Ready(Err("社群规则已撤销，尚未发送邀请".to_owned())),
                }
            }))?;
            let current =
                activity_member_state(&self.runtime, &self.client, &r.activity_room, &r.actor)?
                    .ok_or("成员邀请状态缺失")?;
            #[cfg(feature = "acceptance")]
            acceptance::after_server_event("m.room.member")?;
            self.proof(op, &current)
        })();
        match result {
            Ok(e) => Dispatch::Verified(e),
            Err(e) => Dispatch::Uncertain(e),
        }
    }
    fn lookup(&self, op: &Operation) -> action_receipts::Result<Option<Evidence>> {
        let room = op.action.target.parse().map_err(|_| Error::Integrity)?;
        let values = official_sync::operation_history(
            &self.runtime,
            &self.client,
            &room,
            op,
            "m.room.member",
        )
        .map_err(|_| Error::Integrity)?;
        let matches = values
            .iter()
            .filter_map(|v| self.proof(op, v).ok())
            .collect::<Vec<_>>();
        if matches.len() > 1 {
            return Err(Error::Integrity);
        }
        Ok(matches.into_iter().next())
    }
}

#[cfg(feature = "full-host")]
fn source_event(rt: &Runtime, c: &Client, room: &OwnedRoomId, id: OwnedEventId) -> Result<Value> {
    let response = rt
        .block_on(async {
            c.send(get_room_event::v3::Request::new(room.clone(), id))
                .with_request_config(
                    RequestConfig::new()
                        .disable_retry()
                        .timeout(Duration::from_secs(10)),
                )
                .await
        })
        .map_err(|_| "申请来源暂不可核验")?;
    serde_json::from_str(response.event.json().get()).map_err(|_| "申请来源格式不合法".into())
}
