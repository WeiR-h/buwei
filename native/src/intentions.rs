//! Intention UI adapter. Account identity always comes from the native host.
use super::*;
use buwei_host_core::assistance_tasks::TaskStatus;
use buwei_host_core::intent_feedback::FeedbackScope;
use buwei_host_core::proactive::*;
use buwei_host_core::{assistance::*, calendar, catalog::Catalog};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct GoalForm {
    pub kind: GoalKind,
    pub activity_id: Option<String>,
    pub title: String,
    pub template: String,
    pub earliest: String,
    pub latest: String,
    pub group: String,
    pub target: String,
    pub check_at: String,
    #[serde(default)]
    pub recurrence_days: String,
    #[serde(default)]
    pub preparation_hours: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "command", content = "value", deny_unknown_fields)]
pub(crate) enum IntentCommand {
    PrepareGoal(GoalKind),
    PrepareGoalWithAi {
        text: String,
        kind: GoalKind,
    },
    SelectGoal(String),
    SaveGoal {
        id: Option<String>,
        form: GoalForm,
    },
    SetGoalStatus {
        id: String,
        status: GoalStatus,
    },
    DeleteGoal(String),
    SavePreferences(PersonalPreferences),
    ResetPreferences,
    UseCard {
        id: String,
        fingerprint: String,
    },
    ControlCard {
        id: String,
        snooze: bool,
        disable: bool,
    },
    PauseTask(String),
    ResumeTask(String),
    Feedback {
        goal_id: String,
        template: String,
        group: u8,
        scope: FeedbackScope,
    },
    UndoFeedback(String),
    PreviewAnalysis,
    ConfirmAnalysis(String),
    RevokeAnalysis,
}
pub(crate) fn parse_template(value: &str) -> Result<String> {
    match value.trim() {
        "羽毛球" | "badminton" => Ok("badminton".into()),
        "桌游" | "boardgame" => Ok("boardgame".into()),
        "读书会" | "reading" => Ok("reading".into()),
        "自定义" | "custom" => Ok("custom".into()),
        "不限" | "不限类型" | "any" => Ok("any".into()),
        _ => Err("请选择羽毛球、桌游、读书会、自定义或不限类型".into()),
    }
}
impl GoalForm {
    fn from_goal(g: &ActivityGoal) -> Self {
        Self {
            kind: g.input.kind,
            activity_id: g.input.activity_id.clone(),
            title: g.input.title.clone(),
            template: g.input.template.clone(),
            earliest: calendar::display(g.input.earliest),
            latest: calendar::display(g.input.latest),
            group: g.input.group.to_string(),
            target: g.input.target.map(|n| n.to_string()).unwrap_or_default(),
            check_at: g.input.check_at.map(calendar::display).unwrap_or_default(),
            recurrence_days: g
                .input
                .recurrence_days
                .map(|n| n.to_string())
                .unwrap_or_default(),
            preparation_hours: g.input.preparation_hours.to_string(),
        }
    }
    fn input(&self) -> Result<GoalInput> {
        Ok(GoalInput {
            title: self.title.trim().into(),
            kind: self.kind,
            activity_id: self.activity_id.clone(),
            template: parse_template(&self.template)?,
            earliest: calendar::parse(&self.earliest)?,
            latest: calendar::parse(&self.latest)?,
            group: self
                .group
                .trim()
                .parse()
                .map_err(|_| "请填写包括本人在内的同行人数")?,
            target: if self.target.trim().is_empty() {
                None
            } else {
                Some(
                    self.target
                        .trim()
                        .parse()
                        .map_err(|_| "请填写目标确认人数")?,
                )
            },
            check_at: if self.check_at.trim().is_empty() {
                None
            } else {
                Some(calendar::parse(&self.check_at)?)
            },
            recurrence_days: if self.recurrence_days.trim().is_empty() {
                None
            } else {
                Some(
                    self.recurrence_days
                        .trim()
                        .parse()
                        .map_err(|_| "重复周期请填写 1–28 天")?,
                )
            },
            preparation_hours: if self.preparation_hours.trim().is_empty() {
                24
            } else {
                self.preparation_hours
                    .trim()
                    .parse()
                    .map_err(|_| "筹备提前量请填写小时数")?
            },
        })
    }
}
impl Controller {
    fn retain_assistance_previews(&mut self) -> Result<()> {
        let Some(task_id) = self.active_task.clone() else {
            return Ok(());
        };
        let previews = [
            self.current.as_ref(),
            self.participant_current.as_ref(),
            self.share_current.as_ref(),
        ]
        .into_iter()
        .flatten()
        .cloned()
        .collect::<Vec<_>>();
        self.intent_store()?
            .retain_task_previews(&task_id, &previews, now())
    }

