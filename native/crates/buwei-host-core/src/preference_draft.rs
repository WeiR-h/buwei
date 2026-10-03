//! Incomplete model preferences stay drafts until the person supplies the gaps.
use crate::{Preferences, Result};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreferenceDraft {
    pub earliest: Option<u8>,
    pub latest: Option<u8>,
    pub group: Option<u8>,
    pub needs_clarification: bool,
    pub questions: Vec<String>,
    pub explanation: String,
}

fn safe_text(text: &str, limit: usize) -> bool {
    !text.trim().is_empty()
        && text.chars().count() <= limit
        && !text.contains('@')
        && !text.contains('!')
        && !text.to_ascii_lowercase().contains("http")
        && !text.chars().any(|c| c.is_control() && c != '\n')
}

impl PreferenceDraft {
    pub fn parse(value: &serde_json::Value) -> Result<Self> {
        let draft: Self =
            serde_json::from_value(value.clone()).map_err(|_| "模型需求草稿格式不符合约定")?;
        if draft.earliest.is_some_and(|n| n > 23)
            || draft.latest.is_some_and(|n| n == 0 || n > 24)
            || draft.group.is_some_and(|n| !(1..=8).contains(&n))
            || draft
                .earliest
                .zip(draft.latest)
                .is_some_and(|(a, b)| a >= b)
            || !safe_text(&draft.explanation, 240)
            || draft.questions.len() > 3
            || draft.questions.iter().any(|q| {
                !safe_text(q, 120)
                    || [
                        "账号",
                        "密码",
                        "密钥",
                        "身份证",
                        "手机号",
                        "电话",
                        "apikey",
                        "token",
                    ]
                    .iter()
                    .any(|s| q.to_ascii_lowercase().contains(s))
            })
        {
            return Err("模型时段、人数或追问不符合约定".into());
        }
        if draft.needs_clarification {
            if draft.questions.is_empty() {
                return Err("信息不完整时必须列出待补充的问题".into());
            }
        } else if draft.earliest.is_none()
            || draft.latest.is_none()
            || draft.group.is_none()
            || !draft.questions.is_empty()
        {
            return Err("信息不完整的草稿不能进入报名预览".into());
        }
        Ok(draft)
    }

    /// A model cannot silently round unsupported times, even with valid JSON.
    /// Rewriting the original requirement starts a new draft; an unsupported
    /// original request cannot be cleared by answering only the group question.
    pub fn guard_supported_input(&mut self, turns: &[String]) {
        if turns.iter().any(|s| unsupported_time(s)) {
            self.needs_clarification = true;
            self.earliest = None;
            self.latest = None;
            let question =
                "当前活动支持今天的整数小时。请修改原需求，写清今天几点到几点，例如 19–21 点。";
            if !self.questions.iter().any(|q| q == question) {
                self.questions.truncate(2);
                self.questions.push(question.into());
            }
        }
    }

    pub fn preferences(&self) -> Result<Preferences> {
        if self.needs_clarification {
            return Err("请先补齐问题，或手动填写今天的整数时段".into());
        }
        let p = Preferences {
            earliest: self.earliest.ok_or("开始时间尚未确认")? as u64,
            latest: self.latest.ok_or("结束时间尚未确认")? as u64,
            group: self.group.ok_or("同行人数尚未确认")?,
        };
        p.validate()?;
        Ok(p)
    }

    pub fn display(&self) -> String {
        let hour = |n: Option<u8>| {
            n.map(|n| format!("{n} 点"))
                .unwrap_or_else(|| "待补充".into())
        };
        let group = self
            .group
            .map(|n| format!("{n} 人"))
            .unwrap_or_else(|| "待补充".into());
        let questions = self
            .questions
            .iter()
            .enumerate()
            .map(|(n, q)| format!("{}. {q}", n + 1))
            .collect::<Vec<_>>()
            .join("\n");
        format!(
            "可用时间：{} 至 {} · 同行：{}\n{}\n{}",
            hour(self.earliest),
            hour(self.latest),
            group,
            self.explanation,
            if self.needs_clarification {
                format!("请补充：\n{questions}")
            } else {
                "信息已完整；核对后预览本人报名。".into()
            }
        )
    }
}

