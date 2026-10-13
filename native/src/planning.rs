//! One workflow shared by the module's tools and its native form.
use super::*;
use buwei_host_core::planning::{
    ActivityBasics, ActivityPlanDraft, PlanProposal, PlanStatus, PlanStore,
};
#[derive(Clone, serde::Deserialize)]
#[serde(tag = "command", content = "value", deny_unknown_fields)]
pub(crate) enum PlanningCommand {
    New,
    Prepare {
        text: String,
        id: Option<String>,
        revision: Option<u64>,
    },
    Proposal {
        text: String,
        id: Option<String>,
        revision: Option<u64>,
        proposal: PlanProposal,
        call_id: Option<String>,
    },
    Select(String),
    Save {
        id: Option<String>,
        revision: Option<u64>,
        basics: ActivityBasics,
    },
    SaveAndPreview {
        id: Option<String>,
        revision: Option<u64>,
        basics: ActivityBasics,
    },
    PreviewCreate,
    ConfirmCreate {
        nonce: String,
        basics: ActivityBasics,
    },
}
pub(super) struct CreationPreview {
    pub id: String,
    pub draft: String,
    pub revision: u64,
    pub digest: String,
    pub account: String,
    pub expires: u64,
}
impl Controller {
    pub(crate) fn planning_tool(
        &mut self,
        expected_actor: &str,
        request: super::super::planning_service::Request,
    ) -> Result<ActivityPlanDraft> {
        if expected_actor != self.actor() || !self.host_session_current() {
            return Err("账号或应用实例已变化，请重新开始".into());
        }
        self.g("read")?;
        match request {
            super::super::planning_service::Request::Proposal(command) => {
                let view = self.handle(Command::Planning(command));
                if !view.success {
                    return Err(view.message);
                }
                view.plan.ok_or("策划结果未保存".into())
            }
            super::super::planning_service::Request::Read(id) => match id {
                Some(id) => self.plan_store()?.load(&id),
                None => self.current_plan(),
            },
        }
    }
    pub(super) fn plan_store(&self) -> Result<PlanStore> {
        PlanStore::open(self.data.join("activity-plans.db"), &self.actor())
    }
    pub(super) fn current_plan(&self) -> Result<ActivityPlanDraft> {
        let s = self.plan_store()?;
        if let Some(id) = &self.selected_plan {
            s.load(id)
        } else {
            Err("请先准备活动策划稿".into())
        }
    }
    pub(super) fn apply_planning(&mut self, command: PlanningCommand) -> Result<String> {
        self.g("create")?;
        match command {
            PlanningCommand::SaveAndPreview { id, revision, basics } => {
                self.apply_planning(PlanningCommand::Save { id, revision, basics })?;
                self.apply_planning(PlanningCommand::PreviewCreate)
            }
            PlanningCommand::New => {
                self.selected_plan = None;
                self.create_form = Some(
                    ActivityBasics {
                        template: "custom".into(),
                        ..Default::default()
                    }
                    .into(),
                );
                self.plan_creation_preview = None;
                Ok("填写活动基础信息，或描述需求让 AI 准备策划稿。".into())
            }
            PlanningCommand::Prepare { text, id, revision } => {
                let g = self.g("model")?;
                let permit = self.automation_interlock.issue("_planning_model");
                let old = id
                    .as_ref()
                    .map(|id| self.plan_store()?.load(id))
                    .transpose()?;
                if old.as_ref().is_some_and(|p| Some(p.revision) != revision) {
                    return Err("策划已变化，请重新打开".into());
                }
                let proposal = super::super::community_model::prepare_plan(
                    &self.model,
                    &g,
                    &text,
                    old.as_ref(),
                )?;
                if !permit.valid() || !self.host_session_current() {
                    return Err("授权、账号或应用实例已变化，旧模型结果未保存".into());
                }
                self.apply_planning(PlanningCommand::Proposal {
                    text,
                    id,
                    revision,
                    proposal,
                    call_id: None,
                })
            }
            PlanningCommand::Proposal {
                text,
                id,
                revision,
                proposal,
                call_id,
            } => {
                let store = self.plan_store()?;
                if let Some(call) = &call_id {
                    if let Some(d) = store.completed_call(call)? {
                        self.selected_plan = Some(d.id);
                        return Ok("原策划工具结果已恢复。".into());
                    }
                }
                let mut d = match &id {
                    Some(id) => store.load(id)?,
                    None => ActivityPlanDraft::new(&self.actor(), &text, now())?,
                };
                if id.is_some() && revision != Some(d.revision) {
                    return Err("策划已变化，请重新打开".into());
                }
                d.merge(&text, proposal, now())?;
                store.save(&mut d, revision)?;
                if let Some(call) = call_id {
                    store.record_call(&call, &d.id)?;
                }
                self.create_form = Some(d.basics.clone().into());
                self.selected_plan = Some(d.id.clone());
                self.plan_creation_preview = None;
                std::fs::write(self.data.join("selected-plan.txt"), &d.id)
                    .map_err(|_| "策划已保存，选择记录不可保存")?;
                self.ai_result = d.basics.questions().join("\n");
                Ok("策划稿已保存；请核对基础信息，流程和提醒仍是建议。".into())
            }
            PlanningCommand::Select(id) => {
                let d = self.plan_store()?.load(&id)?;
                self.selected_plan = Some(id.clone());
                self.create_form = Some(d.basics.into());
                self.plan_creation_preview = None;
                std::fs::write(self.data.join("selected-plan.txt"), id)
                    .map_err(|_| "选择记录不可保存")?;
                Ok("已打开原策划稿。".into())
            }
            PlanningCommand::Save {
                id,
                revision,
                basics,
            } => {
                let store = self.plan_store()?;
                let mut d = match &id {
                    Some(id) => store.load(id)?,
                    None => ActivityPlanDraft::new(&self.actor(), "本人填写的活动安排", now())?,
                };
                if id.is_some() && Some(d.revision) != revision {
                    return Err("策划已变化，请重新打开".into());
                }
                d.confirm_basics(basics, now())?;
                store.save(&mut d, revision)?;
                self.create_form = Some(d.basics.into());
                self.selected_plan = Some(d.id.clone());
                self.plan_creation_preview = None;
                std::fs::write(self.data.join("selected-plan.txt"), d.id)
                    .map_err(|_| "选择记录不可保存")?;
                Ok("活动基础信息已由本人核对，尚未创建房间。".into())
            }
            PlanningCommand::PreviewCreate => {
                let d = self.current_plan()?;
                let digest = d.creation_digest(now())?;
                self.plan_creation_preview = Some(CreationPreview {
                    id: new_id(),
                    draft: d.id,
                    revision: d.revision,
                    digest,
                    account: self.actor(),
                    expires: now() + 120,
                });
                Ok("核对当前活动基础信息后确认创建。分享需要另行确认。".into())
            }
            PlanningCommand::ConfirmCreate { nonce, basics } => {
                let p = self
                    .plan_creation_preview
                    .as_ref()
                    .ok_or("请先预览活动创建")?;
                let d = self.plan_store()?.load(&p.draft)?;
                if p.id != nonce
                    || p.account != self.actor()
                    || p.expires <= now()
                    || p.revision != d.revision
                    || p.digest != d.creation_digest(now())?
                    || basics != d.basics
                {
                    return Err("创建预览已失效或表单已编辑，请重新保存并核对".into());
                }
                self.creating_plan = Some(d.id);
                let result = self.apply_community(Command::CreateDated(d.basics.into()));
                self.creating_plan = None;
                self.plan_creation_preview = None;
                result
            }
        }
    }
    pub(super) fn link_plan_setup(&self, id: &str, setup: &str) -> Result<()> {
        let store = self.plan_store()?;
        let mut p = store.load(id)?;
        let revision = p.revision;
        if p.status != PlanStatus::Draft || p.setup_id.is_some() {
            return Err("策划已有创建结果待核实".into());
        }
        p.status = PlanStatus::Creating;
        p.setup_id = Some(setup.into());
        store.save(&mut p, Some(revision))
    }
    pub(super) fn finish_plan_setup(&self, id: &str, activity: &str) -> Result<()> {
        let store = self.plan_store()?;
        let mut p = store.load(id)?;
        let revision = p.revision;
        if p.setup_id.as_deref() != Some(activity)
            || p.activity_id.as_ref().is_some_and(|a| a != activity)
        {
            return Err("策划与原创建编号不一致".into());
        }
        p.status = PlanStatus::Created;
        p.activity_id = Some(activity.into());
        store.save(&mut p, Some(revision))
    }
    pub(super) fn planning_view(&self, v: &mut View) {
        v.plan_choices = self
            .plan_store()
            .and_then(|s| s.list())
            .unwrap_or_default()
            .iter()
            .map(|p| {
                (
                    p.id.clone(),
                    format!("{} · {}", p.basics.title, match p.status {
                        buwei_host_core::planning::PlanStatus::Draft => "草稿",
                        buwei_host_core::planning::PlanStatus::Creating => "创建待核实",
                        buwei_host_core::planning::PlanStatus::Created => "已创建",
                        buwei_host_core::planning::PlanStatus::Published => "已发布",
                    }),
                )
            })
            .collect();
        v.plan = self.current_plan().ok();
        v.plan_creation_nonce = self
            .plan_creation_preview
            .as_ref()
            .filter(|p| p.account == self.actor() && now() < p.expires)
            .map(|p| p.id.clone());
        v.plan_creation_preview = self
            .plan_creation_preview
            .as_ref()
            .map(|p| {
                format!(
                    "创建账号：{}\n{}\n预览有效期 2 分钟；此步创建活动房间。",
                    p.account,
                    self.current_plan().map(|p| p.text()).unwrap_or_default()
                )
            })
            .unwrap_or_default();
    }
}
