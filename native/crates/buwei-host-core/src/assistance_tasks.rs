//! Durable task progress references the existing receipt journal, never replaces it.
use crate::{Result, assistance::*, proactive::*};
use action_receipts::{Operation, Status};
use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    AwaitingConfirmation,
    Executing,
    WaitingReply,
    Reconciling,
    Completed,
    Paused,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TaskStep {
    pub label: String,
    pub operation_id: Option<String>,
    pub status: TaskStatus,
    pub evidence: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AssistanceTask {
    pub id: String,
    pub card_id: String,
    pub goal_id: Option<String>,
    pub goal_revision: u64,
    pub activity_id: String,
    pub room: String,
    pub action: SuggestedAction,
    pub title: String,
    pub status: TaskStatus,
    pub steps: Vec<TaskStep>,
    pub created_at: u64,
    pub updated_at: u64,
    #[serde(default)]
    pub registration_sequence: Option<u64>,
}
fn belongs(t: &AssistanceTask, op: &Operation) -> bool {
    if t.action != SuggestedAction::Reconcile && op.created_at < t.created_at {
        return false;
    }
    let direct = op.action.target == t.room;
    match t.action {
        SuggestedAction::Share => {
            (op.action.permission == "share_card"
                && op.action.payload["card"]["room"] == t.room
                && op.action.payload["card"]["activity_id"] == t.activity_id)
                || (direct && op.action.permission == "invite")
        }
        SuggestedAction::Register => {
            direct
                && op.action.permission == "participate"
                && matches!(
                    op.action.payload["kind"].as_str(),
                    Some("join" | "reply" | "cancel")
                )
        }
        SuggestedAction::ReviewInvitation => {
            direct
                && op.action.permission == "participate"
                && matches!(op.action.payload["kind"].as_str(), Some("reply" | "cancel"))
                && (op.action.payload["kind"] == "cancel"
                    || !t.card_id.starts_with("invitation:")
                    || op.action.payload["invitation_id"].as_str()
                        == t.card_id.strip_prefix("invitation:"))
        }
        SuggestedAction::ReviewConflict => {
            direct && op.action.permission == "participate" && op.action.payload["kind"] == "cancel"
        }
        SuggestedAction::Reconcile => {
            direct
                && matches!(
                    op.status,
                    Status::Dispatching | Status::Unknown | Status::Confirmed
                )
        }
        _ => false,
    }
}
impl IntentStore {
    pub fn start_task(
        &mut self,
        id: &str,
        fingerprint: &str,
        clock: u64,
    ) -> Result<AssistanceTask> {
        let card = self
            .visible_cards(clock)?
            .into_iter()
            .find(|c| c.id == id && c.fingerprint == fingerprint)
            .ok_or("建议已更新，旧建议不能执行")?;
        let mut s = self.load()?;
        if let Some(t) = s.tasks.iter().rev().find(|t| {
            t.card_id == id
                && t.goal_revision == card.goal_revision
                && !matches!(t.status, TaskStatus::Completed | TaskStatus::Paused)
        }) {
            return Ok(t.clone());
        }
        if s.tasks.len() >= 500 {
            return Err("任务记录已满，请先归档旧任务")?;
        }
        let label = match card.action {
            SuggestedAction::Share => "本人选择接收者并确认分享",
            SuggestedAction::Register => "本人核对报名信息并确认",
            SuggestedAction::ReviewInvitation => "本人核对邀请并回复",
            SuggestedAction::ReviewConflict => "本人决定保留或取消哪场活动",
            SuggestedAction::Renew => "本人续期授权并核实旧操作",
            SuggestedAction::Reconcile => "沿原编号核实执行结果",
            SuggestedAction::NextDraft => "本人确认下一场日期、地点与人数",
        };
        let t = AssistanceTask {
            id: action_receipts::new_id(),
            card_id: id.into(),
            goal_id: card.goal_id,
            goal_revision: card.goal_revision,
            activity_id: card.activity_id,
            room: card.room,
            action: card.action,
            title: card.title,
            status: TaskStatus::AwaitingConfirmation,
            steps: vec![TaskStep {
                label: label.into(),
                operation_id: None,
                status: TaskStatus::AwaitingConfirmation,
                evidence: Some(card.evidence),
            }],
            created_at: clock,
            updated_at: clock,
            registration_sequence: card.registration_sequence,
        };
        s.tasks.push(t.clone());
        self.save(&s)?;
        Ok(t)
    }
    pub fn link_operation(&mut self, task_id: &str, op: &Operation, clock: u64) -> Result<()> {
        let mut s = self.load()?;
        if op.account != s.account {
            return Err("旧账号操作不能关联本人的任务".into());
        }
        let t = s
            .tasks
            .iter_mut()
            .find(|t| {
                t.id == task_id
                    && t.status != TaskStatus::Paused
                    && t.status != TaskStatus::Completed
            })
            .ok_or("任务已暂停或完成")?;
        if !belongs(t, op) {
            return Err("此操作不属于任务的活动与步骤".into());
        }
        if t.steps
            .iter()
            .any(|x| x.operation_id.as_deref() == Some(&op.id))
        {
            return Ok(());
        }
        t.steps.push(TaskStep {
            label: op.action.summary.clone(),
            operation_id: Some(op.id.clone()),
            status: TaskStatus::AwaitingConfirmation,
            evidence: None,
        });
        t.updated_at = clock;
        self.save(&s)
    }
    pub fn follow_tasks(
        &mut self,
        facts: &[VerifiedFacts],
        operations: &[Operation],
        clock: u64,
    ) -> Result<()> {
        let mut s = self.load()?;
        for t in &mut s.tasks {
            if t.status == TaskStatus::Completed {
                continue;
            }
            let changed = t.goal_id.as_ref().is_some_and(|id| {
                !s.goals.iter().any(|g| {
                    &g.id == id && g.revision == t.goal_revision && g.status == GoalStatus::Active
                })
            });
            let paused = t.status == TaskStatus::Paused || changed;
            if changed
                && !t
                    .steps
                    .iter()
                    .any(|step| step.label == "目标已修改、暂停或删除")
            {
                t.steps.push(TaskStep {
                    label: "目标已修改、暂停或删除".into(),
                    operation_id: None,
                    status: TaskStatus::Paused,
                    evidence: Some("旧建议停止；已经发生的操作继续沿原编号核实".into()),
                });
            }
            let mut has_effect = false;
            let mut unresolved = false;
            let mut has_preview = false;
            let mut failed = false;
            // Recover a journal write made just before an interrupted task
            // update. The account, action type and activity remain bound.
            for op in operations
                .iter()
                .filter(|o| !paused && o.account == s.account && belongs(t, o))
                .collect::<Vec<_>>()
            {
                if !t
                    .steps
                    .iter()
                    .any(|step| step.operation_id.as_deref() == Some(op.id.as_str()))
                {
                    t.steps.push(TaskStep {
                        label: op.action.summary.clone(),
                        operation_id: Some(op.id.clone()),
                        status: TaskStatus::AwaitingConfirmation,
                        evidence: None,
                    });
                }
            }
            for step in &mut t.steps {
                let Some(id) = &step.operation_id else {
                    continue;
                };
                let Some(op) = operations
                    .iter()
                    .find(|o| o.id == *id && o.account == s.account)
                else {
                    unresolved = true;
                    step.status = TaskStatus::Reconciling;
                    continue;
                };
                step.status = match op.status {
                    Status::Prepared | Status::Queued if op.expires_at > clock => {
                        has_preview = true;
                        TaskStatus::AwaitingConfirmation
                    }
                    Status::Dispatching | Status::Unknown => {
                        unresolved = true;
                        TaskStatus::Reconciling
                    }
                    Status::Confirmed => {
                        if op
                            .receipt
                            .as_ref()
                            .and_then(|r| r.evidence.as_ref())
                            .is_some_and(|e| {
                                e.operation_id == op.id
                                    && e.account == op.account
                                    && e.target == op.action.target
                                    && e.digest == op.digest
                                    && !e.external_id.is_empty()
                            })
                        {
                            has_effect = true;
                            step.evidence = Some("原编号的服务端回执已核验".into());
                            TaskStatus::Completed
                        } else {
                            unresolved = true;
                            TaskStatus::Reconciling
                        }
                    }
                    _ => {
                        failed = true;
                        TaskStatus::Paused
                    }
                };
            }
            // The first step is a proposed user action, not a receipt. Mark it
            // done only when a matching journal operation has verified evidence.
            let primary_done = t.steps.iter().any(|step| {
                step.status == TaskStatus::Completed
                    && step.operation_id.as_ref().is_some_and(|id| {
                        operations.iter().any(|op| {
                            op.id == *id
                                && op.account == s.account
                                && match t.action {
                                    SuggestedAction::Share => op.action.permission == "share_card",
                                    SuggestedAction::Register => {
                                        op.action.payload["kind"] == "join"
                                    }
                                    SuggestedAction::ReviewInvitation => matches!(
                                        op.action.payload["kind"].as_str(),
                                        Some("reply" | "cancel")
                                    ),
                                    SuggestedAction::ReviewConflict => {
                                        op.action.payload["kind"] == "cancel"
                                    }
                                    SuggestedAction::Reconcile => true,
                                    _ => false,
                                }
                        })
                    })
            });
            if primary_done {
                if let Some(step) = t.steps.first_mut() {
                    step.status = TaskStatus::Completed;
                    step.evidence = Some("本步操作的服务端回执已核验".into());
                }
            }
            if unresolved {
                t.status = TaskStatus::Reconciling;
            } else if has_preview {
                t.status = TaskStatus::AwaitingConfirmation;
            } else if has_effect {
                let f = facts.iter().find(|f| {
                    f.complete
                        && f.accessible
                        && f.observed_at <= clock
                        && clock.saturating_sub(f.observed_at) <= 120
                        && f.activity
                            .metadata
                            .as_ref()
                            .is_some_and(|m| m.activity_id == t.activity_id)
                });
                let achieved = f.is_some_and(|f| match t.action {
                    SuggestedAction::Register => f.activity.people.iter().any(|p| {
                        p.account == s.account
                            && t.registration_sequence
                                .is_some_and(|before| p.joined > before)
                            && matches!(
                                p.status,
                                crate::PersonStatus::Confirmed
                                    | crate::PersonStatus::Declined
                                    | crate::PersonStatus::Cancelled
                            )
                    }),
                    SuggestedAction::ReviewInvitation => operations.iter().any(|op| {
                        op.account == s.account
                            && t.steps.iter().any(|step| {
                                step.operation_id.as_deref() == Some(op.id.as_str())
                                    && step.status == TaskStatus::Completed
                            })
                            && f.activity.revision > op.revision
                            && (if op.action.payload["kind"] == "reply" {
                                f.activity.invitations.iter().any(|i| {
                                    i.recipient == s.account
                                        && op.action.payload["invitation_id"] == i.operation_id
                                        && if op.action.payload["accept"] == true {
                                            i.reply == crate::Reply::Accepted
                                        } else {
                                            i.reply == crate::Reply::Declined
                                        }
                                })
                            } else if op.action.payload["kind"] == "cancel" {
                                f.activity.people.iter().any(|p| {
                                    p.account == s.account
                                        && p.status == crate::PersonStatus::Cancelled
                                        && t.registration_sequence == Some(p.joined)
                                })
                            } else {
                                false
                            })
                    }),
                    SuggestedAction::Share => t
                        .goal_id
                        .as_ref()
                        .and_then(|id| s.goals.iter().find(|g| &g.id == id))
                        .and_then(|g| g.input.target)
                        .is_some_and(|n| f.activity.confirmed() >= n as usize),
                    SuggestedAction::Reconcile => true,
                    SuggestedAction::ReviewConflict => f.activity.people.iter().any(|p| {
                        p.account == s.account && p.status == crate::PersonStatus::Cancelled
                    }),
                    _ => false,
                });
                t.status = if achieved {
                    TaskStatus::Completed
                } else {
                    TaskStatus::WaitingReply
                };
                if achieved && !t.steps.iter().any(|x| x.label == "业务结果已核验") {
                    if !primary_done {
                        if let Some(step) = t.steps.first_mut() {
                            step.status = TaskStatus::Paused;
                            step.evidence =
                                Some("目标已通过其他已核验操作达成；本步尚未执行，无需继续".into());
                        }
                    }
                    t.steps.push(TaskStep {
                        label: "业务结果已核验".into(),
                        operation_id: None,
                        status: TaskStatus::Completed,
                        evidence: Some("报名结果或目标确认人数已由宿主核验；到场尚未反馈".into()),
                    });
                }
            } else if failed {
                t.status = TaskStatus::Paused;
            }
            if paused {
                t.status = TaskStatus::Paused;
            }
            t.updated_at = clock;
        }
        self.save(&s)
    }
    pub fn pause_task(&mut self, id: &str, clock: u64) -> Result<()> {
        let mut s = self.load()?;
        let t = s
            .tasks
            .iter_mut()
            .find(|t| t.id == id)
            .ok_or("任务不存在")?;
        t.status = TaskStatus::Paused;
        t.updated_at = clock;
        self.save(&s)
    }
    pub fn resume_task(&mut self, id: &str, clock: u64) -> Result<AssistanceTask> {
        let mut s = self.load()?;
        let t = s
            .tasks
            .iter_mut()
            .find(|t| t.id == id && t.status != TaskStatus::Completed)
            .ok_or("任务已完成或不存在")?;
        if t.goal_id.as_ref().is_some_and(|id| {
            !s.goals.iter().any(|g| {
                &g.id == id && g.revision == t.goal_revision && g.status == GoalStatus::Active
            })
        }) {
            return Err("目标已变化，请使用新目标的建议")?;
        }
        if t.status == TaskStatus::Paused {
            t.status = TaskStatus::AwaitingConfirmation;
        }
        t.updated_at = clock;
        let result = t.clone();
        self.save(&s)?;
        Ok(result)
    }
    pub fn record_next_activity(
        &mut self,
        id: &str,
        activity: &crate::Activity,
        clock: u64,
    ) -> Result<()> {
        let mut s = self.load()?;
        let active_goals: Vec<_> = s
            .goals
            .iter()
            .filter(|g| g.status == GoalStatus::Active)
            .map(|g| (g.id.clone(), g.revision))
            .collect();
        let t = s
            .tasks
            .iter_mut()
            .find(|t| {
                t.id == id
                    && t.action == SuggestedAction::NextDraft
                    && !matches!(t.status, TaskStatus::Completed | TaskStatus::Paused)
                    && t.goal_id.as_ref().is_some_and(|id| {
                        active_goals
                            .iter()
                            .any(|(g, r)| g == id && *r == t.goal_revision)
                    })
            })
            .ok_or("当前没有下一场筹备任务")?;
        if activity.owner != s.account
            || activity.room == t.room
            || activity.start <= clock
            || activity.metadata.is_none()
            || !activity.people.is_empty()
        {
            return Err("下一场活动尚未独立创建并核验")?;
        }
        activity.validate()?;
        if let Some(step) = t.steps.first_mut() {
            step.status = TaskStatus::Completed;
            step.evidence = Some("本人确认的新活动已由宿主核验".into());
        }
        t.steps.push(TaskStep {
            label: "下一场已由本人确认创建".into(),
            operation_id: None,
            status: TaskStatus::Completed,
            evidence: Some(format!(
                "独立活动 {} · {}；成员重新报名",
                activity.metadata.as_ref().unwrap().activity_id,
                crate::calendar::display(activity.start)
            )),
        });
        t.status = TaskStatus::Completed;
        t.updated_at = clock;
        self.save(&s)
    }
}
fn status_text(status: TaskStatus) -> &'static str {
    match status {
        TaskStatus::AwaitingConfirmation => "待本人确认",
        TaskStatus::Executing => "执行中",
        TaskStatus::WaitingReply => "等待回复",
        TaskStatus::Reconciling => "待核实",
        TaskStatus::Completed => "已完成",
        TaskStatus::Paused => "已暂停",
    }
}
pub fn task_text(t: &AssistanceTask) -> String {
    format!(
        "{} · {}\n{}",
        t.title,
        status_text(t.status),
        t.steps
            .iter()
            .map(|s| format!(
                "{} · {}{}{}",
                s.label,
                status_text(s.status),
                s.operation_id
                    .as_ref()
                    .map(|id| format!(" · 原编号 {id}"))
                    .unwrap_or_default(),
                s.evidence
                    .as_ref()
                    .map(|x| format!("\n{x}"))
                    .unwrap_or_default()
            ))
            .collect::<Vec<_>>()
            .join("\n")
    )
}
