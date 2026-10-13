//! Server facts establish public-directory scope; a card cannot declare access.
use crate::{Result, account_valid};
use serde_json::Value;

pub fn verify(states: &[Value]) -> Result<String> {
    if states.iter().any(|s| s["type"] == "m.room.encryption") {
        return Err("公开大厅不能是加密房间".into());
    }
    let state = |kind: &str| {
        states
            .iter()
            .find(|s| s["type"] == kind && s["state_key"] == "")
    };
    if state("m.room.join_rules").is_none_or(|s| s["content"]["join_rule"] != "public") {
        return Err("该房间不是可公开加入的活动大厅".into());
    }
    let marker = state("org.buwei.directory").ok_or("该房间没有补位公开大厅声明")?;
    let creator = state("m.room.create")
        .and_then(|s| s["sender"].as_str())
        .ok_or("公开大厅创建来源缺失")?;
    if marker["content"]["protocol"] != 1
        || marker["content"]["kind"] != "public"
        || marker["sender"] != creator
        || marker["content"]["creator"] != creator
        || !account_valid(creator)
    {
        return Err("公开大厅声明与服务端创建者不一致".into());
    }
    Ok(creator.to_owned())
}
