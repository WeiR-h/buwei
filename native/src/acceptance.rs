//! Explicitly opted-in local acceptance control. Excluded from normal builds.
//! Requests route through the same Controller and SDK identity/consent checks.
use super::*;
use super::controller::{Command,Controller,View};
use serde::Deserialize;
use std::sync::atomic::{AtomicU8,Ordering};
static FAULT:AtomicU8=AtomicU8::new(0);
pub(crate) fn set_fault(mode:&str)->Result<()>{
    let value=match mode{"off"=>0,"before_send"=>1,"lost_ack"=>2,"crash_before_receipt"=>3,_=>return Err("故障类型不符合约定".into())};
    if !std::env::args().any(|a|a=="--acceptance"){return Err("未开启验收控制".into());}
    FAULT.store(value,Ordering::SeqCst);Ok(())
}
pub(crate) fn before_send()->Result<()>{if FAULT.compare_exchange(1,0,Ordering::SeqCst,Ordering::SeqCst).is_ok(){return Err("验收注入：发送前传输不可用，保持原编号待核实".into());}Ok(())}
pub(crate) fn after_server_event(kind:&str)->Result<()>{
    if !matches!(kind,"org.buwei.invitation"|"org.buwei.join"|"org.buwei.reply"|"org.buwei.cancel"|"m.room.message"){return Ok(());}
    if FAULT.compare_exchange(2,0,Ordering::SeqCst,Ordering::SeqCst).is_ok(){return Err("验收注入：服务器事件已读取，丢弃本地回执".into());}
    if FAULT.compare_exchange(3,0,Ordering::SeqCst,Ordering::SeqCst).is_ok(){std::process::abort();}
    Ok(())
}
#[derive(Deserialize)]#[serde(deny_unknown_fields)]struct Config{account:String,room:String,nonce:String,expires_at:u64,#[serde(default)]pause_automatic_sync:bool}
#[derive(Deserialize)]#[serde(deny_unknown_fields)]struct Packet{id:String,nonce:String,action:Command}
fn read<T:serde::de::DeserializeOwned>(path:&Path)->Option<T>{let bytes=std::fs::read(path).ok()?;if bytes.len()>65536{return None;}serde_json::from_slice(&bytes).ok()}
pub(crate) fn automatic_sync_paused(root:&Path)->bool{
    std::env::args().any(|a|a=="--acceptance")&&read::<Config>(&root.join(".run/acceptance/config.local.json")).is_some_and(|c|c.pause_automatic_sync)
}
pub(crate) fn poll(root:&Path,c:&mut Controller)->Option<View>{
    if !std::env::args().any(|a|a=="--acceptance"){return None;}
    let directory=root.join(".run/acceptance");
    let config:Config=read(&directory.join("config.local.json"))?;
    let packet:Packet=read(&directory.join("request.local.json"))?;
    if config.expires_at<=now() || config.expires_at>now()+3600 || config.nonce.len()<32 || packet.nonce!=config.nonce || packet.id.len()!=32 || !packet.id.bytes().all(|b|b.is_ascii_hexdigit()){return None;}
    let snapshot=c.acceptance_snapshot();
    if snapshot["account"]!=config.account || snapshot["activity"].as_object().is_some_and(|a|a.get("room")!=Some(&json!(config.room))){return None;}
    let marker=directory.join(format!("{}.done",packet.id));if marker.exists(){return None;}
    // Persist the control request marker first. Restart cannot replay a stale
    // confirmation. The separate operation journal retains its own recovery ID.
    std::fs::write(&marker,b"claimed").ok()?;
    let view=c.handle(packet.action);let mut report=c.acceptance_snapshot();report["request_id"]=packet.id.clone().into();
    let bytes=serde_json::to_vec_pretty(&report).ok()?;
    let _=std::fs::write(directory.join(format!("{}.result.json",packet.id)),&bytes);
    let _=std::fs::write(directory.join("status.local.json"),bytes);
    Some(view)
}
