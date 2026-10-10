//! Private, account-scoped intentions. Suggestions never confer execution authority.
use crate::{Activity, Result, account_valid, calendar};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GoalKind {
    Organize,
    Participate,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GoalStatus {
    Active,
    Paused,
    Completed,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GoalInput {
    pub title: String,
    pub kind: GoalKind,
    pub activity_id: Option<String>,
    pub template: String,
    pub earliest: u64,
    pub latest: u64,
    pub group: u8,
    pub target: Option<u8>,
    pub check_at: Option<u64>,
    #[serde(default)]
    pub recurrence_days: Option<u8>,
    #[serde(default = "default_preparation")]
    pub preparation_hours: u8,
    #[serde(default)]
    pub availability: Option<AvailabilitySnapshot>,
}
fn default_preparation() -> u8 {
    24
}
/// A confirmed weekly schedule copied into a goal. Later preference edits do not change it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AvailabilitySnapshot {
    pub weekdays: Vec<u8>,
    pub earliest_minute: u16,
    pub latest_minute: u16,
    pub confirmed_at: u64,
}
impl AvailabilitySnapshot {
    pub fn from_preferences(p: &PersonalPreferences) -> Result<Self> {
        p.validate()?;
        let snapshot = Self {
            weekdays: p.weekdays.clone(),
            earliest_minute: p.earliest_minute.ok_or("请先确认每周可用时段")?,
            latest_minute: p.latest_minute.ok_or("请先确认每周可用时段")?,
            confirmed_at: p.confirmed_at,
        };
        snapshot.validate()?;
        Ok(snapshot)
    }
    pub fn validate(&self) -> Result<()> {
        let p = PersonalPreferences {
            weekdays: self.weekdays.clone(),
            earliest_minute: Some(self.earliest_minute),
            latest_minute: Some(self.latest_minute),
            ..Default::default()
        };
        p.validate()?;
        if self.confirmed_at < calendar::MIN_TIME || self.confirmed_at >= calendar::MAX_TIME {
            return Err("每周安排必须由本人先确认".into());
        }
        Ok(())
    }
    pub fn covers(&self, start: u64, end: u64) -> bool {
        if self.validate().is_err() || !calendar::interval(start, end) {
            return false;
        }
        let day = (start + 8 * 3600) / 86400;
        // An after-midnight activity can belong to yesterday's overnight window.
        [day, day - 1].into_iter().any(|anchor| {
            let weekday = ((anchor + 3) % 7) as u8; // Monday = 0.
            let begin = anchor * 86400 - 8 * 3600 + self.earliest_minute as u64 * 60;
            let finish = anchor * 86400 - 8 * 3600
                + self.latest_minute as u64 * 60
                + if self.latest_minute < self.earliest_minute {
                    86400
                } else {
                    0
                };
            (self.weekdays.is_empty() || self.weekdays.contains(&weekday))
                && start >= begin
                && end <= finish
        })
    }
    pub fn description(&self) -> String {
        let days = if self.weekdays.is_empty() {
            "每天".into()
        } else {
            self.weekdays
                .iter()
                .map(|d| ["周一", "周二", "周三", "周四", "周五", "周六", "周日"][*d as usize])
                .collect::<Vec<_>>()
                .join("、")
        };
        format!(
            "{} {:02}:{:02}–{}{:02}:{:02}（本人确认于 {}）",
            days,
            self.earliest_minute / 60,
            self.earliest_minute % 60,
            if self.latest_minute < self.earliest_minute {
                "次日 "
            } else {
                ""
            },
            self.latest_minute / 60,
            self.latest_minute % 60,
            calendar::display(self.confirmed_at)
        )
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ActivityGoal {
    pub id: String,
    pub revision: u64,
    pub input: GoalInput,
    pub status: GoalStatus,
    pub created_at: u64,
    pub updated_at: u64,
    #[serde(default)]
    pub field_sources: std::collections::BTreeMap<String, FieldSource>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldSource {
    SelfEntered,
    ConfirmedPreference,
    ConfirmedSuggestion,
}
pub(crate) fn field_sources(
    input: &GoalInput,
    p: &PersonalPreferences,
) -> std::collections::BTreeMap<String, FieldSource> {
    let mut sources: std::collections::BTreeMap<_, _> = [
        "title",
        "template",
        "earliest",
        "latest",
        "group",
        "target",
        "check_at",
        "recurrence_days",
        "preparation_hours",
    ]
    .into_iter()
    .map(|f| (f.to_string(), FieldSource::SelfEntered))
    .collect();
    if input.kind == GoalKind::Participate && p.confirmed_at > 0 {
        if p.group == Some(input.group) {
            sources.insert("group".into(), FieldSource::ConfirmedPreference);
        }
        if p.template.as_deref() == Some(input.template.as_str()) {
            sources.insert("template".into(), FieldSource::ConfirmedPreference);
        }
    }
    if input.availability.is_some() {
        sources.insert("availability".into(), FieldSource::ConfirmedPreference);
    }
    sources
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PersonalPreferences {
    pub template: Option<String>,
    pub group: Option<u8>,
    pub weekdays: Vec<u8>,
    pub earliest_minute: Option<u16>,
    pub latest_minute: Option<u16>,
    pub quiet_start: u8,
    pub quiet_end: u8,
    pub reminders: bool,
    pub confirmed_at: u64,
}
impl Default for PersonalPreferences {
    fn default() -> Self {
        Self {
            template: None,
            group: None,
            weekdays: vec![],
            earliest_minute: None,
            latest_minute: None,
            quiet_start: 22,
            quiet_end: 8,
            reminders: true,
            confirmed_at: 0,
        }
    }
}
fn template_valid(s: &str) -> bool {
    matches!(s, "badminton" | "boardgame" | "reading" | "custom" | "any")
}
impl PersonalPreferences {
    pub fn validate(&self) -> Result<()> {
        if self.template.as_deref().is_some_and(|s| !template_valid(s))
            || self.group.is_some_and(|n| !(1..=8).contains(&n))
            || self.weekdays.len() > 7
            || self.weekdays.iter().any(|d| *d > 6)
            || self
                .weekdays
                .iter()
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                != self.weekdays.len()
            || self.earliest_minute.is_some() != self.latest_minute.is_some()
            || self.earliest_minute.is_some_and(|m| m >= 1440)
            || self.latest_minute.is_some_and(|m| m > 1440)
            || matches!((self.earliest_minute,self.latest_minute),(Some(a),Some(b)) if a == b)
            || self.quiet_start > 23
            || self.quiet_end > 23
            || self.quiet_start == self.quiet_end
        {
            return Err("请核对活动类型、同行人数、星期、可用时段和安静时段".into());
        }
        Ok(())
    }
    pub fn quiet(&self, clock: u64) -> bool {
        let hour = ((clock / 3600 + 8) % 24) as u8;
        if self.quiet_start > self.quiet_end {
            hour >= self.quiet_start || hour < self.quiet_end
        } else {
            hour >= self.quiet_start && hour < self.quiet_end
        }
    }
}
impl GoalInput {
    pub fn validate(&self, actor: &str, activities: &[Activity], clock: u64) -> Result<()> {
        if let Some(schedule) = &self.availability {
            schedule.validate()?;
            if self.kind != GoalKind::Participate {
                return Err("每周安排仅用于本人参与目标".into());
            }
        }
        if self.title.trim().is_empty()
            || self.title.chars().count() > 160
            || !template_valid(&self.template)
            || self.earliest < calendar::MIN_TIME
            || self.latest >= calendar::MAX_TIME
            || self.earliest >= self.latest
            || self.latest - self.earliest > 90 * 86400
            || self.earliest % 60 != 0
            || self.latest % 60 != 0
            || self.latest <= clock
            || !(1..=8).contains(&self.group)
            || self.recurrence_days.is_some_and(|d| !(1..=28).contains(&d))
            || self.preparation_hours > 168
        {
            return Err(
                "请补充目标、完整起止日期时间和 1–8 人同行人数；筹备提前量为 0–168 小时".into(),
            );
        }
        match self.kind {
            GoalKind::Organize => {
                let a = activities
                    .iter()
                    .find(|a| {
                        a.metadata.as_ref().is_some_and(|m| {
                            Some(m.activity_id.as_str()) == self.activity_id.as_deref()
                                && !m.archived
                        })
                    })
                    .ok_or("组织目标需要关联本人已创建的活动")?;
                if a.owner != actor
                    || self.target.is_none_or(|n| n == 0 || n > a.capacity)
                    || self.earliest != a.start
                    || self.latest != a.end
                    || self
                        .check_at
                        .is_none_or(|t| t < calendar::MIN_TIME || t >= a.start || t % 60 != 0)
                {
                    return Err("请核对本人的活动、目标人数及开始前的检查时间".into());
                }
            }
            GoalKind::Participate => {
                if self.target.is_some()
                    || self.check_at.is_some()
                    || self.recurrence_days.is_some()
                {
                    return Err("参与目标只填写本人想参加的类型、时段和同行人数".into());
                }
                if let Some(id) = &self.activity_id {
                    if !activities.iter().any(|a| {
                        a.metadata
                            .as_ref()
                            .is_some_and(|m| &m.activity_id == id && !m.archived)
                    }) {
                        return Err("目标引用的活动尚未由宿主接入核验".into());
                    }
                }
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PrivateState {
    pub schema: u8,
    pub account: String,
    pub preferences: PersonalPreferences,
    pub goals: Vec<ActivityGoal>,
    #[serde(default)]
    pub recommendations: crate::proactive::RecommendationState,
    #[serde(default)]
    pub tasks: Vec<crate::assistance_tasks::AssistanceTask>,
    #[serde(default)]
    pub feedback: Vec<crate::intent_feedback::IntentFeedback>,
    #[serde(default)]
    pub analyses: std::collections::BTreeMap<String, Vec<String>>,
}
pub struct IntentStore {
    db: Connection,
    actor: String,
    observed: std::cell::RefCell<Option<String>>,
}
impl IntentStore {
    pub fn open(root: impl AsRef<Path>, actor: &str) -> Result<Self> {
        if !account_valid(actor) {
            return Err("个人资料必须绑定宿主核验的账号".into());
        }
        let dir = root.as_ref().join("intentions");
        std::fs::create_dir_all(&dir).map_err(|_| "个人资料目录不可用")?;
        let hash = hex::encode(Sha256::digest(actor.as_bytes()));
        let db = Connection::open(dir.join(format!("{hash}.db"))).map_err(|_| "个人资料不可用")?;
        db.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL; CREATE TABLE IF NOT EXISTS private_state(id INTEGER PRIMARY KEY CHECK(id=1),account TEXT NOT NULL,body TEXT NOT NULL);")
            .map_err(|_| "个人资料不可用")?;
        let s = Self {
            db,
            actor: actor.into(),
            observed: Default::default(),
        };
        s.load()?;
        Ok(s)
    }
    pub fn load(&self) -> Result<PrivateState> {
        let raw: Option<(String, String)> = self
            .db
            .query_row(
                "SELECT account,body FROM private_state WHERE id=1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(|_| "个人资料读取失败")?;
        let Some((account, raw)) = raw else {
            *self.observed.borrow_mut() = None;
            return Ok(PrivateState {
                schema: 1,
                account: self.actor.clone(),
                preferences: Default::default(),
                goals: vec![],
                recommendations: Default::default(),
                tasks: vec![],
                feedback: vec![],
                analyses: Default::default(),
            });
        };
        if account != self.actor || raw.len() > 4 * 1024 * 1024 {
            return Err("个人资料账号或大小不合法，停止写入".into());
        }
        let state: PrivateState =
            serde_json::from_str(&raw).map_err(|_| "个人资料损坏，请保留原目录并恢复备份")?;
        if state.account != self.actor || state.schema != 1 || state.goals.len() > 100 {
            return Err("个人资料来源或版本不匹配，停止写入".into());
        }
        state.preferences.validate()?;
        *self.observed.borrow_mut() = Some(hex::encode(Sha256::digest(raw.as_bytes())));
        Ok(state)
    }
    pub(super) fn save(&mut self, state: &PrivateState) -> Result<()> {
        if state.account != self.actor {
            return Err("账号变化，旧个人资料写入已取消".into());
        }
        let raw = serde_json::to_string(state).map_err(|_| "个人资料格式不合法")?;
        if raw.len() > 4 * 1024 * 1024 {
            return Err("个人资料已达容量，请先归档旧任务与反馈")?;
        }
        let tx = self.db.transaction().map_err(|_| "个人资料事务不可用")?;
        let current: Option<String> = tx
            .query_row("SELECT body FROM private_state WHERE id=1", [], |r| {
                r.get(0)
            })
            .optional()
            .map_err(|_| "个人资料读取失败")?;
        if current
            .as_ref()
            .map(|s| hex::encode(Sha256::digest(s.as_bytes())))
            != *self.observed.borrow()
        {
            return Err("个人资料已经变化，请重新读取后保存")?;
        }
        tx.execute("INSERT INTO private_state VALUES(1,?1,?2) ON CONFLICT(id) DO UPDATE SET body=excluded.body WHERE account=excluded.account",
            params![self.actor,raw]).map_err(|_| "个人资料保存失败")?;
        tx.commit().map_err(|_| "个人资料保存失败")?;
        *self.observed.borrow_mut() = Some(hex::encode(Sha256::digest(raw.as_bytes())));
        Ok(())
    }
    pub fn save_goal(
        &mut self,
        id: Option<&str>,
        input: GoalInput,
        activities: &[Activity],
        clock: u64,
    ) -> Result<ActivityGoal> {
        input.validate(&self.actor, activities, clock)?;
        let mut state = self.load()?;
        let field_sources = field_sources(&input, &state.preferences);
        let goal = if let Some(id) = id {
            let g = state
                .goals
                .iter_mut()
                .find(|g| g.id == id)
                .ok_or("目标已不存在，请重新选择")?;
            g.input = input;
            g.revision += 1;
            g.status = GoalStatus::Active;
            g.updated_at = clock;
            g.field_sources = field_sources;
            g.clone()
        } else {
            if state.goals.len() >= 100 {
                return Err("请先删除不需要的旧目标")?;
            }
            let g = ActivityGoal {
                id: action_receipts::new_id(),
                revision: 1,
                input,
                status: GoalStatus::Active,
                created_at: clock,
                updated_at: clock,
                field_sources,
            };
            state.goals.push(g.clone());
            g
        };
        self.save(&state)?;
        Ok(goal)
    }
    pub fn set_goal_status(&mut self, id: &str, status: GoalStatus, clock: u64) -> Result<()> {
        let mut state = self.load()?;
        let g = state
            .goals
            .iter_mut()
            .find(|g| g.id == id)
            .ok_or("目标不存在")?;
        g.status = status;
        g.revision += 1;
        g.updated_at = clock;
        self.save(&state)
    }
    pub fn delete_goal(&mut self, id: &str) -> Result<()> {
        let mut state = self.load()?;
        state.goals.retain(|g| g.id != id);
        self.save(&state)
    }
    pub fn save_preferences(
        &mut self,
        mut preferences: PersonalPreferences,
        clock: u64,
    ) -> Result<()> {
        preferences.validate()?;
        preferences.confirmed_at = clock;
        let mut state = self.load()?;
        state.preferences = preferences;
        self.save(&state)
    }
    pub fn reset_preferences(&mut self) -> Result<()> {
        let mut state = self.load()?;
        state.preferences = Default::default();
        state.recommendations.disabled.clear();
        self.save(&state)
    }
    pub fn confirm_suggestion_sources(
        &mut self,
        id: &str,
        fields: &[String],
    ) -> Result<ActivityGoal> {
        let mut state = self.load()?;
        let g = state
            .goals
            .iter_mut()
            .find(|g| g.id == id)
            .ok_or("目标不存在")?;
        for field in fields {
            if !matches!(
                field.as_str(),
                "title" | "template" | "earliest" | "latest" | "group" | "target" | "check_at"
            ) {
                return Err("建议来源字段不合法".into());
            }
            if g.field_sources.get(field) != Some(&FieldSource::ConfirmedPreference) {
                g.field_sources
                    .insert(field.clone(), FieldSource::ConfirmedSuggestion);
            }
        }
        let result = g.clone();
        self.save(&state)?;
        Ok(result)
    }
    pub fn cache_analysis(&mut self, fingerprint: String, ids: Vec<String>) -> Result<()> {
        if fingerprint.len() != 64
            || !fingerprint.bytes().all(|b| b.is_ascii_hexdigit())
            || ids.len() > 3
            || ids
                .iter()
                .any(|s| !matches!(s.as_str(), "reason" | "evidence" | "deadline"))
        {
            return Err("主动分析格式不合法")?;
        }
        let mut s = self.load()?;
        if s.analyses.len() >= 1000 {
            let active: std::collections::BTreeSet<_> = s
                .recommendations
                .cards
                .iter()
                .map(|c| c.fingerprint.clone())
                .collect();
            s.analyses.retain(|key, _| active.contains(key));
        }
        s.analyses.insert(fingerprint, ids);
        self.save(&s)
    }
}

pub fn template_name(template: &str) -> &str {
    match template {
        "badminton" => "羽毛球",
        "boardgame" => "桌游",
        "reading" => "读书会",
        "any" => "不限类型",
        _ => "自定义活动",
    }
}
pub fn goal_text(g: &ActivityGoal) -> String {
    let details = match g.input.kind {
        GoalKind::Organize => format!(
            "目标确认 {} 人 · 检查 {}",
            g.input.target.unwrap_or(0),
            g.input.check_at.map(calendar::display).unwrap_or_default()
        ),
        GoalKind::Participate => format!(
            "包括本人 {} 人{}",
            g.input.group,
            g.input
                .availability
                .as_ref()
                .map(|s| format!(" · {}", s.description()))
                .unwrap_or_default()
        ),
    };
    format!(
        "{} · {}\n{}至{} · {}\n来源：本人已确认；各字段来源可在编辑页查看 · 版本{} · {}",
        g.input.title,
        template_name(&g.input.template),
        calendar::display(g.input.earliest),
        calendar::display(g.input.latest),
        details,
        g.revision,
        match g.status {
            GoalStatus::Active => "持续关注",
            GoalStatus::Paused => "已暂停",
            GoalStatus::Completed => "已完成",
        }
    )
}
