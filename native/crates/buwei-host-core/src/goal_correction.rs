//! Account-local, revision-bound goal edits. Confirming an edit never sends a registration.
use crate::{Activity, Result, assistance::*, intent_feedback::*};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FieldChange {
    pub field: String,
    pub value: String,
    pub evidence: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GoalCorrection {
    pub id: String,
    pub goal_id: String,
    pub base_revision: u64,
    pub before: GoalInput,
    pub after: GoalInput,
    pub changes: Vec<FieldChange>,
    pub questions: Vec<String>,
    pub before_preferences: PersonalPreferences,
    pub expires_at: u64,
    pub applied: bool,
}
pub fn corrected_input(
    before: &GoalInput,
    text: &str,
    changes: &[FieldChange],
) -> Result<GoalInput> {
    if text.trim().is_empty() || text.len() > 4000 || changes.len() > 4 {
        return Err("请简短说明本次变化".into());
    }
    let mut after = before.clone();
    let mut seen = std::collections::BTreeSet::new();
    for change in changes {
        if !seen.insert(&change.field)
            || change.evidence.trim().is_empty()
            || change.evidence.len() > 600
            || !text.contains(change.evidence.as_str())
        {
            return Err("修改字段缺少本人原话依据，请重新核对".into());
        }
        let has_cue = |cues: &[&str]| cues.iter().any(|s| change.evidence.contains(s));
        match change.field.as_str() {
            "template" => {
                if !has_cue(&[
                    "读书",
                    "羽毛球",
                    "桌游",
                    "reading",
                    "badminton",
                    "boardgame",
                    "不限",
                    "自定义",
                ]) {
                    return Err("请明确活动类型变化".into());
                }
                after.template = change.value.clone();
            }
            "group" => {
                if !has_cue(&["人", "独自", "同行", "朋友", "同伴", "只有我"]) {
                    return Err("请明确同行人数变化".into());
                }
                after.group = change.value.parse().map_err(|_| "同行人数须为 1–8 人")?;
            }
            "earliest" | "latest" => {
                if !has_cue(&[
                    "点", ":", "：", "时", "周", "月", "日", "明天", "今天", "今晚", "到", "开始",
                    "结束", "早", "晚",
                ]) {
                    return Err("请明确时间变化".into());
                }
                let time = crate::calendar::parse(&change.value)?;
                if change.field == "earliest" {
                    after.earliest = time;
                } else {
                    after.latest = time;
                }
                after.availability = None; // An explicit occasion overrides the weekly snapshot.
            }
            _ => return Err("模型提出了不支持的修改字段".into()),
        }
    }
    Ok(after)
}
impl IntentStore {
    /// Resume the same registration task only after old in-flight effects have been reconciled.
    pub fn prepare_registration_update(
        &mut self,
        goal_id: &str,
        activity: &Activity,
        operations: &[action_receipts::Operation],
        clock: u64,
    ) -> Result<crate::assistance_tasks::AssistanceTask> {
        use crate::assistance_tasks::{AssistanceTask, TaskStatus, TaskStep};
        use crate::proactive::SuggestedAction;
        use action_receipts::Status;
        let mut state = self.load()?;
        let goal = state
            .goals
            .iter()
            .find(|g| {
                g.id == goal_id
                    && g.status == GoalStatus::Active
                    && g.input.kind == GoalKind::Participate
            })
            .ok_or("请先选择有效的参与目标")?
            .clone();
        goal.input
            .validate(&state.account, &[activity.clone()], clock)?;
        let activity_id = activity
            .metadata
            .as_ref()
            .filter(|m| !m.archived)
            .ok_or("活动尚未接入核验")?
            .activity_id
            .clone();
        if goal
            .input
            .activity_id
            .as_ref()
            .is_some_and(|id| id != &activity_id)
        {
            return Err("目标关联另一场活动".into());
        }
        let person = activity
            .people
            .iter()
            .find(|p| p.account == state.account && p.status == crate::PersonStatus::Waiting)
            .ok_or("只可更新候补报名；已确认或获邀请请先核对取消与重新报名流程")?;
        if activity
            .invitations
            .iter()
            .any(|i| i.recipient == state.account && i.reply == crate::Reply::Pending)
        {
            return Err("已有保留名额，请先处理当前邀请".into());
        }
        if operations.iter().any(|op| {
            op.account == state.account
                && op.action.target == activity.room
                && matches!(op.status, Status::Dispatching | Status::Unknown)
        }) {
            return Err("还有原编号待核实操作，请先恢复回执")?;
        }
        if goal
            .input
            .availability
            .as_ref()
            .is_some_and(|s| !s.covers(activity.start, activity.end))
        {
            return Err("本场时间不在本人确认的每周安排内，请先核对目标")?;
        }
        let preferences = crate::Preferences {
            earliest: goal.input.earliest,
            latest: goal.input.latest,
            group: goal.input.group,
        };
        preferences.validate()?;
        if person.preferences == preferences {
            return Err("本场报名与目标一致，无需重复更新".into());
        }
        let position = state.tasks.iter().position(|t| {
            t.goal_id.as_deref() == Some(goal_id)
                && t.activity_id == activity_id
                && t.action == SuggestedAction::Register
                && (t.registration_update_sequence == Some(person.joined)
                    || t.registration_sequence
                        .is_some_and(|before| before < person.joined))
                && t.status != TaskStatus::Completed
        });
        if let Some(index) = position {
            let task = &mut state.tasks[index];
            if task
                .steps
                .iter()
                .filter_map(|step| step.operation_id.as_ref())
                .any(|id| {
                    operations.iter().any(|op| {
                        &op.id == id
                            && matches!(
                                op.status,
                                Status::Prepared
                                    | Status::Queued
                                    | Status::Dispatching
                                    | Status::Unknown
                            )
                    })
                })
            {
                return Err("旧任务仍有未处理预览或回执，请先核实".into());
            }
            task.goal_revision = goal.revision;
            task.registration_update_sequence = Some(person.joined);
            task.status = TaskStatus::AwaitingConfirmation;
            task.updated_at = clock;
            task.steps.push(TaskStep {
                label: "本人更正目标，单独确认候补更新".into(),
                operation_id: None,
                status: TaskStatus::AwaitingConfirmation,
                evidence: Some(format!("新目标版本 {}；候补原排位保持", goal.revision)),
            });
            let result = task.clone();
            self.save(&state)?;
            return Ok(result);
        }
        if state.tasks.len() >= 500 {
            return Err("任务记录已满，请先归档旧任务".into());
        }
        let task = AssistanceTask {
            id: action_receipts::new_id(),
            card_id: format!("correction:{}", goal.id),
            goal_id: Some(goal.id),
            goal_revision: goal.revision,
            activity_id,
            room: activity.room.clone(),
            action: SuggestedAction::Register,
            title: "更新本场候补并继续跟进".into(),
            status: TaskStatus::AwaitingConfirmation,
            steps: vec![TaskStep {
                label: "本人单独确认候补信息".into(),
                operation_id: None,
                status: TaskStatus::AwaitingConfirmation,
                evidence: Some("保留原报名排位；发送后核验组织者状态".into()),
            }],
            created_at: clock,
            updated_at: clock,
            registration_sequence: Some(person.joined),
            registration_update_sequence: Some(person.joined),
        };
        state.tasks.push(task.clone());
        self.save(&state)?;
        Ok(task)
    }
    pub fn prepare_correction(
        &mut self,
        goal_id: &str,
        text: &str,
        changes: Vec<FieldChange>,
        questions: Vec<String>,
        activities: &[Activity],
        clock: u64,
    ) -> Result<GoalCorrection> {
        if questions.len() > 3
            || questions.iter().any(|q| {
                q.trim().is_empty()
                    || q.len() > 480
                    || ["http", "密码", "密钥", "身份证"]
                        .iter()
                        .any(|s| q.contains(s))
            })
        {
            return Err("补充问题超出必要范围".into());
        }
        let mut state = self.load()?;
        let goal = state
            .goals
            .iter()
            .find(|g| {
                g.id == goal_id
                    && g.status == GoalStatus::Active
                    && g.input.kind == GoalKind::Participate
            })
            .ok_or("请先选择本人有效的参与目标")?;
        let after = corrected_input(&goal.input, text, &changes)?;
        if questions.is_empty() {
            after.validate(&state.account, activities, clock)?;
            if after == goal.input {
                return Err("没有明确的修改，请补充时间、人数或类型变化".into());
            }
        }
        let proposal = GoalCorrection {
            id: action_receipts::new_id(),
            goal_id: goal.id.clone(),
            base_revision: goal.revision,
            before: goal.input.clone(),
            after,
            changes,
            questions,
            before_preferences: state.preferences.clone(),
            expires_at: clock + 600,
            applied: false,
        };
        state
            .corrections
            .retain(|p| !p.applied && p.expires_at > clock && p.goal_id != goal_id);
        if state.corrections.len() >= 100 {
            return Err("请先处理已有修改草稿".into());
        }
        state.corrections.push(proposal.clone());
        self.save(&state)?;
        Ok(proposal)
    }
    pub fn confirm_correction(
        &mut self,
        id: &str,
        scope: FeedbackScope,
        activities: &[Activity],
        clock: u64,
    ) -> Result<ActivityGoal> {
        let mut state = self.load()?;
        let p = state
            .corrections
            .iter()
            .find(|p| p.id == id && !p.applied && p.expires_at > clock && p.questions.is_empty())
            .cloned()
            .ok_or("修改草稿已过期、已确认或仍需补充")?;
        let index = state
            .goals
            .iter()
            .position(|g| {
                g.id == p.goal_id
                    && g.revision == p.base_revision
                    && g.input == p.before
                    && g.status == GoalStatus::Active
            })
            .ok_or("目标已有变化，旧修改确认已失效")?;
        p.after.validate(&state.account, activities, clock)?;
        let before_preferences = state.preferences.clone();
        if scope == FeedbackScope::LongTerm {
            if state.preferences != p.before_preferences {
                return Err("长期偏好已变化，请重新准备修改")?;
            }
            for change in &p.changes {
                match change.field.as_str() {
                    "template" => state.preferences.template = Some(p.after.template.clone()),
                    "group" => state.preferences.group = Some(p.after.group),
                    "earliest" | "latest" => {
                        if p.after.latest - p.after.earliest >= 86400 {
                            return Err("长期时间偏好请先确认具体一天的可用窗口")?;
                        }
                        if state.preferences.earliest_minute.is_none()
                            || state.preferences.latest_minute.is_none()
                        {
                            return Err("更新长期时段前，请先在个人偏好中确认每周安排".into());
                        }
                        if change.field == "earliest" {
                            state.preferences.earliest_minute =
                                Some(((p.after.earliest / 60 + 8 * 60) % 1440) as u16);
                        } else {
                            state.preferences.latest_minute =
                                Some(((p.after.latest / 60 + 8 * 60) % 1440) as u16);
                        }
                    }
                    _ => return Err("不支持的修改字段".into()),
                }
            }
            state.preferences.confirmed_at = clock;
            state.preferences.validate()?;
        }
        let g = &mut state.goals[index];
        g.input = p.after.clone();
        g.revision += 1;
        g.updated_at = clock;
        g.field_sources = field_sources(&g.input, &state.preferences);
        for change in &p.changes {
            g.field_sources
                .insert(change.field.clone(), FieldSource::ConfirmedSuggestion);
        }
        let goal = g.clone();
        state.feedback.push(IntentFeedback {
            id: action_receipts::new_id(),
            goal_id: goal.id.clone(),
            scope,
            before: before_preferences,
            after: state.preferences.clone(),
            before_goal: p.before,
            after_goal: p.after,
            goal_revision: goal.revision,
            created_at: clock,
            undone: false,
        });
        state
            .corrections
            .iter_mut()
            .find(|c| c.id == id)
            .unwrap()
            .applied = true;
        self.save(&state)?;
        Ok(goal)
    }
}
pub fn correction_text(p: &GoalCorrection) -> String {
    let value = |g: &GoalInput, field: &str| match field {
        "group" => format!("{} 人", g.group),
        "template" => template_name(&g.template).into(),
        "earliest" => crate::calendar::display(g.earliest),
        _ => crate::calendar::display(g.latest),
    };
    let changes = p
        .changes
        .iter()
        .map(|c| {
            format!(
                "{}：{} → {}（依据：{}）",
                match c.field.as_str() {
                    "group" => "同行人数",
                    "template" => "活动类型",
                    "earliest" => "可用开始",
                    _ => "可用结束",
                },
                value(&p.before, &c.field),
                value(&p.after, &c.field),
                c.evidence
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "原目标版本 {}\n{}\n{}\n确认只保存目标；更新本场报名需另行预览确认。",
        p.base_revision,
        changes,
        if p.questions.is_empty() {
            "请选择仅本次或更新长期偏好。".into()
        } else {
            format!("请补充：{}", p.questions.join("；"))
        }
    )
}
