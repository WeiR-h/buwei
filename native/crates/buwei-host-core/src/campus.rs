//! Public campus facts and deterministic registration rules. No student records.
use crate::{Preferences, Result, calendar};
use serde::{Deserialize, Serialize};

pub const FIELDS: [&str; 8] = [
    "campus.campus", "campus.audience", "campus.topic", "campus.guests",
    "campus.requirements", "campus.signup_deadline", "campus.team_size", "campus.role",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntryRole { Audience, Performer }
impl EntryRole {
    pub fn name(self) -> &'static str { match self { Self::Audience => "观众报名", Self::Performer => "演出招募" } }
    pub fn parse(value: &str) -> Result<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "audience" | "观众" | "观众报名" => Ok(Self::Audience),
            "performer" | "演出招募" | "演出者" => Ok(Self::Performer),
            _ => Err("请选择观众报名或演出招募".into()),
        }
    }
}

pub fn supported(kind: &str) -> bool {
    matches!(kind, "lecture" | "competition" | "movie" | "music_festival" | "club")
}
pub fn dedicated(kind: &str) -> bool { supported(kind) && kind != "movie" }

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CampusDetails {
    pub campus: String,
    pub audience: String,
    pub topic: String,
    pub guests: String,
    pub requirements: String,
    /// Beijing wall time, kept empty until the organizer explicitly supplies it.
    pub signup_deadline: String,
    /// Competition seats count people, including every member of a whole team.
    pub team_size: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<EntryRole>,
}

