//! A single occasion never silently rewrites confirmed long term preferences.
use crate::{Activity, Result, assistance::*};
use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeedbackScope {
    ThisOccasion,
    LongTerm,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IntentFeedback {
    pub id: String,
    pub goal_id: String,
    pub scope: FeedbackScope,
    pub before: PersonalPreferences,
    pub after: PersonalPreferences,
    pub before_goal: GoalInput,
    pub after_goal: GoalInput,
    pub goal_revision: u64,
    pub created_at: u64,
    pub undone: bool,
}
impl IntentStore {
    pub fn feedback(
        &mut self,
        goal_id: &str,
        template: String,
        group: u8,
        scope: FeedbackScope,
        activities: &[Activity],
        clock: u64,
    ) -> Result<IntentFeedback> {
        let mut s = self.load()?;
        let g = s
            .goals
            .iter_mut()
            .find(|g| g.id == goal_id && g.status == GoalStatus::Active)
            .ok_or("请先选择本人有效目标")?;
        if g.input.kind != GoalKind::Participate {
            return Err("个人同行偏好反馈请在参与目标中填写")?;
        }
        let before_goal = g.input.clone();
        let mut after_goal = before_goal.clone();
        after_goal.template = template;
        after_goal.group = group;
        after_goal.validate(&s.account, activities, clock)?;
        let before = s.preferences.clone();
        let mut after = before.clone();
        if scope == FeedbackScope::LongTerm {
            after.template = Some(after_goal.template.clone());
            after.group = Some(group);
            after.confirmed_at = clock;
            after.validate()?;
            s.preferences = after.clone();
        }
        g.input = after_goal.clone();
        g.field_sources = field_sources(&g.input, &s.preferences);
        g.revision += 1;
        g.updated_at = clock;
        let feedback = IntentFeedback {
            id: action_receipts::new_id(),
            goal_id: goal_id.into(),
            scope,
            before,
            after,
            before_goal,
            after_goal,
            goal_revision: g.revision,
            created_at: clock,
            undone: false,
        };
        s.feedback.push(feedback.clone());
        self.save(&s)?;
        Ok(feedback)
    }
    pub fn undo_feedback(&mut self, id: &str, clock: u64) -> Result<()> {
        let mut s = self.load()?;
        let f = s
            .feedback
            .iter_mut()
            .find(|f| f.id == id && !f.undone)
            .ok_or("反馈已撤回或不存在")?;
        let g = s
            .goals
            .iter_mut()
            .find(|g| g.id == f.goal_id && g.revision == f.goal_revision && g.input == f.after_goal)
            .ok_or("目标已有新修改，旧反馈不能覆盖它")?;
        if f.scope == FeedbackScope::LongTerm && s.preferences != f.after {
            return Err("长期偏好已有新修改，请手动核对，不能用旧反馈覆盖")?;
        }
        if f.scope == FeedbackScope::LongTerm {
            s.preferences = f.before.clone();
        }
        g.input = f.before_goal.clone();
        g.field_sources = field_sources(&g.input, &s.preferences);
        g.revision += 1;
        g.updated_at = clock;
        f.undone = true;
        self.save(&s)
    }
}
