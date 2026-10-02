//! Shared native transport/evidence verification for both business scenes.
use super::*;
pub(crate) fn content(op:&Operation,kind:&str)->Value {
    if op.action.permission=="participate" {return buwei_host_core::participation::Intent::parse(&op.action).map(|i|i.wire(op)).unwrap_or(Value::Null);}
    let bound=json!({"operation_id":op.id,"digest":op.digest,"action":op.action});
    if kind=="m.room.message" {json!({"msgtype":"m.text","body":format!("{}\n\n{}",op.action.payload["title"].as_str().unwrap_or_default(),op.action.payload["markdown"].as_str().unwrap_or_default()),"org.buwei.action":bound})}else{bound}
}
pub(crate) fn proof(client:&Client,room:&OwnedRoomId,op:&Operation,value:&Value,kind:&str)->Result<Evidence> {
    if value["type"]!=kind||value["sender"]!=op.account||op.account!=account(client)||op.action.target!=room.as_str()||value["content"]!=content(op,kind){return Err("服务端身份、对象或完整内容不匹配".into());}
    if value.get("room_id").is_some_and(|r|r!=room.as_str()){return Err("服务端活动对象不匹配".into());}
    let id=value["event_id"].as_str().filter(|s|s.starts_with('$')&&s.len()>1).ok_or("服务端事件编号缺失")?;
    Ok(Evidence{operation_id:op.id.clone(),external_id:id.into(),account:op.account.clone(),target:op.action.target.clone(),digest:op.digest.clone()})
}
pub(crate) fn publish(rt:&Runtime,client:&Client,room:&OwnedRoomId,op:&Operation,kind:&str)->Result<Evidence> {
    let id=send(rt,client,room,&op.id,kind,content(op,kind))?;let value=event(rt,client,room,id)?;proof(client,room,op,&value,kind)
}
pub(crate) fn lookup(rt:&Runtime,client:&Client,room:&OwnedRoomId,op:&Operation,kind:&str)->action_receipts::Result<Option<Evidence>> {
    #[cfg(feature="full-host")]
    let values=if rinx_bridge::official_mode(){official_sync::timeline(rt,client,room).map_err(|_|Error::State("服务器历史暂不可完整核实".into()))?}else{fixture_values(rt,client,room)?};
    #[cfg(not(feature="full-host"))]
    let values=fixture_values(rt,client,room)?;
    let mut matches=vec![];
    for value in values {
        let content=if kind=="m.room.message"{&value["content"]["org.buwei.action"]}else{&value["content"]};
        let key=if op.action.permission=="participate"{"action_id"}else{"operation_id"};
        if value["type"]==kind&&content[key]==op.id {matches.push(proof(client,room,op,&value,kind).map_err(|_|Error::Integrity)?);}
    }
    if matches.len()>1{return Err(Error::Integrity);}Ok(matches.pop())
}

fn fixture_values(rt:&Runtime,client:&Client,room:&OwnedRoomId)->action_receipts::Result<Vec<Value>>{
    let mut request=get_message_events::v3::Request::backward(room.clone());request.limit=100u32.into();
    let response=rt.block_on(async{client.send(request).await}).map_err(|_|Error::State("服务端暂不可核实".into()))?;
    response.chunk.into_iter().map(|r|serde_json::from_str(r.json().get()).map_err(|_|Error::Integrity)).collect()
}
