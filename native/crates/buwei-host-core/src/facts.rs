//! Published prose is assembled from verified facts. The model chooses fact IDs.
use crate::{Activity, PersonStatus, Result, calendar};
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Fact {
    pub id: String,
    pub text: String,
}
pub fn list(a: &Activity) -> Vec<Fact> {
    let mut facts = vec![
        Fact {
            id: "time".into(),
            text: format!(
                "活动时间：{} 至 {}。",
                calendar::display(a.start),
                calendar::display(a.end)
            ),
        },
        Fact {
            id: "capacity".into(),
            text: format!("活动共 {} 个名额。", a.capacity),
        },
        Fact {
            id: "accepted".into(),
            text: format!("组织者已核验 {} 个接受名额。", a.confirmed()),
        },
        Fact {
            id: "held".into(),
            text: format!("当前为候补保留 {} 个名额，等待本人回复。", a.held()),
        },
        Fact {
            id: "waiting".into(),
            text: format!(
                "当前有 {} 条候补报名。",
                a.people
                    .iter()
                    .filter(|p| p.status == PersonStatus::Waiting)
                    .count()
            ),
        },
    ];
    if let Some(m) = &a.metadata {
        facts.push(Fact {
            id: "location".into(),
            text: format!("活动地点：{}。", m.location),
        });
        facts.push(Fact {
            id: "description".into(),
            text: format!("活动说明：{}", m.description),
        });
    }
    if let Some(m) = &a.metadata {
        if m.archived {
            facts.push(Fact {
                id: "review".into(),
                text: format!(
                    "已归档活动：当前确认 {} 人，累计取消 {} 人，邀请接受 {} 次。",
                    a.confirmed(),
                    m.metrics.cancelled_people,
                    m.metrics.successful_invitations
                ),
            });
        }
    }
    facts
}
pub fn list_for(a: &Activity, actor: &str) -> Vec<Fact> {
    let mut facts = list(a);
    let text = if let Some(p) = a.people.iter().find(|p| p.account == actor) {
        let position = a
            .ordered_people()
            .iter()
            .position(|p| p.account == actor)
            .map(|n| n + 1)
            .unwrap_or(0);
        let held = a.invitations.iter().any(|i| {
            i.recipient == actor
                && i.reply == crate::Reply::Pending
                && i.delivery != crate::Delivery::Rejected
        });
        let status = if held {
            "名额为本人保留，等待本人回复"
        } else {
            match p.status {
                PersonStatus::Waiting => "候补中",
                PersonStatus::Confirmed => "报名已确认",
                PersonStatus::Declined => "本人已拒绝",
                PersonStatus::Cancelled => "本人已取消",
                PersonStatus::Expired => "邀请已过期",
            }
        };
        format!(
            "本人报名：{} 人，当前列表第 {} 位，{}。",
            p.preferences.group, position, status
        )
    } else {
        "本人尚未报名本场活动。".into()
    };
    facts.push(Fact {
        id: "my_signup".into(),
        text,
    });
    facts
}
fn render_facts(facts: Vec<Fact>, ids: &[String]) -> Result<String> {
    if ids.is_empty() || ids.len() > 7 {
        return Err("请先选择需要说明的活动事实".into());
    }
    let mut seen = std::collections::BTreeSet::new();
    let mut lines = vec![];
    for id in ids {
        if !seen.insert(id) {
            return Err("活动事实重复".into());
        }
        let fact = facts
            .iter()
            .find(|f| &f.id == id)
            .ok_or("回答引用了未核验的活动事实")?;
        lines.push(format!("{}〔依据：{}〕", fact.text, fact.id));
    }
    Ok(lines.join("\n\n"))
}
pub fn render(a: &Activity, ids: &[String]) -> Result<String> {
    render_facts(list(a), ids)
}
pub fn render_for(a: &Activity, actor: &str, ids: &[String]) -> Result<String> {
    render_facts(list_for(a, actor), ids)
}