pub struct Playbook {
    pub topic: &'static str,
    pub guests: &'static str,
    pub requirements: &'static str,
    pub checklist: &'static str,
}
pub fn playbook(kind: &str) -> Playbook {
    match kind {
        "competition" => Playbook {
            topic: "竞赛项目与主题", guests: "主办单位 / 指导老师（选填）",
            requirements: "赛制、提交方式、参赛要求；人数在下方单独设置",
            checklist: "策划检查：个人赛或团队赛 → 报名截止 → 比赛流程与提交方式 → 评审与结果通知安排。名额按实际人数计算，整队递补。",
        },
        "movie" => Playbook {
            topic: "影片名称与版本", guests: "导赏 / 映后交流人（选填）",
            requirements: "片长、放映安排、入场和映后交流说明",
            checklist: "策划检查：影片与片长 → 放映场地与设备 → 入场安排 → 映后交流。放映安排需组织者自行核实。",
        },
        "music_festival" => Playbook {
            topic: "演出主题 / 场次", guests: "已确认的演出嘉宾 / 节目（选填）",
            requirements: "观众报名或演出招募、入场时间、场地与天气安排",
            checklist: "策划检查：明确观众或演出招募 → 场次与入场时间 → 节目与设备 → 天气及秩序安排。请只填写已确认的嘉宾与节目。",
        },
        "club" => Playbook {
            topic: "社团方向与体验内容", guests: "组织社团 / 接待团队（选填）",
            requirements: "体验内容、准备事项与加入方式",
            checklist: "策划检查：面向人群 → 体验与交流流程 → 报名人数 → 后续加入方式。活动报名不代表已获社团录取。",
        },
        _ => Playbook {
            topic: "讲座主题", guests: "已确认的主讲人（选填）",
            requirements: "讲座内容、问答安排、入场与准备事项",
            checklist: "策划检查：主题与面向人群 → 主讲人与场地 → 开场、讲授与问答 → 材料与人数确认。学分、证书等须有明确依据。",
        },
    }
}
impl CampusDetails {
    pub fn validate(&self, kind: &str, start: Option<u64>) -> Result<()> {
        if !supported(kind) { return Err("该活动类型不使用校园专项信息".into()); }
        for (text, max) in [(&self.campus, 120), (&self.audience, 180), (&self.topic, 200),
                            (&self.guests, 200), (&self.requirements, 600)] {
            if text.chars().count() > max || text.chars().any(|c| c.is_control() && c != '\n') {
                return Err("校园信息过长或含有不可显示字符".into());
            }
        }
        if self.team_size.is_some_and(|n| kind != "competition" || !(1..=8).contains(&n)) {
            return Err("竞赛每份报名须为 1–8 人；其他校园活动不设置队伍人数".into());
        }
        if self.role.is_some() && kind != "music_festival" { return Err("参与角色只用于音乐节场次".into()); }
        if let Some(deadline) = self.deadline()? {
            if start.is_some_and(|s| deadline > s) {
                return Err("报名截止不能晚于活动开始，请分别核对两个时间".into());
            }
        }
        Ok(())
    }
    pub fn deadline(&self) -> Result<Option<u64>> {
        if self.signup_deadline.trim().is_empty() { Ok(None) }
        else { calendar::parse(&self.signup_deadline).map(Some) }
    }
    pub fn questions(&self, kind: &str) -> Vec<String> {
        if kind == "competition" && self.team_size.is_none() {
            vec!["竞赛是个人赛还是团队赛？请填写每份报名人数，个人赛为 1，团队赛为 2–8。".into()]
        } else if kind == "music_festival" && self.role.is_none() {
            vec!["本场音乐节是观众报名还是演出招募？请选择本场用途，分别管理名额。".into()]
        } else { vec![] }
    }
    pub fn check_signup(&self, kind: &str, preferences: &Preferences, clock: u64) -> Result<()> {
        if self.deadline()?.is_some_and(|deadline| clock >= deadline) {
            return Err("本场报名已截止；已有候补仍可递补，已送达邀请可在有效期内回复".into());
        }
        self.check_group(kind, preferences.group)
    }
    pub fn check_group(&self, kind: &str, group: u8) -> Result<()> {
        if kind == "competition" {
            if let Some(size) = self.team_size {
                if group != size { return Err(format!("本场竞赛每份报名须为 {size} 人，含本人且整组报名；请核对人数")); }
            }
        }
        Ok(())
    }
    pub fn set(&mut self, field: &str, value: &str) -> Result<()> {
        match field {
            "campus.campus" => self.campus = value.into(),
            "campus.audience" => self.audience = value.into(),
            "campus.topic" => self.topic = value.into(),
            "campus.guests" => self.guests = value.into(),
            "campus.requirements" => self.requirements = value.into(),
            "campus.signup_deadline" => self.signup_deadline = value.into(),
            "campus.team_size" => self.team_size = if value.trim().is_empty() { None }
                else { Some(value.parse().map_err(|_| "每份报名人数须为 1–8 的整数")?) },
            "campus.role" => self.role = if value.trim().is_empty() { None } else { Some(EntryRole::parse(value)?) },
            _ => return Err("没有这个校园策划字段".into()),
        }
        Ok(())
    }
    pub fn summary(&self, kind: &str) -> String {
        let p = playbook(kind);
        let mut lines = vec!["校园活动 · 组织者填写的公开信息".to_owned()];
        if let Some(role) = self.role { lines.push(format!("本场用途：{}；本场名额仅用于该用途", role.name())); }
        for (label, value) in [("学校 / 校区", &self.campus), ("面向人群", &self.audience),
                                (p.topic, &self.topic), (p.guests, &self.guests), ("参加须知", &self.requirements)] {
            if !value.trim().is_empty() { lines.push(format!("{label}：{value}")); }
        }
        if !self.signup_deadline.trim().is_empty() {
            lines.push(format!("报名截止：{} · 北京时间；截止后已有候补仍可递补", self.signup_deadline));
        }
        if let Some(size) = self.team_size { lines.push(format!("每份报名 {size} 人（含本人），整组确认与候补；总名额按人数计算")); }
        lines.join("\n")
    }
    pub fn listing_summary(&self) -> String {
        let short = |s: &str, max: usize| {
            let mut text: String = s.replace(['\r', '\n'], " ").chars().take(max).collect();
            if s.chars().count() > max { text.push('…'); }
            text
        };
        let mut fields = vec![];
        if let Some(role) = self.role { fields.push(role.name().to_owned()); }
        for (s,max) in [(&self.campus,18),(&self.topic,24),(&self.audience,16)] {
            if !s.trim().is_empty() { fields.push(short(s,max)); }
        }
        if !self.signup_deadline.is_empty() { fields.push(format!("报名截止 {}",self.signup_deadline)); }
        if let Some(size) = self.team_size { fields.push(format!("每份 {size} 人")); }
        if fields.is_empty() { "校园场次 · 打开查看安排".into() } else { fields.join(" · ") }
    }
    /// Copying an event never silently reuses a past deadline.
    pub fn for_next_event(&self) -> Self {
        Self { signup_deadline: String::new(), ..self.clone() }
    }
}

pub fn check_signup(activity: &crate::Activity, preferences: &Preferences, clock: u64) -> Result<()> {
    if let Some(m) = &activity.metadata {
        if let Some(c) = &m.campus { c.check_signup(&m.template, preferences, clock)?; }
    }
    Ok(())
}
