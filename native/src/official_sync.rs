//! SDK events are the identity source; content cannot supply its own actor.
use super::*;
use matrix_sdk::ruma::{events::StateEventType,api::client::state::{get_state_events,send_state_event}};
const KIND:&str="org.buwei.activity";
pub(crate) fn snapshot_from_events(room:&str,actor:&str,events:&[Value])->Result<Activity> {
    let create=events.iter().find(|v|v["type"]=="m.room.create"&&v["state_key"]=="").ok_or("房间创建证据缺失")?;
    let owner=create["sender"].as_str().ok_or("房间创建者缺失")?;
    if !events.iter().any(|v|v["type"]=="m.room.member"&&v["state_key"]==actor&&v["content"]["membership"]=="join"){return Err("本人尚未加入这个活动房间".into());}
    let state=events.iter().find(|v|v["type"]==KIND&&v["state_key"]=="").ok_or("组织者尚未同步活动")?;
    if state["sender"]!=owner||state["content"]["protocol"]!=1||state.get("room_id").is_some_and(|v|v!=room)||state["content"].to_string().len()>262144{return Err("活动来源或协议不匹配".into());}
    let a:Activity=serde_json::from_value(state["content"]["activity"].clone()).map_err(|_|"活动格式不符合约定")?;
    a.validate()?;if a.owner!=owner||a.room!=room{return Err("活动组织者或房间不一致".into());}Ok(a)
}
fn states(rt:&Runtime,c:&Client,room:&OwnedRoomId)->Result<Vec<Value>> {
    let response=rt.block_on(async {c.send(get_state_events::v3::Request::new(room.clone())).await}).map_err(|_|"活动房间状态暂不可读取")?;
    if response.room_state.len()>500{return Err("房间状态超出本版范围".into());}
    response.room_state.into_iter().map(|r|serde_json::from_str(r.json().get()).map_err(|_|"房间状态格式不合法".into())).collect()
}
pub(crate) fn fetch(rt:&Runtime,c:&Client,room:&OwnedRoomId)->Result<Activity> {snapshot_from_events(room.as_str(),&account(c),&states(rt,c,room)?)}
pub(crate) fn publish(rt:&Runtime,c:&Client,a:&Activity)->Result<()> {
    rinx_bridge::ensure_current(c)?;a.validate()?;if account(c)!=a.owner{return Err("活动同步需要组织者身份".into());}
    let room=OwnedRoomId::try_from(a.room.as_str()).map_err(|_|"活动房间不合法")?;
    // Read before writing makes a lost acknowledgement recoverable without
    // another state event. This state projection does not allocate seats.
    let current=states(rt,c,&room)?;
    if current.iter().any(|v|v["type"]==KIND&&v["state_key"]==""){let existing=snapshot_from_events(room.as_str(),&account(c),&current)?;if serde_json::to_value(&existing).map_err(|_|"状态不合法")?==serde_json::to_value(a).map_err(|_|"状态不合法")?{return Ok(());}}
    let content=json!({"protocol":1,"activity":a});
    let raw=Raw::from_json(serde_json::value::to_raw_value(&content).map_err(|_|"活动快照不合法")?);
    let request=send_state_event::v3::Request::new_raw(room.clone(),StateEventType::from(KIND),String::new(),raw);
    let id=rt.block_on(async {c.send(request).await}).map_err(|_|"活动同步结果待核实；再次同步先读取原快照")?.event_id;
    let v=event(rt,c,&room,id)?;
    if v["sender"]!=a.owner||v["type"]!=KIND||v["state_key"]!=""||v["content"]!=content{return Err("活动同步服务端证据不匹配".into());}Ok(())
}
pub(crate) fn timeline(rt:&Runtime,c:&Client,room:&OwnedRoomId)->Result<Vec<Value>> {
    let mut all=vec![];let mut from=None;let mut seen=std::collections::BTreeSet::new();
    for _ in 0..20 {
        let mut request=get_message_events::v3::Request::backward(room.clone());request.limit=100u32.into();request.from=from.clone();
        let response=rt.block_on(async {c.send(request).await}).map_err(|_|"参与者消息暂不可读取")?;
        let empty=response.chunk.is_empty();for raw in response.chunk {let v:Value=serde_json::from_str(raw.json().get()).map_err(|_|"消息格式不合法")?;if v.to_string().len()>262144{return Err("消息过大".into());}all.push(v);}
        if empty||response.end.is_none(){all.reverse();return Ok(all);}
        let next=response.end.unwrap();if !seen.insert(next.clone()){return Err("服务器分页未前进".into());}from=Some(next);
    }Err("活动历史超过 2000 条；本版停止同步并保留记录".into())
}
pub(crate) fn apply_participant(a:&mut Activity,room:&str,v:&Value,clock:u64)->Result<()> {
    if a.room!=room||v.get("room_id").is_some_and(|r|r!=room){return Err("消息活动对象不一致".into());}
    let actor=v["sender"].as_str().filter(|s|buwei_host_core::account_valid(s)).ok_or("消息发送者缺失")?;
    if actor==a.owner{return Err("组织者不能代替参与者回复".into());}
    let time=v["origin_server_ts"].as_u64().ok_or("服务端时间缺失")?/1000;
    if time>clock.saturating_add(5){return Err("服务端时间异常".into());}
    let c=&v["content"];
    match v["type"].as_str(){
        Some("org.buwei.join")=>{let p:Preferences=serde_json::from_value(c["preferences"].clone()).map_err(|_|"时段不合法")?;a.join_own(actor.into(),"参与者".into(),p)},
        Some("org.buwei.reply")=>a.receive_reply(c["operation_id"].as_str().ok_or("回复编号缺失")?,actor,room,c["m.relates_to"]["m.in_reply_to"]["event_id"].as_str().ok_or("回复关联缺失")?,c["accept"].as_bool().ok_or("回复结果缺失")?,time,clock),
        Some("org.buwei.cancel") if c["activity"]==room=>a.cancel_own(actor),
        _=>Err("不支持的参与者消息".into())
    }
}
#[cfg(test)]mod tests {
    use super::*;
    fn a()->Activity{Activity::new("@owner:server".into(),"!room".into(),"测试活动".into(),1,19,21).unwrap()}
    fn states()->Vec<Value>{vec![json!({"type":"m.room.create","state_key":"","sender":"@owner:server"}),json!({"type":"m.room.member","state_key":"@p:server","content":{"membership":"join"}}),json!({"type":KIND,"state_key":"","sender":"@owner:server","content":{"protocol":1,"activity":a()}})]}
    #[test]fn snapshot_requires_creator_sender_exact_room_and_membership(){let e=states();assert!(snapshot_from_events("!room","@p:server",&e).is_ok());assert!(snapshot_from_events("!other","@p:server",&e).is_err());assert!(snapshot_from_events("!room","@q:server",&e).is_err());let mut e=states();e[2]["sender"]=json!("@p:server");assert!(snapshot_from_events("!room","@p:server",&e).is_err());}
    #[test]fn content_cannot_impersonate_sender(){let mut a=a();let v=json!({"type":"org.buwei.join","sender":"@p:server","origin_server_ts":100000,"content":{"sender":"@q:server","preferences":{"earliest":17,"latest":23,"group":1}}});apply_participant(&mut a,"!room",&v,101).unwrap();assert_eq!(a.people[0].account,"@p:server");assert!(apply_participant(&mut a,"!other",&v,101).is_err());let mut v=v;v["sender"]=json!("@owner:server");assert!(apply_participant(&mut a,"!room",&v,101).is_err());}
}
