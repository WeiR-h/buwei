//! Account-bound community workflows on the official native SDK.
use super::*;
use buwei_host_core::{
    automation::Policy,
    calendar::{self, Metadata},
    catalog::Catalog,
};
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct ActivityForm {
    pub title: String,
    pub capacity: u8,
    pub start: String,
    pub end: String,
    pub template: String,
    pub location: String,
    pub description: String,
}
impl ActivityForm {
    fn activity(&self, owner: String, room: String, id: String) -> Result<Activity> {
        let start = calendar::parse(&self.start)?;
        let end = calendar::parse(&self.end)?;
        let mut a = Activity::new(owner, room, self.title.clone(), self.capacity, 19, 21)?;
        a.start = start;
        a.end = end;
        a.metadata = Some(Metadata {
            activity_id: id,
            template: self.template.clone(),
            location: self.location.clone(),
            description: self.description.clone(),
            archived: false,
            metrics: Default::default(),
        });
        a.validate()?;
        Ok(a)
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DatedDraft {
    pub earliest: Option<String>,
    pub latest: Option<String>,
    pub group: Option<u8>,
    pub questions: Vec<String>,
    pub explanation: String,
}
impl DatedDraft {
    pub fn preferences(&self) -> Result<Preferences> {
        if !self.questions.is_empty() {
            return Err("请先补充日期、时段或同行人数".into());
        }
        let p = Preferences {
            earliest: calendar::parse(self.earliest.as_deref().ok_or("请补充开始时间")?)?,
            latest: calendar::parse(self.latest.as_deref().ok_or("请补充结束时间")?)?,
            group: self.group.ok_or("请补充人数")?,
        };
        p.validate()?;
        Ok(p)
    }
}
pub(super) fn last_selected(data: &Path) -> Option<String> {
    let id = std::fs::read_to_string(data.join("selected-activity.txt")).ok()?;
    let id = id.trim();
    if id.len() != 32 || !id.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let catalog = Catalog::open(data).ok()?;
    catalog
        .list()
        .ok()?
        .iter()
        .any(|a| a.metadata.as_ref().is_some_and(|m| m.activity_id == id))
        .then(|| id.into())
}
pub(super) fn is_community_command(c: &Command) -> bool {
    matches!(
        c,
        Command::CreateDated(_)
            | Command::UseTemplate(_)
            | Command::SelectActivity(_)
            | Command::CopyActivity
            | Command::ArchiveActivity
            | Command::RecoverSetup
            | Command::LoadContacts
            | Command::PrepareShare(_)
            | Command::ConfirmShare(_)
            | Command::OpenCard { .. }
            | Command::JoinCard
            | Command::PreviewAutomation(_)
            | Command::ConfirmAutomation { .. }
            | Command::PauseAutomation
            | Command::PauseAutomationFor(_)
            | Command::CreateWithAi(_)
            | Command::ApplyActivityDraft
            | Command::Ask(_)
            | Command::SuggestDated { .. }
            | Command::ApplyDatedSuggestion { .. }
    )
}
impl Controller {
    pub(super) fn state_path(&self) -> PathBuf {
        self.selected_activity
            .as_ref()
            .map(|id| self.data.join("activities").join(id).join("activity.db"))
            .unwrap_or_else(|| self.data.join("activity.db"))
    }
    pub(super) fn select_activity(&mut self, id: String) -> Result<()> {
        let a = Catalog::open(&self.data)?
            .list()?
            .into_iter()
            .find(|a| a.metadata.as_ref().is_some_and(|m| m.activity_id == id))
            .ok_or("活动不存在，请刷新列表")?;
        if a.owner != self.actor() {
            self.g("read")?;
        }
        self.selected_activity = Some(id.clone());
        self.current = None;
        self.participant_current = None;
        self.article_current = None;
        self.share_current = None;
        self.suggestion = None;
        self.dated_suggestion = None;
        self.dated_dialogue = None;
        self.note = None;
        self.explanation = None;
        self.ai_result.clear();
        std::fs::write(self.data.join("selected-activity.txt"), id)
            .map_err(|_| "活动选择不可保存")?;
        Ok(())
    }
    fn attach_activity(&mut self, a: Activity, create: bool) -> Result<()> {
        let id = a
            .metadata
            .as_ref()
            .ok_or("请使用旧版入口恢复这场历史活动")?
            .activity_id
            .clone();
        let mut catalog = Catalog::open(&self.data)?;
        if !catalog.list()?.iter().any(|known| known.room == a.room) {
            catalog.ensure_capacity(now())?;
        }
        let path = catalog.path(&id)?;
        std::fs::create_dir_all(path.parent().unwrap()).map_err(|_| "活动目录不可写")?;
        let mut store = Store::open(&path)?;
        if create && store.load()?.is_none() {
            store.create(&self.g("create")?, &a, now())?;
        } else {
            store.cache_verified_snapshot(&a)?;
        }
        catalog.register(&a, now())?;
        self.select_activity(id)
    }
    pub(super) fn apply_community(&mut self, c: Command) -> Result<String> {
        match c {
            Command::UseTemplate(kind) => {
                let (title, capacity, description) = calendar::template(&kind);
                self.create_form = Some(ActivityForm {
                    title: title.into(),
                    capacity,
                    start: String::new(),
                    end: String::new(),
                    template: kind,
                    location: String::new(),
                    description: description.into(),
                });
                Ok("活动模板已填入，请设置日期和地点。".into())
            }
            Command::CreateDated(form) => {
                self.g("create")?;
                Catalog::open(&self.data)?.ensure_capacity(now())?;
                if self.data.join("pending-setup.json").exists() {
                    return Err("已有活动创建结果待核实，请先恢复创建结果".into());
                }
                let id = new_id();
                let candidate = form.activity(self.actor(), "!pending".into(), id.clone())?;
                if candidate.start <= now() {
                    return Err("请选择未来的活动开始时间".into());
                }
                let record = json!({"owner":self.actor(),"form":form,"id":id,"assistance_task_id":self.active_task});
                std::fs::write(
                    self.data.join("pending-setup.json"),
                    serde_json::to_vec(&record).unwrap(),
                )
                .map_err(|_| "创建记录不可保存")?;
                let mut r = create_room::v3::Request::new();
                r.name = Some(candidate.title.clone());
                r.initial_state.push(matrix_sdk::ruma::serde::Raw::from_json(serde_json::value::to_raw_value(&json!({"type":"org.buwei.setup","state_key":"","content":{"activity_id":id,"owner":self.actor()}})).map_err(|_|"活动创建信息不合法")?));
                let room = self
                    .rt
                    .block_on(async {
                        self.active()
                            .send(r)
                            .with_request_config(
                                RequestConfig::new()
                                    .disable_retry()
                                    .timeout(Duration::from_secs(10)),
                            )
                            .await
                    })
                    .map_err(|_| "活动创建结果待核实，请使用恢复创建结果")?
                    .room_id;
                let a = form.activity(self.actor(), room.to_string(), id)?;
                self.attach_activity(a, true)?;
                std::fs::remove_file(self.data.join("pending-setup.json"))
                    .map_err(|_| "活动已创建，请核实创建记录")?;
                self.create_form = None;
                #[cfg(feature = "full-host")]
                if rinx_bridge::official_mode() {
                    self.sync_activity()?;
                }
                if let (Some(task), Ok(a)) = (&self.active_task, self.activity()) {
                    let _ = self.intent_store()?.record_next_activity(task, &a, now());
                }
                Ok("活动已创建。选择联系人并预览分享，成员可以从活动卡片报名。".into())
            }
            Command::RecoverSetup => {
                self.g("create")?;
                let path = self.data.join("pending-setup.json");
                let record: Value = serde_json::from_slice(
                    &std::fs::read(&path).map_err(|_| "当前没有待核实的活动创建")?,
                )
                .map_err(|_| "创建记录损坏，请保留原文件")?;
                if record["owner"] != self.actor() {
                    return Err("待核实创建属于其他账号".into());
                }
                let form: ActivityForm =
                    serde_json::from_value(record["form"].clone()).map_err(|_| "创建记录不合法")?;
                let id = record["id"].as_str().ok_or("创建编号缺失")?;
                let response=self.rt.block_on(async{self.active().send(matrix_sdk::ruma::api::client::membership::joined_rooms::v3::Request::new()).await}).map_err(|_|"已加入房间暂不可核实")?;
                let mut found = vec![];
                for room in response.joined_rooms {
                    let states=self.rt.block_on(async{self.active().send(matrix_sdk::ruma::api::client::state::get_state_events::v3::Request::new(room.clone())).await}).map_err(|_|"创建结果尚未完整核实")?;
                    if states.room_state.iter().any(|raw| {
                        serde_json::from_str::<Value>(raw.json().get()).is_ok_and(|v| {
                            v["type"] == "org.buwei.setup"
                                && v["sender"] == self.actor()
                                && v["content"]["activity_id"] == id
                        })
                    }) {
                        found.push(room);
                    }
                }
                if found.len() != 1 {
                    return Err("尚不能唯一核实原创建结果，创建记录已保留".into());
                }
                self.attach_activity(
                    form.activity(self.actor(), found[0].to_string(), id.into())?,
                    true,
                )?;
                std::fs::remove_file(path).map_err(|_| "创建恢复后记录不可清理")?;
                #[cfg(feature = "full-host")]
                if rinx_bridge::official_mode() {
                    self.sync_activity()?;
                }
                if let (Some(task), Ok(a)) =
                    (record["assistance_task_id"].as_str(), self.activity())
                {
                    let _ = self.intent_store()?.record_next_activity(task, &a, now());
                }
                Ok("已恢复原活动房间，没有重复创建。".into())
            }
            Command::SelectActivity(index) => {
                let list = Catalog::open(&self.data)?.list()?;
                let id = list
                    .get(index)
                    .and_then(|a| a.metadata.as_ref())
                    .ok_or("请选择列表中的活动")?
                    .activity_id
                    .clone();
                self.select_activity(id)?;
                Ok("已打开活动。".into())
            }
            Command::CopyActivity => {
                self.g("create")?;
                let a = self.activity()?;
                let m = a.metadata.as_ref().ok_or("历史活动请填写新日期")?;
                self.create_form = Some(ActivityForm {
                    title: a.title,
                    capacity: a.capacity,
                    start: String::new(),
                    end: String::new(),
                    template: m.template.clone(),
                    location: m.location.clone(),
                    description: m.description.clone(),
                });
                Ok("已复制活动设置，请确认新日期。原成员需重新报名。".into())
            }
            Command::ArchiveActivity => {
                self.g("create")?;
                let a = self.activity()?;
                if a.owner != self.actor() || a.held() > 0 {
                    return Err("请先处理所有待回复或待核实邀请，再归档活动")?;
                }
                Store::open(self.state_path())?.update(a.revision, |a| {
                    a.metadata.as_mut().ok_or("历史活动请从旧版查看")?.archived = true;
                    a.paused = true;
                    a.revision += 1;
                    Ok(())
                })?;
                if let Some(id) = &self.selected_activity {
                    self.policies.remove(id);
                }
                #[cfg(feature = "full-host")]
                if rinx_bridge::official_mode() {
                    self.sync_activity()?;
                }
                Ok("活动已归档，原记录和小记已保留。".into())
            }
            Command::PreviewAutomation(settings) => {
                self.g("invite")?;
                settings.validate()?;
                let a = self.activity()?;
                let id = a
                    .metadata
                    .as_ref()
                    .ok_or("请先创建带日期的新活动")?
                    .activity_id
                    .clone();
                if a.owner != self.actor() {
                    return Err("只有组织者可以启用自动补位".into());
                }
                self.policies.remove(&id);
                let permit = self.automation_interlock.issue(&id);
                self.policy_preview = Some((new_id(), id, settings, now() + 120, permit));
                Ok("请核对活动、发送时段、邀请期限与次数上限，再确认启用。".into())
            }
            Command::ConfirmAutomation { id, settings } => {
                let (nonce, activity_id, preview, until, permit) =
                    self.policy_preview.as_ref().ok_or("请先查看自动补位规则")?;
                if !permit.valid()
                    || id != *nonce
                    || settings != *preview
                    || now() >= *until
                    || self.selected_activity.as_ref() != Some(activity_id)
                {
                    return Err("活动、规则或确认期限已变化，请重新查看规则")?;
                }
                let a = self.activity()?;
                let policy = Policy::issue(
                    &self.g("invite")?,
                    &a,
                    settings,
                    now(),
                    self.authorized_until,
                )?;
                self.policies.insert(activity_id.clone(), policy);
                self.policy_permits.insert(
                    activity_id.clone(),
                    self.automation_interlock.issue(activity_id),
                );
                self.policy_preview = None;
                Ok("自动补位已启用，系统会按确认的规则邀请候补并核实回复。".into())
            }
            Command::PauseAutomation | Command::PauseAutomationFor(_) => {
                if let Some(id) = &self.selected_activity {
                    self.policies.remove(id);
                }
                self.policy_preview = None;
                Ok("自动补位已暂停。".into())
            }
            Command::LoadContacts => {
                self.load_contacts()?;
                Ok("已读取 Rinx 联系人。请选择接收者并预览分享。".into())
            }
            Command::PrepareShare(recipient) => self.prepare_share(&recipient),
            Command::ConfirmShare(recipient) => self.confirm_share(&recipient),
            Command::OpenCard { room, activity_id } => {
                OwnedRoomId::try_from(room.as_str()).map_err(|_| "活动卡片房间不合法")?;
                if activity_id.len() != 32 || !activity_id.bytes().all(|b| b.is_ascii_hexdigit()) {
                    return Err("活动卡片编号不合法".into());
                }
                self.incoming_card = Some((room, activity_id));
                Ok("活动卡片已打开。核对本人账号，确认加入后读取组织者的活动资料。".into())
            }
            Command::JoinCard => {
                self.g("participate")?;
                let (room, id) = self.incoming_card.clone().ok_or("请先打开活动卡片")?;
                let room = OwnedRoomId::try_from(room.as_str()).map_err(|_| "活动房间不合法")?;
                self.rt
                    .block_on(self.active().join_room_by_id(&room))
                    .map_err(|_| "请先在 Rinx 查看并接受活动房间邀请")?;
                #[cfg(feature = "full-host")]
                let a = official_sync::fetch(&self.rt, self.active(), &room)?;
                #[cfg(not(feature = "full-host"))]
                return Err("活动卡片需要完整官方宿主".into());
                #[cfg(feature = "full-host")]
                {
                    if a.metadata.as_ref().is_none_or(|m| m.activity_id != id) {
                        return Err("活动卡片与组织者当前活动不一致".into());
                    }
                    self.attach_activity(a, false)?;
                    self.incoming_card = None;
                    Ok("已核验活动来源，可以填写本人报名。".into())
                }
            }
            Command::CreateWithAi(requirement) => {
                let g = self.g("model")?;
                let (form, questions) =
                    super::super::model::create_activity(&self.model, &g, &requirement)?;
                self.create_form = form;
                self.ai_result = questions;
                Ok("活动草稿已生成，请核对并填写缺失信息后创建。".into())
            }
            Command::ApplyActivityDraft => {
                let form = self.create_form.clone().ok_or("请先生成活动草稿")?;
                self.apply_community(Command::CreateDated(form))
            }
            Command::SuggestDated {
                requirement,
                answer,
            } => {
                let g = self.g("model")?;
                let a = self.activity()?;
                let mut turns = self
                    .dated_dialogue
                    .as_ref()
                    .filter(|(base, _)| base == &requirement)
                    .map(|(_, turns)| turns.clone())
                    .unwrap_or_default();
                if !answer.trim().is_empty() && turns.last() != Some(&answer) {
                    turns.push(answer.clone());
                }
                if turns.len() > 3
                    || requirement.len() + turns.iter().map(String::len).sum::<usize>() > 4000
                {
                    return Err("最多补充三轮，请将完整信息重新填入需求".into());
                }
                let supplement = turns.join("\n后续更正：");
                let draft = super::super::model::recommend_dated(
                    &self.model,
                    &g,
                    &a,
                    &requirement,
                    &supplement,
                )?;
                self.dated_dialogue = Some((requirement.clone(), turns));
                self.ai_result = format!("{}\n{}", draft.explanation, draft.questions.join("\n"));
                self.dated_suggestion = Some((
                    self.actor(),
                    dialogue_binding(&requirement, &answer),
                    a.revision,
                    draft,
                ));
                Ok("报名建议已生成，请核对日期、时间与人数。".into())
            }
            Command::ApplyDatedSuggestion {
                requirement,
                answer,
            } => {
                let a = self.activity()?;
                let (actor, binding, revision, draft) =
                    self.dated_suggestion.as_ref().ok_or("请先生成报名建议")?;
                if *actor != self.actor()
                    || *binding != dialogue_binding(&requirement, &answer)
                    || self.dated_dialogue.as_ref().is_none_or(|(base, turns)| {
                        base != &requirement
                            || (!answer.is_empty() && turns.last() != Some(&answer))
                    })
                    || *revision != a.revision
                {
                    return Err("需求、账号或活动已变化，请重新生成建议".into());
                }
                let p = draft.preferences()?;
                let result = self.apply(Command::Join(p));
                if result.is_ok() {
                    self.dated_suggestion = None;
                }
                result
            }
            Command::Ask(question) => {
                let g = self.g("model")?;
                let a = self.activity()?;
                self.ai_result = super::super::model::ask_activity(&self.model, &g, &a, &question)?;
                Ok("已根据本场已核验资料回答。".into())
            }
            _ => Err("社区操作入口不匹配".into()),
        }
    }
    pub(super) fn community_view(&self, v: &mut View) {
        let selected = self.activity().ok();
        let clock = now();
        let visible = |op: &Operation| {
            selected
                .as_ref()
                .is_some_and(|a| a.room == op.action.target)
                && self.grant.as_ref().is_some_and(|g| {
                    Journal::preview_current(g, op, clock)
                        || (matches!(op.status, Status::Unknown | Status::Dispatching)
                            && op.account == self.actor()
                            && g.check(&op.action.permission, clock).is_ok())
                })
        };
        v.confirmation_ready = self.current.as_ref().is_some_and(visible)
            || self.participant_current.as_ref().is_some_and(visible);
        v.activity_list = Catalog::open(&self.data)
            .and_then(|c| c.list())
            .unwrap_or_default()
            .iter()
            .map(|a| {
                format!(
                    "{} · {}\n{} · 已确认 {}/{}",
                    a.title,
                    if a.metadata.as_ref().is_some_and(|m| m.archived) || now() >= a.end {
                        "已结束"
                    } else if now() >= a.start {
                        "进行中"
                    } else {
                        "待举办"
                    },
                    calendar::display(a.start),
                    a.confirmed(),
                    a.capacity
                )
            })
            .collect();
        v.create_form = self.create_form.clone();
        v.ai_result = self.ai_result.clone();
        v.contacts = self.contacts.iter().map(|(id, _)| id.clone()).collect();
        v.card_status = if self.incoming_card.is_some() {
            "已收到活动卡片，请确认加入并报名。".into()
        } else {
            String::new()
        };
        v.share_preview = self
            .share_current
            .as_ref()
            .map(|op| {
                format!(
                    "发送账号：{}\n接收者：{}\n内容：{}\n状态：{:?}",
                    op.account,
                    op.action.payload["recipient"].as_str().unwrap_or_default(),
                    op.action.payload["body"].as_str().unwrap_or_default(),
                    op.status
                )
            })
            .unwrap_or_default();
        v.policy_consent_id = self
            .policy_preview
            .as_ref()
            .filter(|p| p.4.valid())
            .map(|p| p.0.clone());
        v.policy_preview=self.policy_preview.as_ref().filter(|p|p.4.valid()).map(|(_,id,s,_,_)|format!("账号：{}\n活动：{}\n按报名顺序选择时间覆盖且剩余名额足够的同行报名；整组保留。\n邀请 {} 分钟；发送时段 {:02}:00–{:02}:00；本次最多 {} 次。\n只邀请本场主动报名成员，截止不超过活动开始；授权最长一小时。",self.actor(),self.activity().map(|a|a.title).unwrap_or_else(|_|id.clone()),s.invitation_minutes,s.quiet_start,s.quiet_end,s.max_invitations)).unwrap_or_default();
        v.automation_status = self
            .selected_activity
            .as_ref()
            .and_then(|id| self.policies.get(id))
            .map(|p| {
                let running = self
                    .grant
                    .as_ref()
                    .zip(self.activity().ok().as_ref())
                    .is_some_and(|(g, a)| p.check(g, a, now()).is_ok())
                    && self.policy_permits.get(&p.activity_id).is_some_and(|p|p.valid());
                format!(
                    "自动补位{} · 邀请 {} 分钟 · 北京时间 {:02}:00–{:02}:00 · 本次上限 {} · 到期 {}{}",
                    if running { "运行中" } else { "已暂停" },
                    p.settings.invitation_minutes,
                    p.settings.quiet_start,p.settings.quiet_end,p.settings.max_invitations,
                    calendar::display(p.expires_at),
                    if p.expires_at.saturating_sub(now()) <= 300 {
                        " · 请续期授权并重新启用规则"
                    } else {
                        ""
                    }
                )
            })
            .unwrap_or_else(|| "自动补位未启用，可先查看规则。".into());
        if let Ok(a) = self.activity() {
            v.activity_identity = a
                .metadata
                .as_ref()
                .map(|m| m.activity_id.clone())
                .unwrap_or_else(|| a.room.clone());
            v.dated = a.metadata.is_some();
            if let Some(m) = &a.metadata {
                v.activity = format!(
                    "{}\n{} 至 {} · 北京时间\n地点：{}\n{}\n已确认 {} · 为候补保留 {} · 剩余 {} / {}",
                    a.title,
                    calendar::display(a.start),
                    calendar::display(a.end),
                    m.location,
                    m.description,
                    a.confirmed(),
                    a.held(),
                    a.free(),
                    a.capacity
                );
                if let Some(status) = self.sync_results.get(&m.activity_id) {
                    v.activity.push_str(&format!("\n本场同步：{status}"));
                }
            }
            v.people = a
                .ordered_people()
                .iter()
                .enumerate()
                .map(|(n, p)| {
                    format!(
                        "第 {} 位 · {} · {} 人 · {}\n{} 至 {}",
                        n + 1,
                        p.name,
                        p.preferences.group,
                        if a.invitations.iter().any(|i| i.recipient == p.account
                            && i.reply == buwei_host_core::Reply::Pending
                            && i.delivery != buwei_host_core::Delivery::Rejected)
                        {
                            "名额为你保留"
                        } else {
                            match p.status {
                                buwei_host_core::PersonStatus::Waiting => "候补中",
                                buwei_host_core::PersonStatus::Confirmed => "报名已确认",
                                buwei_host_core::PersonStatus::Declined => "已拒绝",
                                buwei_host_core::PersonStatus::Cancelled => "已取消",
                                buwei_host_core::PersonStatus::Expired => "邀请已过期",
                            }
                        },
                        calendar::display(p.preferences.earliest),
                        calendar::display(p.preferences.latest)
                    )
                })
                .collect::<Vec<_>>()
                .join("\n\n");
            fn preview(op: &Operation, title: &str) -> String {
                format!(
                    "账号：{}\n活动：{}\n操作：{}\n{}",
                    op.account,
                    title,
                    op.action.summary,
                    match op.status {
                        Status::Prepared => "请确认以上内容；修改后需重新预览",
                        Status::Unknown | Status::Dispatching =>
                            "发送结果待核实，名额与原编号已保留",
                        Status::Confirmed => "服务端回执已核验",
                        _ => "请查看活动记录",
                    }
                )
            }
            if let Some(op) = &self.current {
                v.preview = preview(op, &a.title);
            }
            if let Some(op) = &self.participant_current {
                v.participant_preview = preview(op, &a.title);
            }
            if let Some(m) = &a.metadata {
                if m.archived || now() >= a.end {
                    v.activity.push_str(&format!("\n活动复盘：确认 {} 人 · 取消 {} 人 · 邀请成功 {} 次 · 平均邀请回复耗时 {}",a.confirmed(),m.metrics.cancelled_people,m.metrics.successful_invitations,if m.metrics.timed_responses>0{format!("{} 秒",m.metrics.response_seconds/m.metrics.timed_responses as u64)}else{"暂无已核验时间记录".into()}));
                }
            }
            if let Some((_, p)) = &v.participant_inputs {
                v.dated_inputs = Some((
                    calendar::display(p.earliest),
                    calendar::display(p.latest),
                    p.group,
                ));
            }
        }
    }
    pub(crate) fn sync_interval(&self) -> Duration {
        let count = Catalog::open(&self.data)
            .and_then(|c| c.list())
            .map(|v| {
                v.iter()
                    .filter(|a| a.metadata.as_ref().is_some_and(|m| !m.archived) && a.end > now())
                    .count()
            })
            .unwrap_or(1)
            .clamp(1, 5);
        Duration::from_millis(10000 / count as u64)
    }
    #[cfg(feature = "full-host")]
    pub(super) fn community_tick(&mut self) -> Option<View> {
        let list: Vec<_> = match Catalog::open(&self.data).and_then(|c| c.list()) {
            Ok(list) => list
                .into_iter()
                .filter(|a| a.metadata.as_ref().is_some_and(|m| !m.archived) && a.end > now())
                .collect(),
            Err(error) => {
                self.last_sync_status = format!("活动资料需恢复：{error}");
                return Some(self.view());
            }
        };
        if list.is_empty() {
            if self.activity().is_err() {
                return None;
            }
            let v = self.handle(Command::SyncActivity);
            self.last_sync_status = v.message;
            return Some(self.view());
        }
        let a = &list[self.sync_rotation % list.len()];
        self.sync_rotation += 1;
        let saved = self.selected_activity.clone();
        let current = self.current.take();
        let participant = self.participant_current.take();
        let article = self.article_current.take();
        self.selected_activity = a.metadata.as_ref().map(|m| m.activity_id.clone());
        let started = std::time::Instant::now();
        let synced = self.sync_activity();
        let succeeded = synced.is_ok();
        let result = synced.and_then(|_| self.automatic_invite());
        let message = result.unwrap_or_else(|e| e);
        let id = a.metadata.as_ref().unwrap().activity_id.clone();
        self.sync_results.insert(id.clone(), message.clone());
        let diagnostic = self.data.join("sync-status");
        if std::fs::create_dir_all(&diagnostic).is_ok() {
            let _ = std::fs::write(diagnostic.join(format!("{id}.json")), serde_json::to_vec(&serde_json::json!({"collected_at_unix":now(),"elapsed_ms":started.elapsed().as_millis(),"success":succeeded,"message":message})).unwrap_or_default());
        }
        self.last_sync_status = format!("最近同步 {}：{}", a.title, message);
        self.selected_activity = saved;
        self.current = current;
        self.participant_current = participant;
        self.article_current = article;
        let _ = self.refresh_assistance();
        Some(self.view())
    }
    fn automatic_invite(&mut self) -> Result<String> {
        let Some(id) = self.selected_activity.clone() else {
            return Ok("已同步".into());
        };
        let Some(policy) = self.policies.get(&id).cloned() else {
            return Ok("已同步".into());
        };
        let permit = self
            .policy_permits
            .get(&id)
            .cloned()
            .ok_or("自动规则需重新确认")?;
        if !permit.valid() {
            return Err("规则已修改，自动补位已暂停".into());
        }
        let g = self.g("invite")?;
        let a = self.activity()?;
        policy.check(&g, &a, now())?;
        for op in self
            .journal
            .pending(&g, now())
            .map_err(|_| "待核实操作不可读取")?
            .into_iter()
            .filter(|o| o.action.target == a.room && o.action.permission == "invite")
        {
            let adapter = self.adapter()?;
            let restored = self
                .journal
                .reconcile(&g, &op.id, &adapter, now())
                .map_err(|_| "原邀请尚待核实，自动补位暂停")?;
            if restored.status != Status::Confirmed {
                return Err("原邀请尚待核实，自动补位暂停".into());
            }
        }
        for _ in 0..3 {
            if !permit.valid() {
                return Err("规则已修改，自动补位已暂停".into());
            }
            let a = self.activity()?;
            if a.candidate().is_none() {
                break;
            }
            let action = policy.action(&g, &a, now())?;
            let op = self
                .journal
                .prepare(&g, action, a.revision, now(), 120)
                .map_err(|_| "自动操作不可保存")?;
            Store::open(self.state_path())?.audit_policy(
                &policy.policy_id,
                &op.id,
                &policy.digest(),
                policy.settings.max_invitations,
            )?;
            let mut adapter = self.adapter()?;
            adapter.automation_permit = Some(permit.clone());
            policy.check(&g, &self.activity()?, now())?;
            self.journal
                .confirm(&g, &op.id, &adapter, now())
                .map_err(|_| "规则确认已失效")?;
            let done = self
                .journal
                .execute(&g, &op.id, &adapter, now())
                .map_err(|_| "自动邀请暂不可执行")?;
            if done.status != Status::Confirmed {
                return Err("邀请结果待核实，原编号和名额已保留".into());
            }
        }
        #[cfg(feature = "full-host")]
        if rinx_bridge::official_mode() {
            let sg = self.sync_grant.as_ref().ok_or("同步授权已失效")?.clone();
            let client = self.active().clone();
            official_sync::publish(
                self.rt.clone(),
                &client,
                self.state_path(),
                &sg,
                &mut self.journal,
            )?;
        }
        Ok("回复已核验，自动补位已检查。".into())
    }
    pub(super) fn load_contacts(&mut self) -> Result<()> {
        self.g("read")?;
        let mut contacts = vec![];
        for room in self.active().rooms() {
            for user in room.direct_targets() {
                if user.as_str() != self.actor() {
                    contacts.push((user.to_string(), room.room_id().to_string()));
                }
            }
        }
        contacts.sort();
        contacts.dedup_by(|a, b| a.0 == b.0);
        self.contacts = contacts;
        Ok(())
    }
    fn prepare_share(&mut self, recipient: &str) -> Result<String> {
        let g = self.g("share_card")?;
        self.load_contacts()?;
        let room = self
            .contacts
            .iter()
            .find(|(id, _)| id == recipient)
            .ok_or("请从已核验的 Rinx 联系人中选择接收者")?
            .1
            .clone();
        let a = self.activity()?;
        if a.owner != self.actor() {
            return Err("分享活动需由组织者确认".into());
        }
        let card = super::super::activity_card::Card::from_activity(&a)?;
        let body = card.body();
        let action = Action {
            permission: "share_card".into(),
            target: room,
            summary: format!("向 {recipient} 分享活动，并邀请加入活动房间"),
            payload: json!({"recipient":recipient,"body":body,"card":card}),
        };
        self.share_current = Some(
            self.journal
                .prepare(&g, action, a.revision, now(), 120)
                .map_err(|_| "分享预览不可保存")?,
        );
        Ok("活动卡片已预览，核对接收者和内容后确认分享。".into())
    }
    fn confirm_share(&mut self, recipient: &str) -> Result<String> {
        let g = self.g("share_card")?;
        let op = self
            .share_current
            .as_ref()
            .ok_or("请先预览活动分享")?
            .clone();
        if op.action.payload["recipient"] != recipient {
            return Err("接收者已变化，请重新预览")?;
        }
        let adapter = super::super::activity_card::CardAdapter {
            runtime: self.rt.clone(),
            client: self.active().clone(),
            state: self.state_path(),
        };
        if op.status == Status::Prepared {
            self.journal
                .confirm(&g, &op.id, &adapter, now())
                .map_err(|_| "分享确认已失效")?;
        }
        self.share_current = Some(
            self.journal
                .execute(&g, &op.id, &adapter, now())
                .map_err(|_| "分享结果需要沿原编号恢复")?,
        );
        Ok("分享执行记录已保存，请查看核验结果。".into())
    }
}