fn unsupported_time(text: &str) -> bool {
    if text.contains('点') && text.contains('分') {
        return true;
    }
    if [
        "半", "刻", "分钟", "明天", "后天", "下周", "下月", "跨天", "次日", "周一", "周二", "周三",
        "周四", "周五", "周六", "周日", "星期", "明晚",
    ]
    .iter()
    .any(|s| text.contains(s))
    {
        return true;
    }
    let chars: Vec<char> = text.chars().collect();
    for (n, c) in chars.iter().enumerate() {
        if (*c == ':' || *c == '：') && n > 0 && chars[n - 1].is_ascii_digit() {
            let minute: String = chars
                .iter()
                .skip(n + 1)
                .take_while(|c| c.is_ascii_digit())
                .collect();
            if !minute.is_empty() && minute.parse::<u8>().ok() != Some(0) {
                return true;
            }
        }
        if *c == '.'
            && n > 0
            && chars[n - 1].is_ascii_digit()
            && chars.get(n + 1).is_some_and(char::is_ascii_digit)
        {
            return true;
        }
    }
    false
}

pub fn dialogue_binding(requirement: &str, latest_answer: &str) -> String {
    if latest_answer.is_empty() {
        requirement.to_owned()
    } else {
        format!("{requirement}\n补充回答：\n{latest_answer}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn complete() -> serde_json::Value {
        json!({"earliest":19,"latest":21,"group":1,"needs_clarification":false,"questions":[],"explanation":"今晚 19 至 21 点，独自参加。"})
    }
    #[test]
    fn complete_draft_only_produces_typed_preferences() {
        assert_eq!(
            PreferenceDraft::parse(&complete())
                .unwrap()
                .preferences()
                .unwrap(),
            Preferences {
                earliest: 19,
                latest: 21,
                group: 1
            }
        );
    }
    #[test]
    fn partial_fields_are_visible_without_fabricating_times() {
        let v = json!({"earliest":null,"latest":null,"group":1,"needs_clarification":true,"questions":["今晚具体几点到几点有空？"],"explanation":"已知道独自参加，时间需要确认。"});
        let d = PreferenceDraft::parse(&v).unwrap();
        assert!(d.display().contains("待补充"));
        assert!(d.preferences().is_err());
    }
    #[test]
    fn incomplete_draft_cannot_claim_ready() {
        let mut v = complete();
        v["earliest"] = serde_json::Value::Null;
        assert!(PreferenceDraft::parse(&v).is_err());
    }
    #[test]
    fn malformed_or_injected_output_fails_closed() {
        for (field, value) in [
            ("group", json!(9)),
            ("latest", json!(18)),
            ("earliest", json!(-1)),
            ("execute", json!(true)),
            ("explanation", json!("https://example.test")),
        ] {
            let mut v = complete();
            v[field] = value;
            assert!(PreferenceDraft::parse(&v).is_err());
        }
    }
    #[test]
    fn clarification_requires_a_bounded_question() {
        let mut v = complete();
        v["needs_clarification"] = true.into();
        assert!(PreferenceDraft::parse(&v).is_err());
        v["questions"] = json!(["请提供 @someone:test 的账号"]);
        assert!(PreferenceDraft::parse(&v).is_err());
        v["questions"] = json!(["请提供登录密码才能报名"]);
        assert!(PreferenceDraft::parse(&v).is_err());
        v["questions"] = json!(["几点有空？", "几个人？", "今天吗？", "怎么联系？"]);
        assert!(PreferenceDraft::parse(&v).is_err());
    }
    #[test]
    fn fractional_and_other_day_inputs_cannot_be_rounded() {
        for text in [
            "今天19:30到21:30一人",
            "明天19–21点一人",
            "今晚七点半到九点",
            "今天19.5到21.5点",
            "今晚七点一刻到九点",
        ] {
            let mut d = PreferenceDraft::parse(&complete()).unwrap();
            d.guard_supported_input(&[text.into()]);
            assert!(d.needs_clarification);
            assert!(d.preferences().is_err());
            assert_eq!(d.earliest, None);
            assert_eq!(d.latest, None);
        }
    }
    #[test]
    fn integer_hour_and_complete_dialogue_remain_available() {
        let mut d = PreferenceDraft::parse(&complete()).unwrap();
        d.guard_supported_input(&["今天19:00到21:00".into(), "我一个人参加".into()]);
        assert!(d.preferences().is_ok());
    }
    #[test]
    fn changing_latest_answer_changes_confirmation_binding() {
        assert_ne!(
            dialogue_binding("今晚七点到九点", "一个人"),
            dialogue_binding("今晚七点到九点", "两个人")
        );
        assert_eq!(dialogue_binding("测试", ""), "测试");
    }
}