    fn invalidate_assistance_previews(&mut self) -> Result<()> {
        let grant = self.g("read")?;
        self.journal
            .invalidate_previews(&grant, now())
            .map_err(|_| "目标已变化，旧预览暂不可撤销；请核实执行记录后重新预览")?;
        for preview in [
            &mut self.current,
            &mut self.participant_current,
            &mut self.share_current,
        ] {
            if preview
                .as_ref()
                .is_some_and(|op| matches!(op.status, Status::Prepared | Status::Queued))
            {
                *preview = None;
            }
        }
        self.active_task = None;
        Ok(())
    }

    pub(super) fn check_assistance_confirmation(&self, op: &Operation) -> Result<()> {
        self.g("read")?;
        self.intent_store()?
            .check_operation_confirmation(op, self.active_task.as_deref())
    }

    pub(super) fn intent_store(&self) -> Result<IntentStore> {
        IntentStore::open(&self.data, &self.actor())
    }
    pub(super) fn intent_activities(&self) -> Result<Vec<Activity>> {
        Catalog::open(&self.data)?.list()
    }
    pub(super) fn refresh_assistance(&mut self) -> Result<()> {
        self.g("read")?;
        let clock = now();
        let pending = self
            .journal
            .pending(&self.g("read")?, clock)
            .map_err(|_| "待核实操作不可读取")?;
        let facts = self
            .intent_activities()?
            .into_iter()
            .filter_map(|a| {
                let id = &a.metadata.as_ref()?.activity_id;
                let path = self.data.join("sync-status").join(format!("{id}.json"));
                let status: Value = std::fs::read(path)
                    .ok()
                    .and_then(|b| serde_json::from_slice(&b).ok())
                    .unwrap_or(Value::Null);
                let has_pending_operation = pending.iter().any(|o| o.action.target == a.room);
                let pause = if has_pending_operation {
                    Some("原操作尚待核实，保留名额和原编号。".into())
                } else if self.policies.get(id).is_some_and(|p| p.expires_at <= clock) {
                    Some("自动补位授权已到期，请核实后重新确认规则。".into())
                } else if !status["success"].as_bool().unwrap_or(false) {
                    Some("活动同步尚未完整，暂停推进并保留已有记录。".into())
                } else {
                    None
                };
                Some(VerifiedFacts {
                    observed_at: status["collected_at_unix"].as_u64().unwrap_or(0),
                    complete: status["success"].as_bool().unwrap_or(false),
                    accessible: a
                        .room
                        .parse::<OwnedRoomId>()
                        .ok()
                        .and_then(|id| self.active().get_room(&id))
                        .is_some_and(|r| r.state() == matrix_sdk::RoomState::Joined),
                    activity: a,
                    automation_pause: pause,
                    has_pending_operation,
                })
            })
            .collect::<Vec<_>>();
        let mut store = self.intent_store()?;
        store.refresh_cards(&facts, clock)?;
        let task_operation_ids = store
            .load()?
            .tasks
            .iter()
            .filter(|task| task.status != TaskStatus::Completed)
            .flat_map(|task| {
                task.steps
                    .iter()
                    .filter_map(|step| step.operation_id.clone())
            })
            .collect::<Vec<_>>();
        let mut ops = self
            .journal
            .recent_with_references(&self.g("read")?, &task_operation_ids, clock)
            .map_err(|_| "执行记录不可读取")?;
        ops.extend(pending);
        if let Some(task) = &self.active_task {
            for op in [
                &self.current,
                &self.participant_current,
                &self.article_current,
                &self.share_current,
            ]
            .into_iter()
            .flatten()
            {
                let _ = store.link_operation(task, op, clock);
            }
        }
        store.follow_tasks(&facts, &ops, clock)?;
        let task_state = store.load()?;
        if self.active_task.as_ref().is_some_and(|id| {
            !task_state.tasks.iter().any(|task| {
                &task.id == id && !matches!(task.status, TaskStatus::Completed | TaskStatus::Paused)
            })
        }) {
            self.active_task = None;
        }
        if self.analysis_until > clock && clock.saturating_sub(self.last_analysis) >= 60 {
            let state = store.load()?;
            if let Some(card) = store
                .visible_cards(clock)?
                .into_iter()
                .find(|c| !state.analyses.contains_key(&c.fingerprint))
            {
                if let Some(fact) = facts.iter().find(|f| {
                    f.complete
                        && f.accessible
                        && f.observed_at <= clock
                        && clock.saturating_sub(f.observed_at) <= 120
                        && f.activity
                            .metadata
                            .as_ref()
                            .is_some_and(|m| m.activity_id == card.activity_id)
                }) {
                    let g = self.g("model")?;
                    let permit = self.automation_interlock.issue("_intent_ai");
                    self.last_analysis = clock;
                    let goal = state
                        .goals
                        .iter()
                        .find(|g| Some(g.id.as_str()) == card.goal_id.as_deref());
                    let ids = crate::intent_model::analyze(
                        &self.model,
                        &g,
                        card.kind,
                        &fact.activity,
                        goal,
                    )
                    .unwrap_or_default();
                    if permit.valid()
                        && self.host_session_current()
                        && now() < self.analysis_until
                        && g.check("model", now()).is_ok()
                    {
                        store.cache_analysis(card.fingerprint, ids)?;
                    }
                }
            }
        }
        Ok(())
    }
    pub(crate) fn assistance_notifications(&mut self) {
        if !crate::tray::enabled() || !self.is_authorized() {
            return;
        }
        let clock = now();
        if self.authorized_until.saturating_sub(clock) <= 300
            && self.expiration_notified != self.authorized_until
        {
            let prefs = self
                .intent_store()
                .and_then(|s| s.load())
                .map(|s| s.preferences)
                .unwrap_or_default();
            if !prefs.quiet(clock)
                && prefs.reminders
                && crate::tray::notify(
                    "补位授权即将到期",
                    "请打开补位核对并续期；到期将停止同步、模型调用和发送。",
                )
            {
                self.expiration_notified = self.authorized_until;
            }
        }
        if let Ok(mut s) = self.intent_store() {
            if let Ok(cards) = s.claim_notifications(clock) {
                for c in cards {
                    let _ = crate::tray::notify(&c.title, &c.reason);
                }
            }
        }
    }
    pub(super) fn apply_intention(&mut self, command: IntentCommand) -> Result<String> {
        self.g("read")?;
        if matches!(
            &command,
            IntentCommand::SaveGoal { .. }
                | IntentCommand::SetGoalStatus { .. }
                | IntentCommand::DeleteGoal(_)
                | IntentCommand::PauseTask(_)
                | IntentCommand::Feedback { .. }
                | IntentCommand::UndoFeedback(_)
        ) {
            self.retain_assistance_previews()?;
        }
        match command {
            IntentCommand::PreviewAnalysis => {
                self.g("model")?;
                self.analysis_consent = Some(consent::Consent::prepare(self.actor(), now()));
                Ok("主动 AI 分析预览：仅将活动类型、完整时段、人数统计与本人确认的目标字段发送至 api.minimax.cn，使用共享预算；不发送姓名、账号、房间编号或活动标题。相关事实或目标变化后最多每分钟一次，相同依据复用结果；只选择事实说明，不能执行或修改偏好。与本人授权同时到期，最长一小时，可单独撤销。".into())
            }
            IntentCommand::ConfirmAnalysis(id) => {
                self.g("model")?;
                self.analysis_consent
                    .as_ref()
                    .ok_or("请先查看主动分析说明")?
                    .check(&id, &self.actor(), now())?;
                self.analysis_until = self.authorized_until;
                self.analysis_consent = None;
                Ok("主动分析已由本人单独授权，到期停止。".into())
            }
            IntentCommand::RevokeAnalysis => {
                self.automation_interlock.invalidate("_intent_ai");
                self.analysis_until = 0;
                self.analysis_consent = None;
                Ok("主动 AI 分析已撤销，确定规则提示继续可用。".into())
            }
            IntentCommand::PrepareGoalWithAi { text, kind } => {
                self.apply_intention(IntentCommand::PrepareGoal(kind))?;
                let a = self.activity().ok();
                let permit = self.automation_interlock.issue("_intent_ai");
                let d = crate::intent_model::draft(
                    &self.model,
                    &self.g("model")?,
                    &text,
                    kind,
                    if kind == GoalKind::Organize {
                        a.as_ref()
                    } else {
                        None
                    },
                )?;
                self.g("model")?;
                if !permit.valid() || !self.host_session_current() {
                    return Err("账号或授权已变化，请重新核对目标".into());
                }
                let f = self.goal_form.as_mut().ok_or("请先准备目标")?;
                f.title = d.title;
                f.template = d.template;
                if kind == GoalKind::Participate {
                    f.earliest = d.earliest;
                    f.latest = d.latest;
                    f.group = if d.group == 0 {
                        String::new()
                    } else {
                        d.group.to_string()
                    };
                } else {
                    f.target = if d.target == 0 {
                        String::new()
                    } else {
                        d.target.to_string()
                    };
                    f.check_at = d.check_at;
                }
                self.intention_result = if d.questions.is_empty() {
                    "AI 已准备可编辑目标草稿，请核对所有字段后确认保存。".into()
                } else {
                    format!("还需要你补充：{}", d.questions.join("；"))
                };
                self.goal_ai_draft = true;
                Ok(self.intention_result.clone())
            }
            IntentCommand::PrepareGoal(kind) => {
                self.goal_ai_draft = false;
                let p = self.intent_store()?.load()?.preferences;
                let a = self.activity().ok();
                if kind == GoalKind::Organize
                    && a.as_ref()
                        .is_none_or(|a| a.owner != self.actor() || a.metadata.is_none())
                {
                    return Err("请先选择本人已创建、带完整日期的活动".into());
                }
                self.selected_goal = None;
                self.goal_form = Some(GoalForm {
                    kind,
                    activity_id: if kind == GoalKind::Organize {
                        a.as_ref()
                            .and_then(|a| a.metadata.as_ref())
                            .map(|m| m.activity_id.clone())
                    } else {
                        None
                    },
                    title: if kind == GoalKind::Organize {
                        "把这场活动办起来".into()
                    } else {
                        "找到适合自己的活动".into()
                    },
                    template: a
                        .as_ref()
                        .and_then(|a| a.metadata.as_ref())
                        .map(|m| m.template.clone())
                        .or(p.template)
                        .unwrap_or("any".into()),
                    earliest: a
                        .as_ref()
                        .map(|a| calendar::display(a.start))
                        .unwrap_or_default(),
                    latest: a
                        .as_ref()
                        .map(|a| calendar::display(a.end))
                        .unwrap_or_default(),
                    group: if kind == GoalKind::Organize {
                        "1".into()
                    } else {
                        p.group.map(|n| n.to_string()).unwrap_or_default()
                    },
                    target: if kind == GoalKind::Organize {
                        a.as_ref()
                            .map(|a| a.capacity.to_string())
                            .unwrap_or_default()
                    } else {
                        String::new()
                    },
                    check_at: if kind == GoalKind::Organize {
                        a.as_ref()
                            .map(|a| {
                                calendar::display(a.start.saturating_sub(86400).max(now() + 60))
                            })
                            .unwrap_or_default()
                    } else {
                        String::new()
                    },
                    recurrence_days: String::new(),
                    preparation_hours: "24".into(),
                });
                self.intention_result =
                    "核对本次目标。时间与人数为空时请补充；填入的长期偏好仍可修改。".into();
                Ok(self.intention_result.clone())
            }
            IntentCommand::SelectGoal(id) => {
                self.goal_ai_draft = false;
                let g = self
                    .intent_store()?
                    .load()?
                    .goals
                    .into_iter()
                    .find(|g| g.id == id)
                    .ok_or("目标已不存在")?;
                self.selected_goal = Some(id);
                self.goal_form = Some(GoalForm::from_goal(&g));
                Ok("目标已打开，修改后确认保存；原执行记录保留。".into())
            }
            IntentCommand::SaveGoal { id, form } => {
                let suggested_fields = if self.goal_ai_draft {
                    let old = serde_json::to_value(&self.goal_form).unwrap_or(Value::Null);
                    let edited = serde_json::to_value(&form).unwrap_or(Value::Null);
                    [
                        "title", "template", "earliest", "latest", "group", "target", "check_at",
                    ]
                    .into_iter()
                    .filter(|key| {
                        old[*key] == edited[*key]
                            && old[*key].as_str().is_some_and(|s| !s.is_empty())
                            && (form.kind != GoalKind::Organize
                                || matches!(*key, "title" | "template" | "target" | "check_at"))
                    })
                    .map(String::from)
                    .collect::<Vec<_>>()
                } else {
                    Vec::new()
                };
                let activities = self.intent_activities()?;
                let mut store = self.intent_store()?;
                let goal = store.save_goal(id.as_deref(), form.input()?, &activities, now())?;
                let goal = store.confirm_suggestion_sources(&goal.id, &suggested_fields)?;
                self.invalidate_assistance_previews()?;
                self.goal_ai_draft = false;
                self.selected_goal = Some(goal.id.clone());
                self.goal_form = Some(GoalForm::from_goal(&goal));
                self.intention_result = "目标已按本人账号保存；本次要求优先于长期偏好。".into();
                Ok(self.intention_result.clone())
            }
            IntentCommand::SetGoalStatus { id, status } => {
                self.intent_store()?.set_goal_status(&id, status, now())?;
                self.invalidate_assistance_previews()?;
                Ok("目标状态已更新。".into())
            }
            IntentCommand::DeleteGoal(id) => {
                self.intent_store()?.delete_goal(&id)?;
                self.invalidate_assistance_previews()?;
                if self.selected_goal.as_deref() == Some(id.as_str()) {
                    self.selected_goal = None;
                    self.goal_form = None;
                }
                Ok("目标已删除，已执行操作的回执仍保留。".into())
            }
            IntentCommand::SavePreferences(p) => {
                self.intent_store()?.save_preferences(p, now())?;
                Ok("长期偏好已由本人确认保存。单次报名不会自动修改长期偏好。".into())
            }
            IntentCommand::ResetPreferences => {
                self.intent_store()?.reset_preferences()?;
                Ok("个人偏好已恢复默认。".into())
            }
            IntentCommand::ControlCard {
                id,
                snooze,
                disable,
            } => {
                self.intent_store()?.control_card(
                    &id,
                    if snooze { Some(now() + 3600) } else { None },
                    disable,
                    now(),
                )?;
                Ok(if snooze {
                    "已设为一小时后提醒。"
                } else {
                    "建议设置已保存。"
                }
                .into())
            }
            IntentCommand::PauseTask(id) => {
                self.intent_store()?.pause_task(&id, now())?;
                self.invalidate_assistance_previews()?;
                Ok("任务已暂停，已发生的操作继续沿原编号核实。".into())
            }
            IntentCommand::ResumeTask(id) => {
                let task = self.intent_store()?.resume_task(&id, now())?;
                self.select_activity(task.activity_id)?;
                self.active_task = Some(id);
                self.assistance_route = Some(1);
                self.recover_pending()?;
                match task.action {
                    SuggestedAction::Share => self.apply(Command::LoadContacts),
                    SuggestedAction::NextDraft => self.apply(Command::CopyActivity),
                    _ => Ok(
                        "已打开原任务对应活动，先核实原编号；新的报名或回复仍需本人预览确认。"
                            .into(),
                    ),
                }
            }
            IntentCommand::Feedback {
                goal_id,
                template,
                group,
                scope,
            } => {
                let activities = self.intent_activities()?;
                self.intent_store()?.feedback(
                    &goal_id,
                    parse_template(&template)?,
                    group,
                    scope,
                    &activities,
                    now(),
                )?;
                self.invalidate_assistance_previews()?;
                let g = self
                    .intent_store()?
                    .load()?
                    .goals
                    .into_iter()
                    .find(|g| g.id == goal_id)
                    .ok_or("目标不存在")?;
                self.selected_goal = Some(goal_id);
                self.goal_form = Some(GoalForm::from_goal(&g));
                Ok(if scope == FeedbackScope::ThisOccasion {
                    "仅修改本次目标，长期偏好保持本人原确认内容。"
                } else {
                    "本次目标与长期偏好已由本人明确确认更新，可在反馈记录中撤回。"
                }
                .into())
            }
            IntentCommand::UndoFeedback(id) => {
                self.intent_store()?.undo_feedback(&id, now())?;
                self.invalidate_assistance_previews()?;
                self.goal_form = None;
                self.selected_goal = None;
                Ok("反馈已撤回，请重新打开目标核对。".into())
            }
            IntentCommand::UseCard { id, fingerprint } => {
                self.refresh_assistance()?;
                let card = self
                    .intent_store()?
                    .visible_cards(now())?
                    .into_iter()
                    .find(|c| c.id == id && c.fingerprint == fingerprint)
                    .ok_or("建议依据已变化或过期，请核对最新卡片")?;
                let task = self.intent_store()?.start_task(&id, &fingerprint, now())?;
                self.select_activity(card.activity_id.clone())?;
                self.active_task = Some(task.id);
                match card.action {
                    SuggestedAction::Share => {
                        self.assistance_route = Some(1);
                        self.apply(Command::LoadContacts)
                    }
                    SuggestedAction::Register => {
                        let goal = self
                            .intent_store()?
                            .load()?
                            .goals
                            .into_iter()
                            .find(|g| {
                                Some(g.id.as_str()) == card.goal_id.as_deref()
                                    && g.revision == card.goal_revision
                                    && g.status == GoalStatus::Active
                            })
                            .ok_or("目标已变化，请重新核对")?;
                        self.assistance_route = Some(1);
                        self.apply(Command::Join(Preferences {
                            earliest: self.activity()?.start,
                            latest: self.activity()?.end,
                            group: goal.input.group,
                        }))
                    }
                    SuggestedAction::ReviewInvitation | SuggestedAction::ReviewConflict => {
                        self.assistance_route = Some(1);
                        Ok(
                            "已打开对应活动，请核对本人报名和邀请结果；接受或取消由本人预览确认。"
                                .into(),
                        )
                    }
                    SuggestedAction::Renew => {
                        self.assistance_route = Some(4);
                        Ok(
                            "请查看本人授权范围，续期后先核实旧操作，再确认本场自动补位规则。"
                                .into(),
                        )
                    }
                    SuggestedAction::Reconcile => {
                        self.assistance_route = Some(2);
                        self.apply(Command::ReconcilePending)
                    }
                    SuggestedAction::NextDraft => {
                        self.assistance_route = Some(1);
                        self.apply(Command::CopyActivity)
                    }
                }
            }
        }
    }
    pub(super) fn intention_view(&self, v: &mut View) {
        match self.intent_store().and_then(|s| s.load()) {
            Ok(state) => {
                v.task_choices = state
                    .tasks
                    .iter()
                    .rev()
                    .filter(|t| {
                        t.status != buwei_host_core::assistance_tasks::TaskStatus::Completed
                    })
                    .map(|t| {
                        (
                            t.id.clone(),
                            buwei_host_core::assistance_tasks::task_text(t),
                        )
                    })
                    .collect();
                v.assistance_tasks = state
                    .tasks
                    .iter()
                    .rev()
                    .take(20)
                    .map(buwei_host_core::assistance_tasks::task_text)
                    .collect::<Vec<_>>()
                    .join("\n\n");
                v.latest_feedback = state.feedback.iter().rev().find(|f| !f.undone).map(|f| {
                    (
                        f.id.clone(),
                        format!(
                            "最近反馈：{} · {} 人 · {} · {}",
                            template_name(&f.after_goal.template),
                            f.after_goal.group,
                            if f.scope == FeedbackScope::ThisOccasion {
                                "仅本次"
                            } else {
                                "更新长期偏好"
                            },
                            calendar::display(f.created_at)
                        ),
                    )
                });
                v.goals = state
                    .goals
                    .iter()
                    .map(|g| (g.id.clone(), goal_text(g)))
                    .collect();
                v.personal_preferences = Some(state.preferences);
            }
            Err(e) => v.intention_result = e,
        }
        v.goal_form = self.goal_form.clone();
        if let Some(form) = &self.goal_form {
            let prefs = v.personal_preferences.as_ref();
            let saved_sources = self
                .intent_store()
                .and_then(|s| s.load())
                .ok()
                .and_then(|s| {
                    s.goals
                        .into_iter()
                        .find(|g| Some(&g.id) == self.selected_goal.as_ref())
                })
                .map(|g| g.field_sources)
                .unwrap_or_default();
            let source = |name: &str| {
                if self.goal_ai_draft {
                    return "待确认建议";
                }
                let field = match name {
                    "dates" => "earliest",
                    "organizer" => "target",
                    other => other,
                };
                if saved_sources.get(field) == Some(&FieldSource::ConfirmedSuggestion) {
                    return "本人确认的建议";
                }
                if form.kind == GoalKind::Participate
                    && prefs.is_some_and(|p| {
                        p.confirmed_at > 0
                            && (name == "template"
                                && p.template.as_deref() == Some(form.template.as_str())
                                || name == "group"
                                    && p.group.map(|n| n.to_string()).as_deref()
                                        == Some(form.group.as_str()))
                    })
                {
                    "本人确认的偏好"
                } else {
                    "本人填写"
                }
            };
            v.goal_sources = format!(
                "目标名称：{} · 活动类型：{} · 起止时间：{} · 同行人数：{}",
                source("title"),
                source("template"),
                source("dates"),
                source("group")
            );
            if form.kind == GoalKind::Organize {
                v.goal_sources.push_str(&format!(
                    "\n目标人数、检查时间、周期与提前量：{}",
                    source("organizer")
                ));
            }
            if prefs.is_some_and(|p| {
                form.kind == GoalKind::Participate
                    && p.confirmed_at > 0
                    && (p
                        .group
                        .is_some_and(|n| !form.group.is_empty() && n.to_string() != form.group)
                        || p.template
                            .as_deref()
                            .is_some_and(|t| t != "any" && t != form.template))
            }) {
                v.goal_sources.push_str(
                    "\n本次要求与长期偏好不同，请核对后确认；本次优先，长期偏好不会自动修改。",
                );
            }
        }
        v.goal_id = self.selected_goal.clone();
        if self.is_authorized() {
            v.assistance_cards = self
                .intent_store()
                .and_then(|s| s.visible_cards(now()))
                .unwrap_or_default();
            if let Ok(state) = self.intent_store().and_then(|s| s.load()) {
                for card in &mut v.assistance_cards {
                    if let Some(ids) = state.analyses.get(&card.fingerprint) {
                        let focus = ids
                            .iter()
                            .filter_map(|id| match id.as_str() {
                                "reason" => Some("触发原因"),
                                "evidence" => Some("已核验的活动与人数"),
                                "deadline" => Some("建议有效期"),
                                _ => None,
                            })
                            .collect::<Vec<_>>();
                        if !focus.is_empty() {
                            card.reason
                                .push_str(&format!("\nAI 建议优先核对：{}。", focus.join("、")));
                        }
                    }
                }
            }
        }
        v.analysis_consent_id = self.analysis_consent.as_ref().map(|p| p.id.clone());
        v.analysis_status = if self.analysis_consent.is_some() {
            "主动分析预览：发送去身份的活动类型、时段、人数与本人目标字段至 api.minimax.cn；使用共享预算，变化后最多每分钟一次，相同事实复用结果。只解释已有事实，不能执行或更新偏好；随本人授权到期，最长一小时，可单独撤销。".into()
        } else if self.analysis_until > now() {
            format!(
                "主动 AI 分析已授权至 {}；相同事实复用结果，规则负责执行边界",
                calendar::display(self.analysis_until)
            )
        } else {
            "主动 AI 分析未开启；规则建议和手动目标持续可用".into()
        };
        v.assistance_route = self.assistance_route;
        v.background_status = if crate::host::background() && crate::host::paused() {
            "值守已暂停，授权已撤销；重新开启后请核对并重新授权"
        } else if crate::host::background() {
            "已开启窗口关闭后的值守；授权到期停止，托盘可暂停或退出"
        } else {
            "后台值守未开启；真正退出后停止同步与发送"
        }
        .into();
        if v.intention_result.is_empty() {
            v.intention_result = self.intention_result.clone();
        }
    }
}
