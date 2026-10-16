//! Account-private activity plans. A plan can never perform a Matrix action.
use crate::{Result, calendar};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, path::Path};

pub const BASIC_FIELDS: [&str; 7] = [
    "title",
    "template",
    "start",
    "end",
    "location",
    "capacity",
    "description",
];
pub fn plan_fields() -> Vec<&'static str> {
    BASIC_FIELDS.into_iter().chain(crate::campus::FIELDS).collect()
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActivityBasics {
    pub title: String,
    pub template: String,
    pub start: String,
    pub end: String,
    pub location: String,
    pub capacity: u8,
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub campus: Option<crate::campus::CampusDetails>,
}
impl ActivityBasics {
    pub fn validate_draft(&self) -> Result<()> {
        if let Some(c) = &self.campus {
            c.validate(&self.template, if self.start.is_empty() { None } else { Some(calendar::parse(&self.start)?) })?;
        }
        if self.title.chars().count() > 80
            || self.location.chars().count() > 200
            || self.description.len() > 4000
            || self.capacity > 30
            || (!self.template.is_empty() && !crate::activity_types::valid(&self.template))
        {
            return Err("活动草稿字段超出范围".into());
        }
        for s in [&self.start, &self.end] {
            if !s.is_empty() {
                calendar::parse(s)?;
            }
        }
        if !self.start.is_empty() && !self.end.is_empty() {
            let start = calendar::parse(&self.start)?;
            let end = calendar::parse(&self.end)?;
            if !calendar::interval(start, end) || end - start > 48 * 3600 {
                return Err("活动起止时间不合法，最长 48 小时".into());
            }
        }
        Ok(())
    }
    pub fn questions(&self) -> Vec<String> {
        let mut q = vec![];
        if self.title.trim().is_empty() {
            q.push("这场活动叫什么名字？".into());
        }
        if self.template.is_empty() {
            q.push("准备举办哪类活动？可从活动库选择，或使用自定义活动。".into());
        }
        match (self.start.is_empty(), self.end.is_empty()) {
            (true, true) => q.push("活动哪天几点开始、哪天几点结束？".into()),
            (true, false) => q.push("活动哪天几点开始？".into()),
            (false, true) => q.push("活动哪天几点结束？".into()),
            (false, false) => {}
        }
        if self.location.trim().is_empty() {
            q.push("活动在哪里举办？".into());
        }
        if self.capacity == 0 {
            q.push("准备提供几个参与名额（1–30）？".into());
        }
        if let Some(c) = &self.campus { q.extend(c.questions(&self.template)); }
        q
    }
    pub fn ready(&self, clock: u64) -> Result<()> {
        self.validate_draft()?;
        let questions = self.questions();
        if !questions.is_empty() { return Err(questions.join("\n")); }
        if calendar::parse(&self.start)? <= clock {
            return Err("请选择未来的活动开始时间".into());
        }
        if let Some(c) = &self.campus {
            if c.deadline()?.is_some_and(|d| d <= clock) { return Err("新活动的报名截止须在将来，请重新确认截止时间".into()); }
            if c.team_size.is_some_and(|n| n > self.capacity) { return Err("总名额不能少于一支完整队伍的人数".into()); }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FieldUpdate {
    pub field: String,
    pub value: String,
    /// Exact fragment of this turn's input; not an account or a permission.
    pub source: String,
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanProposal {
    pub updates: Vec<FieldUpdate>,
    pub agenda: Vec<String>,
    pub preparation: Vec<String>,
    pub recruitment: String,
    pub reminders: Vec<String>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanStatus {
    Draft,
    Creating,
    Created,
    Published,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActivityPlanDraft {
    pub id: String,
    pub account: String,
    pub revision: u64,
    pub goal: String,
    pub basics: ActivityBasics,
    pub agenda: Vec<String>,
    pub preparation: Vec<String>,
    pub recruitment: String,
    pub reminders: Vec<String>,
    pub confirmed_fields: BTreeSet<String>,
    pub turns: Vec<String>,
    pub updated_at: u64,
    pub status: PlanStatus,
    pub setup_id: Option<String>,
    pub activity_id: Option<String>,
    pub publication_operations: Vec<String>,
}
impl ActivityPlanDraft {
    pub fn from_template(account: &str, kind: &str, clock: u64) -> Result<Self> {
        let t = crate::activity_types::get(kind).ok_or("请选择活动库中的类型")?;
        let mut draft = Self::new(account, &format!("本人选用{}模板", t.name), clock)?;
        draft.basics = ActivityBasics {
            title: t.title.into(), template: t.id.into(), capacity: t.capacity,
            description: t.description.into(), ..Default::default()
        };
        if crate::campus::dedicated(kind) { draft.basics.campus = Some(Default::default()); }
        draft.agenda = t.agenda.iter().map(|s| s.to_string()).collect();
        draft.preparation = t.preparation.iter().map(|s| s.to_string()).collect();
        draft.reminders = vec!["建议在活动前确认人数与安排；提醒尚未启用。".into()];
        Ok(draft)
    }
    pub fn from_campus_template(account: &str, kind: &str, clock: u64) -> Result<Self> {
        if !crate::campus::supported(kind) { return Err("请选择校园活动模板".into()); }
        let mut draft = Self::from_template(account, kind, clock)?;
        draft.basics.campus = Some(Default::default());
        if kind == "movie" { draft.basics.title = "校园观影与映后交流".into(); }
        Ok(draft)
    }
    pub fn new(account: &str, text: &str, clock: u64) -> Result<Self> {
        if !crate::account_valid(account) || text.trim().is_empty() || text.len() > 4000 {
            return Err("策划需求或本人账号不合法".into());
        }
        Ok(Self {
            id: action_receipts::new_id(),
            account: account.into(),
            revision: 1,
            goal: text.into(),
            basics: ActivityBasics::default(),
            agenda: vec![],
            preparation: vec![],
            recruitment: String::new(),
            reminders: vec![],
            confirmed_fields: Default::default(),
            turns: vec![],
            updated_at: clock,
            status: PlanStatus::Draft,
            setup_id: None,
            activity_id: None,
            publication_operations: vec![],
        })
    }
    pub fn merge(&mut self, text: &str, p: PlanProposal, clock: u64) -> Result<()> {
        if self.status != PlanStatus::Draft
            || text.trim().is_empty()
            || text.len() > 4000
            || self.turns.len() >= 12
        {
            return Err("请新建策划稿或缩短需求；已创建的活动不能通过草稿覆盖".into());
        }
        let mut next = self.clone();
        let mut seen = BTreeSet::new();
        for u in &p.updates {
            if !plan_fields().contains(&u.field.as_str())
                || !seen.insert(u.field.clone())
                || u.source.trim().is_empty()
                || !text.contains(&u.source)
                || u.source.len() > 1200
            {
                return Err("修改字段缺少本次原话依据，请手动核对".into());
            }
        }
        for u in p.updates.iter().filter(|u| !u.field.starts_with("campus.")) {
            match u.field.as_str() {
                "title" => next.basics.title = u.value.clone(),
                "template" => {
                    if u.value != "custom" && !crate::activity_types::evidence_supports(&u.value, &u.source) {
                        return Err("活动类型与本次原话不符，请核对类型".into());
                    }
                    next.basics.template = u.value.clone();
                }
                "start" => next.basics.start = u.value.clone(),
                "end" => next.basics.end = u.value.clone(),
                "location" => next.basics.location = u.value.clone(),
                "capacity" => next.basics.capacity = u.value.parse().map_err(|_| "容量不合法")?,
                "description" => next.basics.description = u.value.clone(),
                _ => unreachable!(),
            }
            next.confirmed_fields.remove(&u.field);
        }
        if crate::campus::dedicated(&next.basics.template) && next.basics.campus.is_none() {
            next.basics.campus = Some(Default::default());
        }
        for u in p.updates.iter().filter(|u| u.field.starts_with("campus.")) {
            if !["campus.signup_deadline", "campus.team_size", "campus.role"].contains(&u.field.as_str())
                && !u.value.is_empty() && !u.source.contains(&u.value) {
                return Err("校园事实须直接引用本人原话，请核对学校、主题和嘉宾".into());
            }
            if u.field == "campus.role" && !u.value.is_empty() {
                let source = u.source.to_ascii_lowercase();
                let valid = match crate::campus::EntryRole::parse(&u.value)? {
                    crate::campus::EntryRole::Audience => ["观众", "audience"].iter().any(|s| source.contains(s)),
                    crate::campus::EntryRole::Performer => ["演出招募", "表演", "演出者", "乐手", "performer"].iter().any(|s| source.contains(s)),
                };
                if !valid { return Err("音乐节用途缺少本次原话依据，请选择观众报名或演出招募".into()); }
            }
            next.basics.campus.get_or_insert_with(Default::default).set(&u.field, &u.value)?;
            next.confirmed_fields.clear();
        }
        next.basics.validate_draft()?;
        for lines in [&p.agenda, &p.preparation, &p.reminders] {
            if lines.len() > 8 || lines.iter().any(|s| s.len() > 600) {
                return Err("策划建议超出范围".into());
            }
        }
        if p.recruitment.len() > 1600 {
            return Err("招募文案超出范围".into());
        }
        next.agenda = p.agenda;
        next.preparation = p.preparation;
        next.recruitment = p.recruitment;
        next.reminders = p.reminders;
        next.turns.push(text.into());
        next.updated_at = clock;
        *self = next;
        Ok(())
    }
    pub fn confirm_basics(&mut self, basics: ActivityBasics, clock: u64) -> Result<()> {
        if self.status != PlanStatus::Draft {
            return Err("本策划已进入创建流程，请核实原创建结果".into());
        }
        basics.ready(clock)?;
        self.basics = basics;
        self.confirmed_fields = BASIC_FIELDS.iter().map(|s| s.to_string()).collect();
        self.updated_at = clock;
        Ok(())
    }
    pub fn creation_digest(&self, clock: u64) -> Result<String> {
        self.basics.ready(clock)?;
        if self.status != PlanStatus::Draft
            || BASIC_FIELDS
                .iter()
                .any(|f| !self.confirmed_fields.contains(*f))
        {
            return Err("请先核对并保存基础信息".into());
        }
        Ok(hex::encode(Sha256::digest(
            serde_json::to_vec(self).map_err(|_| "草稿不可核对")?,
        )))
    }
    pub fn text(&self) -> String {
        let b = &self.basics;
        let lines = |v: &[String]| {
            if v.is_empty() {
                "待补充".into()
            } else {
                v.iter()
                    .map(|s| format!("• {s}"))
                    .collect::<Vec<_>>()
                    .join("\n")
            }
        };
        let mut text = format!(
            "进度：{}\n原需求：{}\n基础信息（请本人核对）：{} · {} · {} 人\n{} 至 {} · 地点：{}\n{}\n\n流程建议：\n{}\n准备建议：\n{}\n招募文案建议：\n{}\n提醒建议（尚未启用）：\n{}\n{}",
            match self.status {
                PlanStatus::Draft => "策划待本人确认",
                PlanStatus::Creating => "创建结果待核实，沿原编号恢复",
                PlanStatus::Created => "活动已创建，等待本人确认分享",
                PlanStatus::Published => "活动已发布，继续核实报名结果",
            },
            self.goal,
            b.title,
            crate::activity_types::name(&b.template),
            b.capacity,
            b.start,
            b.end,
            b.location,
            b.description,
            lines(&self.agenda),
            lines(&self.preparation),
            self.recruitment,
            lines(&self.reminders),
            self.basics.questions().join("\n")
        );
        if let Some(c) = &b.campus {
            text.push_str(&format!("\n\n{}\n{}", c.summary(&b.template), crate::campus::playbook(&b.template).checklist));
        }
        text
    }
}

pub struct PlanStore {
    db: Connection,
    account: String,
}
impl PlanStore {
    pub fn open(path: impl AsRef<Path>, account: &str) -> Result<Self> {
        if !crate::account_valid(account) {
            return Err("本人账号未核验".into());
        }
        let db = Connection::open(path).map_err(|_| "策划资料不可读取")?;
        db.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL; CREATE TABLE IF NOT EXISTS plans(id TEXT PRIMARY KEY,account TEXT NOT NULL,revision INTEGER NOT NULL,body TEXT NOT NULL); CREATE TABLE IF NOT EXISTS plan_calls(account TEXT NOT NULL,call_id TEXT NOT NULL,draft_id TEXT NOT NULL,PRIMARY KEY(account,call_id));").map_err(|_|"策划资料不可初始化")?;
        Ok(Self {
            db,
            account: account.into(),
        })
    }
    pub fn load(&self, id: &str) -> Result<ActivityPlanDraft> {
        let body: Option<String> = self
            .db
            .query_row(
                "SELECT body FROM plans WHERE id=?1 AND account=?2",
                params![id, self.account],
                |r| r.get(0),
            )
            .optional()
            .map_err(|_| "策划资料不可读取")?;
        let p: ActivityPlanDraft = serde_json::from_str(&body.ok_or("本账号没有这份策划稿")?)
            .map_err(|_| "策划资料损坏")?;
        if p.account != self.account || p.id != id {
            return Err("策划来源不一致".into());
        }
        Ok(p)
    }
    pub fn list(&self) -> Result<Vec<ActivityPlanDraft>> {
        let mut q = self
            .db
            .prepare("SELECT id FROM plans WHERE account=?1 ORDER BY rowid DESC LIMIT 50")
            .map_err(|_| "策划列表不可读取")?;
        let ids = q
            .query_map([&self.account], |r| r.get::<_, String>(0))
            .map_err(|_| "策划列表不可读取")?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|_| "策划列表不可读取")?;
        ids.iter().map(|id| self.load(id)).collect()
    }
    pub fn save(&self, p: &mut ActivityPlanDraft, expected: Option<u64>) -> Result<()> {
        if p.account != self.account
            || p.id.len() != 32
            || !p.id.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err("策划资料账号或编号不一致".into());
        }
        p.basics.validate_draft()?;
        if expected.is_some_and(|r| r >= i64::MAX as u64) {
            return Err("策划版本超出范围".into());
        }
        let revision = expected.map_or(1, |r| r + 1);
        let mut next = p.clone();
        next.revision = revision;
        let body = serde_json::to_string(&next).map_err(|_| "策划不可保存")?;
        let changed = if let Some(previous) = expected {
            self.db.execute(
                "UPDATE plans SET revision=?1,body=?2 WHERE id=?3 AND account=?4 AND revision=?5",
                params![revision as i64, body, p.id, self.account, previous as i64],
            )
        } else {
            self.db.execute(
                "INSERT INTO plans VALUES(?1,?2,?3,?4)",
                params![p.id, self.account, revision as i64, body],
            )
        }
        .map_err(|_| "策划保存失败，请保留原稿")?;
        if changed != 1 {
            return Err("策划已变化，请重新打开并核对".into());
        }
        *p = next;
        Ok(())
    }
    pub fn completed_call(&self, call: &str) -> Result<Option<ActivityPlanDraft>> {
        let id: Option<String> = self
            .db
            .query_row(
                "SELECT draft_id FROM plan_calls WHERE account=?1 AND call_id=?2",
                params![self.account, call],
                |r| r.get(0),
            )
            .optional()
            .map_err(|_| "工具回执不可读取")?;
        id.map(|id| self.load(&id)).transpose()
    }
    pub fn record_call(&self, call: &str, id: &str) -> Result<()> {
        self.load(id)?;
        self.db
            .execute(
                "INSERT INTO plan_calls VALUES(?1,?2,?3) ON CONFLICT(account,call_id) DO NOTHING",
                params![self.account, call, id],
            )
            .map_err(|_| "工具回执不可保存")?;
        Ok(())
    }
}
