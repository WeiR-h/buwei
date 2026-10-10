//! Deterministic recommendations over verified BuWei facts; no execution authority.
use crate::{Activity, Delivery, PersonStatus, Reply, Result, assistance::*};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CardKind {
    Shortfall,
    NoCandidate,
    AutomationPaused,
    Opportunity,
    Conflict,
    Invitation,
    RegistrationChanged,
    NextActivity,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SuggestedAction {
    Share,
    Register,
    ReviewInvitation,
    ReviewConflict,
    Renew,
    Reconcile,
    NextDraft,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AssistanceCard {
    pub id: String,
    pub kind: CardKind,
    pub goal_id: Option<String>,
    pub goal_revision: u64,
    pub activity_id: String,
    pub room: String,
    pub title: String,
    pub reason: String,
    pub evidence: String,
    pub action: SuggestedAction,
    pub observed_at: u64,
    pub expires_at: u64,
    pub fingerprint: String,
    #[serde(default)]
    pub registration_sequence: Option<u64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VerifiedFacts {
    pub activity: Activity,
    pub observed_at: u64,
    pub complete: bool,
    pub accessible: bool,
    pub automation_pause: Option<String>,
    pub has_pending_operation: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct CardControl {
    pub id: String,
    pub dismissed_fingerprint: Option<String>,
    pub snooze_until: u64,
    pub notified: bool,
    #[serde(default)]
    pub notified_fingerprint: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct RecommendationState {
    pub cards: Vec<AssistanceCard>,
    pub controls: Vec<CardControl>,
    pub disabled: Vec<CardKind>,
    pub notification_day: u64,
    pub ordinary_notifications: u8,
}
fn make(
    f: &VerifiedFacts,
    kind: CardKind,
    goal: Option<&ActivityGoal>,
    title: &str,
    reason: String,
    action: SuggestedAction,
    clock: u64,
    deadline: u64,
) -> AssistanceCard {
    let a = &f.activity;
    let id = &a.metadata.as_ref().unwrap().activity_id;
    let goal_id = goal.map(|g| g.id.clone());
    let revision = goal.map_or(0, |g| g.revision);
    let key = format!("{kind:?}:{id}:{}", goal_id.as_deref().unwrap_or("account"));
    let evidence = format!(
        "活动 {} · 已确认 {}/{} 人 · 保留 {} 人 · 活动版本 {} · 服务端记录由宿主核验",
        a.title,
        a.confirmed(),
        a.capacity,
        a.held(),
        a.revision
    );
    let fingerprint = hex::encode(Sha256::digest(format!(
        "{key}|{revision}|{reason}|{evidence}"
    )));
    AssistanceCard {
        id: key,
        kind,
        goal_id,
        goal_revision: revision,
        activity_id: id.clone(),
        room: a.room.clone(),
        title: title.into(),
        reason,
        evidence,
        action,
        observed_at: f.observed_at,
        expires_at: deadline.min(clock + 600),
        fingerprint,
        registration_sequence: None,
    }
}
pub fn evaluate(
    actor: &str,
    state: &PrivateState,
    facts: &[VerifiedFacts],
    clock: u64,
) -> Vec<AssistanceCard> {
    if state.account != actor {
        return vec![];
    }
    let mut result = Vec::new();
    // A failed sync must itself remain actionable. It is not evidence for
    // capacity or invitations, so only offer recovery/renewal in this branch.
    for f in facts.iter().filter(|f| {
        f.accessible
            && f.activity.owner == actor
            && f.activity.metadata.as_ref().is_some_and(|m| !m.archived)
            && f.activity.start > clock
            && f.automation_pause.is_some()
    }) {
        let mut card = make(
            f,
            CardKind::AutomationPaused,
            None,
            "自动补位需要处理",
            f.automation_pause.clone().unwrap(),
            if f.has_pending_operation {
                SuggestedAction::Reconcile
            } else {
                SuggestedAction::Renew
            },
            clock,
            f.activity.start,
        );
        card.evidence = format!(
            "来源：本人宿主的同步与授权状态 · 上次核验 {}；重新核实前不推进名额",
            crate::calendar::display(f.observed_at)
        );
        result.push(card);
    }
    for f in facts
        .iter()
        .filter(|f| f.accessible && f.activity.owner == actor && f.activity.metadata.is_some())
    {
        for g in state.goals.iter().filter(|g| {
            g.status == GoalStatus::Active
                && g.input.kind == GoalKind::Organize
                && g.input.activity_id.as_deref()
                    == f.activity.metadata.as_ref().map(|m| m.activity_id.as_str())
        }) {
            let Some(days) = g.input.recurrence_days else {
                continue;
            };
            let period = days as u64 * 86400;
            if period == 0 {
                continue;
            }
            let next =
                f.activity.start + period * (clock.saturating_sub(f.activity.start) / period + 1);
            let prepare = next.saturating_sub(g.input.preparation_hours as u64 * 3600);
            if clock >= prepare && next < crate::calendar::MAX_TIME {
                let mut c = make(
                    f,
                    CardKind::NextActivity,
                    Some(g),
                    "该筹备下一场活动了",
                    format!(
                        "按你确认的 {} 天周期，先准备 {} 的活动草稿；日期、地点和人数仍需重新确认，成员重新报名。",
                        days,
                        crate::calendar::display(next)
                    ),
                    SuggestedAction::NextDraft,
                    clock,
                    next,
                );
                c.id = format!("next:{}:{next}", g.id);
                if state.tasks.iter().any(|t| {
                    t.card_id == c.id
                        && t.action == SuggestedAction::NextDraft
                        && t.status == crate::assistance_tasks::TaskStatus::Completed
                }) {
                    continue;
                }
                c.observed_at = g.updated_at;
                c.evidence = format!(
                    "来源：本人确认的周期与原活动设置 · 目标版本 {} · 不推断实际到场",
                    g.revision
                );
                result.push(c);
            }
        }
    }
    let facts: Vec<_> = facts
        .iter()
        .filter(|f| {
            f.complete
                && f.accessible
                && f.observed_at <= clock
                && clock - f.observed_at <= 120
                && f.activity.metadata.as_ref().is_some_and(|m| !m.archived)
        })
        .collect();
    for f in &facts {
        let a = &f.activity;
        let id = &a.metadata.as_ref().unwrap().activity_id;
        if a.end <= clock {
            continue;
        }
        if a.owner == actor {
            for g in state.goals.iter().filter(|g| {
                g.status == GoalStatus::Active
                    && g.input.kind == GoalKind::Organize
                    && g.input.activity_id.as_deref() == Some(id)
            }) {
                if g.input.check_at.is_some_and(|t| t <= clock)
                    && a.start > clock
                    && g.input.target.is_some_and(|n| a.confirmed() < n as usize)
                {
                    result.push(make(
                        f,
                        CardKind::Shortfall,
                        Some(g),
                        "活动人数还没齐",
                        format!(
                            "已到你设置的检查时间，距离目标还差 {} 人。",
                            g.input.target.unwrap() as usize - a.confirmed()
                        ),
                        SuggestedAction::Share,
                        clock,
                        a.start,
                    ));
                }
            }
            if a.start > clock
                && a.free() > 0
                && a.people.iter().any(|p| p.status == PersonStatus::Waiting)
                && a.candidate().is_none()
            {
                result.push(make(
                    f,
                    CardKind::NoCandidate,
                    None,
                    "有名额，但暂时无法整组补位",
                    "候补的时间或同行人数不满足条件，原排位保留。可以选择联系人分享活动。".into(),
                    SuggestedAction::Share,
                    clock,
                    a.start,
                ));
            }
        } else {
            let person = a.people.iter().find(|p| p.account == actor);
            if let Some(i) = a.invitations.iter().rev().find(|i| {
                i.recipient == actor
                    && i.reply == Reply::Pending
                    && i.delivery == Delivery::Delivered
                    && i.until > clock
            }) {
                let mut c = make(
                    f,
                    CardKind::Invitation,
                    None,
                    "你有一份待回复邀请",
                    format!(
                        "名额已为你保留，请在 {} 前亲自回复。",
                        crate::calendar::display(i.until)
                    ),
                    SuggestedAction::ReviewInvitation,
                    clock,
                    i.until,
                );
                c.id = format!("invitation:{}", i.operation_id);
                result.push(c);
            }
            if let Some(p) = person.filter(|p| p.status != PersonStatus::Waiting) {
                result.push(make(
                    f,
                    CardKind::RegistrationChanged,
                    None,
                    "我的报名有了结果",
                    match p.status {
                        PersonStatus::Confirmed => "组织者已核验接受，报名已确认。",
                        PersonStatus::Cancelled => "本人取消已核验，名额已释放。",
                        PersonStatus::Declined => "本人拒绝已核验。",
                        PersonStatus::Expired => "这次邀请已过期，可查看活动并重新报名。",
                        _ => "",
                    }
                    .into(),
                    SuggestedAction::ReviewInvitation,
                    clock,
                    a.end,
                ));
            }
            for g in state
                .goals
                .iter()
                .filter(|g| g.status == GoalStatus::Active && g.input.kind == GoalKind::Participate)
            {
                if g.input.activity_id.as_deref().is_some_and(|x| x != id)
                    || g.input.template != "any"
                        && g.input.template != a.metadata.as_ref().unwrap().template
                    || g.input.earliest > a.start
                    || g.input.latest < a.end
                    || a.start <= clock
                    || a.free() < g.input.group as usize
                    || person.is_some_and(|p| {
                        matches!(p.status, PersonStatus::Waiting | PersonStatus::Confirmed)
                    })
                {
                    continue;
                }
                result.push(make(
                    f,
                    CardKind::Opportunity,
                    Some(g),
                    "发现适合你的活动",
                    format!(
                        "符合本次目标的类型和时段，当前可容纳你们 {} 人。报名信息将先由本人确认。",
                        g.input.group
                    ),
                    SuggestedAction::Register,
                    clock,
                    a.start,
                ));
            }
        }
    }
    for (n, left) in facts.iter().enumerate() {
        if !left
            .activity
            .people
            .iter()
            .any(|p| p.account == actor && p.status == PersonStatus::Confirmed)
        {
            continue;
        }
        for right in facts.iter().skip(n + 1) {
            if right
                .activity
                .people
                .iter()
                .any(|p| p.account == actor && p.status == PersonStatus::Confirmed)
                && left.activity.start < right.activity.end
                && right.activity.start < left.activity.end
                && left.activity.end > clock
                && right.activity.end > clock
            {
                let mut card = make(
                    left,
                    CardKind::Conflict,
                    None,
                    "两场已确认活动时间重叠",
                    format!(
                        "{} 与 {} 的时段重叠，请选择保留哪场；取消仍由本人确认。",
                        left.activity.title, right.activity.title
                    ),
                    SuggestedAction::ReviewConflict,
                    clock,
                    left.activity
                        .start
                        .min(right.activity.start)
                        .max(clock + 60),
                );
                let mut ids = [
                    left.activity.metadata.as_ref().unwrap().activity_id.clone(),
                    right
                        .activity
                        .metadata
                        .as_ref()
                        .unwrap()
                        .activity_id
                        .clone(),
                ];
                ids.sort();
                card.id = format!("conflict:{}:{}", ids[0], ids[1]);
                result.push(card);
            }
        }
    }
    for c in &mut result {
        if let Some(f) = facts.iter().find(|f| f.activity.room == c.room) {
            c.registration_sequence = Some(
                f.activity
                    .people
                    .iter()
                    .find(|p| p.account == actor)
                    .map_or(0, |p| p.joined),
            );
        }
    }
    result
}
impl IntentStore {
    pub fn refresh_cards(
        &mut self,
        facts: &[VerifiedFacts],
        clock: u64,
    ) -> Result<Vec<AssistanceCard>> {
        let mut state = self.load()?;
        let cards = evaluate(&state.account, &state, facts, clock);
        state.recommendations.cards = cards;
        self.save(&state)?;
        self.visible_cards(clock)
    }
    pub fn visible_cards(&self, clock: u64) -> Result<Vec<AssistanceCard>> {
        let state = self.load()?;
        Ok(state
            .recommendations
            .cards
            .iter()
            .filter(|c| {
                c.expires_at > clock
                    && !state.recommendations.disabled.contains(&c.kind)
                    && !state.recommendations.controls.iter().any(|x| {
                        x.id == c.id
                            && (x.snooze_until > clock
                                || x.dismissed_fingerprint.as_ref() == Some(&c.fingerprint))
                    })
            })
            .cloned()
            .collect())
    }
    pub fn control_card(
        &mut self,
        id: &str,
        snooze_until: Option<u64>,
        disable_kind: bool,
        clock: u64,
    ) -> Result<()> {
        let mut s = self.load()?;
        let card = s
            .recommendations
            .cards
            .iter()
            .find(|c| c.id == id && c.expires_at > clock)
            .cloned()
            .ok_or("建议已更新，请刷新后核对")?;
        if disable_kind && !s.recommendations.disabled.contains(&card.kind) {
            s.recommendations.disabled.push(card.kind);
        }
        if let Some(until) = snooze_until {
            if until <= clock || until > clock + 86400 {
                return Err("稍后提醒需在一天内".into());
            }
        }
        if !s.recommendations.controls.iter().any(|c| c.id == id) {
            s.recommendations.controls.push(CardControl {
                id: id.into(),
                ..Default::default()
            });
        }
        let ctl = s
            .recommendations
            .controls
            .iter_mut()
            .find(|c| c.id == id)
            .unwrap();
        ctl.snooze_until = snooze_until.unwrap_or(0);
        if snooze_until.is_some() {
            ctl.notified = false;
            ctl.notified_fingerprint = None;
            ctl.dismissed_fingerprint = None;
        }
        if snooze_until.is_none() {
            ctl.dismissed_fingerprint = Some(card.fingerprint);
        }
        self.save(&s)
    }
    /// Persist the reminder claim before handing it to the desktop notifier.
    pub fn claim_notifications(&mut self, clock: u64) -> Result<Vec<AssistanceCard>> {
        let cards = self.visible_cards(clock)?;
        let mut s = self.load()?;
        if !s.preferences.reminders || s.preferences.quiet(clock) {
            return Ok(vec![]);
        }
        let day = (clock + 8 * 3600) / 86400;
        if s.recommendations.notification_day != day {
            s.recommendations.notification_day = day;
            s.recommendations.ordinary_notifications = 0;
        }
        let mut claimed = vec![];
        for c in cards {
            if !s.recommendations.controls.iter().any(|x| x.id == c.id) {
                s.recommendations.controls.push(CardControl {
                    id: c.id.clone(),
                    ..Default::default()
                });
            }
            let ctl = s
                .recommendations
                .controls
                .iter_mut()
                .find(|x| x.id == c.id)
                .unwrap();
            if (if c.kind == CardKind::Invitation {
                ctl.notified
            } else {
                ctl.notified_fingerprint.as_ref() == Some(&c.fingerprint)
            }) || c.kind != CardKind::Invitation && s.recommendations.ordinary_notifications >= 3
            {
                continue;
            }
            ctl.notified = true;
            ctl.notified_fingerprint = Some(c.fingerprint.clone());
            if c.kind != CardKind::Invitation {
                s.recommendations.ordinary_notifications += 1;
            }
            claimed.push(c);
        }
        self.save(&s)?;
        Ok(claimed)
    }
}
