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
pub(crate) fn check_authority(rt:&Runtime,c:&Client,a:&Activity)->Result<()> {
    let room=OwnedRoomId::try_from(a.room.as_str()).map_err(|_|"活动房间不合法")?;
    let values=states(rt,c,&room)?;
    if values.iter().any(|v|v["type"]==KIND&&v["state_key"]==""){
        let remote=snapshot_from_events(room.as_str(),&account(c),&values)?;
        if remote.revision>a.revision || (remote.revision==a.revision&&serde_json::to_value(&remote).map_err(|_|"状态不合法")?!=serde_json::to_value(a).map_err(|_|"状态不合法")?){return Err("服务端活动比本机更新或冲突；停止推进，请只保留一个组织者宿主并恢复原资料".into());}
    }Ok(())
}
fn snapshot_digest(a:&Activity)->Result<String>{use sha2::{Digest,Sha256};Ok(hex::encode(Sha256::digest(serde_json::to_vec(a).map_err(|_|"快照不合法")?)))}
fn projection_action(a:&Activity)->Result<Action>{Ok(Action{permission:"sync_state".into(),target:a.room.clone(),summary:"同步已核验的活动快照；不发送新邀请".into(),payload:json!({"snapshot_digest":snapshot_digest(a)?})})}
fn projection_content(op:&Operation,a:&Activity)->Value{json!({"protocol":1,"activity":a,"org.buwei.action":{"operation_id":op.id,"digest":op.digest,"action":op.action,"revision":op.revision}})}
fn projection_proof(op:&Operation,v:&Value)->Result<Evidence>{
    let a:Activity=serde_json::from_value(v["content"]["activity"].clone()).map_err(|_|"同步快照缺失")?;a.validate()?;
    if op.app!="buwei-sync" || op.action!=projection_action(&a)? || op.account!=a.owner || op.revision!=a.revision || v["sender"]!=op.account || v["type"]!=KIND || v["state_key"]!="" || v.get("room_id").is_some_and(|r|r!=a.room.as_str()) || v["content"]!=projection_content(op,&a){return Err("快照身份、内容、版本或原编号不匹配".into());}
    let id=v["event_id"].as_str().filter(|id|id.starts_with('$')&&id.len()>1).ok_or("同步服务端编号缺失")?;
    Ok(Evidence{operation_id:op.id.clone(),external_id:id.into(),account:op.account.clone(),target:op.action.target.clone(),digest:op.digest.clone()})
}
struct StateAdapter{runtime:Arc<Runtime>,client:Client,room:OwnedRoomId,state:PathBuf,grant:Grant}
impl Adapter for StateAdapter{
    fn validate(&self,action:&Action,revision:u64)->action_receipts::Result<()>{
        self.grant.check("sync_state",now())?;
        let a=Store::open(&self.state).and_then(|s|s.load()).map_err(|_|Error::Integrity)?.ok_or(Error::Integrity)?;
        if account(&self.client)!=a.owner || self.room.as_str()!=a.room || revision!=a.revision || projection_action(&a).map_err(|_|Error::Integrity)?!=*action{return Err(Error::Conflict);}Ok(())
    }
    fn dispatch(&self,op:&Operation)->Dispatch{
        let result=(||{
            self.validate(&op.action,op.revision).map_err(|_|"同步授权或业务版本已变化")?;rinx_bridge::ensure_current(&self.client)?;
            let a=Store::open(&self.state)?.load()?.ok_or("活动缺失")?;
            let raw=Raw::from_json(serde_json::value::to_raw_value(&projection_content(op,&a)).map_err(|_|"快照格式不合法")?);
            let request=send_state_event::v3::Request::new_raw(self.room.clone(),StateEventType::from(KIND),String::new(),raw);
            // State PUT has no Matrix transaction ID. Disable SDK retransmits;
            // journal lookup must resolve a missing acknowledgment first.
            let id=self.runtime.block_on(async{self.client.send(request).with_request_config(RequestConfig::new().disable_retry().timeout(Duration::from_secs(10))).await}).map_err(|_|"快照发送结果待核实")?.event_id;
            projection_proof(op,&event(&self.runtime,&self.client,&self.room,id)?)
        })();match result{Ok(e)=>Dispatch::Verified(e),Err(message)=>Dispatch::Uncertain(message)}
    }
    fn lookup(&self,op:&Operation)->action_receipts::Result<Option<Evidence>>{
        self.grant.check("sync_state",now())?;rinx_bridge::ensure_current(&self.client).map_err(|_|Error::Authorization)?;
        if account(&self.client)!=op.account || op.action.target!=self.room.as_str(){return Err(Error::Authorization);}
        let events=states(&self.runtime,&self.client,&self.room).map_err(|_|Error::State("快照暂不可读取".into()))?;
        let Some(v)=events.iter().find(|v|v["type"]==KIND&&v["state_key"]=="")else{return Ok(None);};
        if v["content"]["org.buwei.action"]["operation_id"]!=op.id{return Ok(None);}
        Ok(Some(projection_proof(op,v).map_err(|_|Error::Integrity)?))
    }
}
pub(crate) fn publish(rt:Arc<Runtime>,c:&Client,state:PathBuf,g:&Grant,j:&mut Journal)->Result<()> {
    g.check("sync_state",now()).map_err(|_|"自动同步授权已失效")?;rinx_bridge::ensure_current(c)?;
    let a=Store::open(&state)?.load()?.ok_or("活动缺失")?;a.validate()?;
    if a.owner!=account(c)||g.account()!=a.owner||g.app()!="buwei-sync"{return Err("快照需要组织者当前身份".into());}
    let room=OwnedRoomId::try_from(a.room.as_str()).map_err(|_|"活动房间不合法")?;
    let adapter=StateAdapter{runtime:rt.clone(),client:c.clone(),room:room.clone(),state,grant:g.clone()};
    for op in j.pending(g,now()).map_err(|_|"同步记录不可读取")?{
        let restored=j.reconcile(g,&op.id,&adapter,now()).map_err(|_|"原同步回执暂不可核实")?;
        if restored.status!=Status::Confirmed{return Err("原快照发送待核实；已停止新快照发送，请保持原编号".into());}
    }
    let current=states(&rt,c,&room)?;
    if current.iter().any(|v|v["type"]==KIND&&v["state_key"]==""){
        let existing=snapshot_from_events(room.as_str(),&account(c),&current)?;
        if serde_json::to_value(&existing).map_err(|_|"状态不合法")?==serde_json::to_value(&a).map_err(|_|"状态不合法")?{return Ok(());}
        if existing.revision>=a.revision{return Err("服务端已有不同或更新的活动版本；停止覆盖，请恢复原组织者资料".into());}
    }
    let op=j.prepare(g,projection_action(&a)?,a.revision,now(),120).map_err(|_|"快照预执行记录不可保存")?;
    j.confirm(g,&op.id,&adapter,now()).map_err(|_|"同步确认已失效")?;
    if j.execute(g,&op.id,&adapter,now()).map_err(|_|"快照执行记录不可恢复")?.status!=Status::Confirmed{return Err("快照送达待核实；保留原同步编号".into());}Ok(())
}
pub(crate) fn timeline(rt:&Runtime,c:&Client,room:&OwnedRoomId)->Result<Vec<Value>> {
    timeline_since(rt,c,room,None)
}
pub(crate) fn timeline_since(rt:&Runtime,c:&Client,room:&OwnedRoomId,checkpoint:Option<&str>)->Result<Vec<Value>> {
    let mut all=vec![];let mut from=None;let mut seen=std::collections::BTreeSet::new();
    for _ in 0..20 {
        let mut request=get_message_events::v3::Request::backward(room.clone());request.limit=100u32.into();request.from=from.clone();
        let response=rt.block_on(async {c.send(request).await}).map_err(|_|"参与者消息暂不可读取")?;
        let empty=response.chunk.is_empty();for raw in response.chunk {let v:Value=serde_json::from_str(raw.json().get()).map_err(|_|"消息格式不合法")?;if v.to_string().len()>262144{return Err("消息过大".into());}if checkpoint.is_some_and(|id|v["event_id"]==id){all.reverse();return Ok(all);}all.push(v);}
        if empty||response.end.is_none(){if checkpoint.is_some(){return Err("历史中找不到已保存的同步进度；暂停处理和过期释放，请恢复历史".into());}all.reverse();return Ok(all);}
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
    #[test]fn projection_receipt_rejects_changed_identity_revision_content_and_operation(){
        let a=a();let authority=Authority::default();authority.set_account(Some(&a.owner));
        let grant=authority.grant("buwei-sync",&["sync_state"],100,3600).unwrap();
        let mut journal=Journal::open(":memory:").unwrap();let op=journal.prepare(&grant,projection_action(&a).unwrap(),a.revision,100,120).unwrap();
        let valid=json!({"type":KIND,"state_key":"","sender":a.owner,"room_id":a.room,"event_id":"$evidence","origin_server_ts":101000,"content":projection_content(&op,&a)});
        assert!(projection_proof(&op,&valid).is_ok());
        for (field,value) in [("sender",json!("@other:server")),("state_key",json!("other")),("room_id",json!("!other")),("event_id",json!(""))]{let mut bad=valid.clone();bad[field]=value;assert!(projection_proof(&op,&bad).is_err());}
        let mut bad=valid.clone();bad["content"]["org.buwei.action"]["operation_id"]=json!("different");assert!(projection_proof(&op,&bad).is_err());
        let mut bad=valid.clone();bad["content"]["activity"]["title"]=json!("changed");assert!(projection_proof(&op,&bad).is_err());
        let mut bad=valid;bad["content"]["activity"]["revision"]=json!(a.revision+1);assert!(projection_proof(&op,&bad).is_err());
    }
    fn a()->Activity{Activity::new("@owner:server".into(),"!room".into(),"测试活动".into(),1,19,21).unwrap()}
    fn states()->Vec<Value>{vec![json!({"type":"m.room.create","state_key":"","sender":"@owner:server"}),json!({"type":"m.room.member","state_key":"@p:server","content":{"membership":"join"}}),json!({"type":KIND,"state_key":"","sender":"@owner:server","content":{"protocol":1,"activity":a()}})]}
    #[test]fn snapshot_requires_creator_sender_exact_room_and_membership(){let e=states();assert!(snapshot_from_events("!room","@p:server",&e).is_ok());assert!(snapshot_from_events("!other","@p:server",&e).is_err());assert!(snapshot_from_events("!room","@q:server",&e).is_err());let mut e=states();e[2]["sender"]=json!("@p:server");assert!(snapshot_from_events("!room","@p:server",&e).is_err());}
    #[test]fn content_cannot_impersonate_sender(){let mut a=a();let v=json!({"type":"org.buwei.join","sender":"@p:server","origin_server_ts":100000,"content":{"sender":"@q:server","preferences":{"earliest":17,"latest":23,"group":1}}});apply_participant(&mut a,"!room",&v,101).unwrap();assert_eq!(a.people[0].account,"@p:server");assert!(apply_participant(&mut a,"!other",&v,101).is_err());let mut v=v;v["sender"]=json!("@owner:server");assert!(apply_participant(&mut a,"!room",&v,101).is_err());}
}
