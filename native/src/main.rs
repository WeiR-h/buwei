//! Native SDK integration runner for a private loopback server. Its client,
//! DPAPI identities and Grants are host-owned, never guest arguments.
use action_receipts::{Action, Adapter, Authority, Dispatch, Error, Evidence, Grant, Journal, Operation, Status, new_id};
use buwei_host_core::{Activity, Preferences, Store};
use matrix_sdk::{Client, authentication::matrix::MatrixSession, config::RequestConfig, ruma::{OwnedRoomId, OwnedEventId, api::client::{message::{send_message_event,get_message_events},room::{create_room,get_room_event}},events::MessageLikeEventType,serde::Raw}};
use serde_json::{Value,json};
use std::{collections::BTreeMap,path::{Path,PathBuf},sync::Arc,time::{Duration,SystemTime,UNIX_EPOCH}};
use tokio::runtime::Runtime;
#[cfg(feature="full-host")]mod rinx_bridge;
mod channel;
mod consent;
#[cfg(feature="full-host")]mod official_sync;
mod article;
mod participant;
#[cfg(feature="acceptance")]mod acceptance;
#[cfg(feature="desktop")]mod controller;
#[cfg(feature="desktop")]mod gui;
#[cfg(feature="desktop")]mod host;
#[cfg(feature="desktop")]mod model;
#[cfg(feature="desktop")]mod shell_app;
type Result<T>=std::result::Result<T,String>;
fn now()->u64 {SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()}

#[cfg(windows)]
fn unseal(path:&Path,entropy:&[u8])->Result<Vec<u8>> {
    #[repr(C)]struct Blob{len:u32,data:*mut u8}
    #[link(name="crypt32")]unsafe extern "system"{fn CryptUnprotectData(input:*const Blob,description:*mut *mut u16,entropy:*const Blob,reserved:*mut u8,prompt:*mut u8,flags:u32,output:*mut Blob)->i32;}
    #[link(name="kernel32")]unsafe extern "system"{fn LocalFree(memory:*mut u8)->*mut u8;}
    let mut sealed=std::fs::read(path).map_err(|_|"请先完成本机私密配置")?;
    if sealed.len()>65536{return Err("私密配置超出范围".into());}
    let input=Blob{len:sealed.len() as u32,data:sealed.as_mut_ptr()};let extra=Blob{len:entropy.len() as u32,data:entropy.as_ptr() as *mut u8};let mut out=Blob{len:0,data:std::ptr::null_mut()};
    if unsafe{CryptUnprotectData(&input,std::ptr::null_mut(),&extra,std::ptr::null_mut(),std::ptr::null_mut(),1,&mut out)}==0{return Err("当前 Windows 用户无法读取私密配置".into());}
    unsafe{let clear=std::slice::from_raw_parts_mut(out.data,out.len as usize);let copied=clear.to_vec();clear.fill(0);LocalFree(out.data);Ok(copied)}
}
#[cfg(not(windows))]fn unseal(_: &Path,_:&[u8])->Result<Vec<u8>>{Err("此私密配置仅支持 Windows".into())}
fn private_sessions(path:&Path)->Result<BTreeMap<String,MatrixSession>>{let mut clear=unseal(path,b"buwei/matrix/v1")?;let parsed=serde_json::from_slice(&clear).map_err(|_|"私密身份格式不符合约定".into());clear.fill(0);parsed}
fn client(rt:&Runtime,session:MatrixSession)->Result<Client> {
    // The fixture permits only loopback. Production takes its client from Rinx.
    rt.block_on(async {
        let c=Client::builder().homeserver_url("http://127.0.0.1:19080").request_config(RequestConfig::new().disable_retry().timeout(Duration::from_secs(10))).build().await.map_err(|_|"SDK 客户端初始化失败")?;
        let expected=session.meta.user_id.clone();c.restore_session(session).await.map_err(|_|"SDK 会话恢复失败")?;
        let actual=c.whoami().await.map_err(|_|"服务端身份核验失败")?;
        if actual.user_id!=expected{return Err("SDK 身份与服务端身份不一致".into());}Ok(c)
    })
}
fn account(c:&Client)->String {c.user_id().expect("host verified SDK login").to_string()}
fn send(rt:&Runtime,c:&Client,room:&OwnedRoomId,transaction:&str,kind:&str,content:Value)->Result<OwnedEventId> {
    #[cfg(feature="acceptance")] acceptance::before_send()?;
    #[cfg(feature="full-host")] if rinx_bridge::official_mode(){rinx_bridge::ensure_current(c)?;}
    let raw=Raw::from_json(serde_json::value::to_raw_value(&content).map_err(|_|"消息格式不合法")?);
    let request=send_message_event::v3::Request::new_raw(room.clone(),transaction.into(),MessageLikeEventType::from(kind),raw);
    rt.block_on(async {c.send(request).await}).map(|r|r.event_id).map_err(|e|format!("消息发送结果待核实；HTTP {}",e.as_client_api_error().map(|e|e.status_code.as_u16()).unwrap_or(0)))
}
fn event(rt:&Runtime,c:&Client,room:&OwnedRoomId,id:OwnedEventId)->Result<Value> {
    let response=rt.block_on(async {c.send(get_room_event::v3::Request::new(room.clone(),id)).await}).map_err(|_|"服务端事件暂不可核实")?;
    #[cfg(feature="acceptance")] acceptance::after_server_event()?;
    serde_json::from_str(response.event.json().get()).map_err(|_|"服务端事件格式不合法".into())
}
struct SdkAdapter {runtime:Arc<Runtime>,client:Client,room:OwnedRoomId,state:PathBuf,drop_ack:bool}
impl SdkAdapter {
    fn proof(&self,op:&Operation,value:&Value)->Result<Evidence> {
        channel::proof(&self.client,&self.room,op,value,"org.buwei.invitation")
    }
    fn record(&self,proof:&Evidence)->Result<()> {let mut s=Store::open(&self.state)?;let a=s.load()?.ok_or("活动不存在")?;s.update(a.revision,|a|a.record_delivery(&proof.operation_id,&proof.account,&proof.target,&proof.external_id,&proof.digest))?;Ok(())}
}
impl Adapter for SdkAdapter {
    fn validate(&self,action:&Action,revision:u64)->action_receipts::Result<()> {
        let a=Store::open(&self.state).and_then(|s|s.load()).map_err(|_|Error::Integrity)?.ok_or(Error::Integrity)?;
        if account(&self.client)!=a.owner || action.target!=self.room.as_str() || revision!=a.revision || a.candidate()!=action.payload["person"].as_str() {return Err(Error::Conflict);}Ok(())
    }
    fn dispatch(&self,op:&Operation)->Dispatch {
        if account(&self.client)!=op.account{return Dispatch::Rejected("SDK 账号已变化".into());}
        let reserve=Store::open(&self.state).and_then(|mut s|s.update(op.revision,|a|a.reserve(op,now())));
        if reserve.is_err(){return Dispatch::Rejected("活动版本或候补规则已变化".into());}
        let content=json!({"operation_id":op.id,"digest":op.digest,"action":op.action});
        let result=send(&self.runtime,&self.client,&self.room,&op.id,"org.buwei.invitation",content).and_then(|id|event(&self.runtime,&self.client,&self.room,id)).and_then(|v|self.proof(op,&v));
        match result {
            Ok(proof) if !self.drop_ack => match self.record(&proof){Ok(())=>Dispatch::Verified(proof),Err(_)=>Dispatch::Uncertain("服务端已返回事件，业务回执待恢复".into())},
            _=>{if let Ok(mut store)=Store::open(&self.state){if let Ok(Some(a))=store.load(){let _=store.update(a.revision,|a|a.mark_unknown(&op.id));}}Dispatch::Uncertain("原操作编号保留，等待服务端核实".into())}
        }
    }
    fn lookup(&self,op:&Operation)->action_receipts::Result<Option<Evidence>> {
        let found=channel::lookup(&self.runtime,&self.client,&self.room,op,"org.buwei.invitation")?;
        if let Some(e)=&found{self.record(e).map_err(|_|Error::Integrity)?;}Ok(found)
    }
}
fn seed(root:&Path,rt:Arc<Runtime>,owner:Client,room:OwnedRoomId,participant:&str,drop_ack:bool)->Result<(Authority,Grant,Journal,SdkAdapter)> {
    std::fs::create_dir_all(root).map_err(|_|"测试记录目录不可用")?;
    let authority=Authority::default();authority.set_account(Some(&account(&owner)));let g=authority.grant("buwei",&["create","invite"],now(),3600).map_err(|_|"宿主授权失败")?;
    let mut a=Activity::new(account(&owner),room.to_string(),"补位 SDK 验收".into(),1,19,21)?;
    a.add_person(participant.into(),"测试参与者".into(),Preferences{earliest:17,latest:23,group:1},true)?;
    let state=root.join("activity.db");Store::open(&state)?.create(&g,&a,now())?;
    let journal=Journal::open(root.join("operations.db")).map_err(|_|"执行记录不可用")?;
    Ok((authority,g,journal,SdkAdapter{runtime:rt,client:owner,room,state,drop_ack}))
}
fn invite(j:&mut Journal,g:&Grant,adapter:&SdkAdapter,until:u64)->Result<Operation> {
    let a=Store::open(&adapter.state)?.load()?.ok_or("活动不存在")?;
    let op=j.prepare(g,Action{permission:"invite".into(),target:adapter.room.to_string(),summary:format!("邀请 {}；有效至 {until}",a.candidate().ok_or("没有可邀请的候补")?),payload:json!({"person":a.candidate(),"until":until})},a.revision,now(),120).map_err(|_|"动作预览失败")?;
    j.confirm(g,&op.id,adapter,now()).map_err(|_|"动作确认失败")?;j.execute(g,&op.id,adapter,now()).map_err(|_|"动作执行失败".into())
}
fn reply(rt:&Runtime,participant:&Client,adapter:&SdkAdapter,op:&Operation,accept:bool)->Result<Value> {
    let a=Store::open(&adapter.state)?.load()?.ok_or("活动不存在")?;let i=a.invitations.iter().find(|i|i.operation_id==op.id).ok_or("邀请不存在")?;
    let content=json!({"operation_id":op.id,"accept":accept,"m.relates_to":{"m.in_reply_to":{"event_id":i.server_event}}});
    let id=send(rt,participant,&adapter.room,&new_id(),"org.buwei.reply",content)?;event(rt,&adapter.client,&adapter.room,id)
}
fn apply_reply(adapter:&SdkAdapter,v:&Value)->Result<Activity> {
    if v["type"]!="org.buwei.reply" {return Err("回复事件类型不匹配".into());}
    let c=&v["content"];let mut s=Store::open(&adapter.state)?;let a=s.load()?.ok_or("活动不存在")?;
    s.update(a.revision,|a|a.receive_reply(c["operation_id"].as_str().ok_or("回复编号缺失")?,v["sender"].as_str().ok_or("服务端身份缺失")?,adapter.room.as_str(),c["m.relates_to"]["m.in_reply_to"]["event_id"].as_str().ok_or("回复关联缺失")?,c["accept"].as_bool().ok_or("回复结果缺失")?,v["origin_server_ts"].as_u64().ok_or("服务端时间缺失")?/1000,now()))
}
fn run(root:&Path)->Result<Value> {
    let sessions=private_sessions(&root.join(".run/matrix/identities.dpapi"))?;let rt=Arc::new(Runtime::new().map_err(|_|"SDK 运行环境初始化失败")?);
    let owner=client(&rt,sessions.get("organizer").ok_or("组织者身份缺失")?.clone())?;let participant=client(&rt,sessions.get("participant").ok_or("参与者身份缺失")?.clone())?;
    if account(&owner)==account(&participant){return Err("需要两个不同的真实测试身份".into());}
    let room=rt.block_on(async {let mut request=create_room::v3::Request::new();request.name=Some("补位本机双账号验收".into());request.invite.push(participant.user_id().unwrap().to_owned());owner.create_room(request).await}).map_err(|_|"测试房间创建失败")?.room_id().to_owned();
    rt.block_on(participant.join_room_by_id(&room)).map_err(|_|"参与者加入测试房间失败")?;
    let run=root.join(".run/matrix-checks").join(new_id());let mut rounds=vec![];
    for n in 1..=5 {
        let(_,g,mut j,a)=seed(&run.join(format!("normal-{n}")),rt.clone(),owner.clone(),room.clone(),&account(&participant),false)?;let op=invite(&mut j,&g,&a,now()+300)?;
        if op.status!=Status::Confirmed{return Err("真实邀请缺少已核实的服务端回执".into());}
        j.execute(&g,&op.id,&a,now()).map_err(|_|"重复确认检查失败")?;
        let response=reply(&rt,&participant,&a,&op,true)?;let accepted=apply_reply(&a,&response)?;
        if accepted.confirmed()!=1 || accepted.held()!=0 || apply_reply(&a,&response).is_ok(){return Err("本人接受或重复回复规则失败".into());}
        let mut s=Store::open(&a.state)?;s.update(accepted.revision,|a|a.cancel_own(&account(&participant)))?;if s.load()?.unwrap().confirmed()!=0{return Err("本人取消规则失败".into());}
        rounds.push(json!({"round":n,"operation_id":op.id,"server_event":op.receipt.as_ref().unwrap().evidence.as_ref().unwrap().external_id,"invite_verified":true,"participant_accept_verified":true,"duplicate_reply_rejected":true,"participant_cancel_verified":true}));
    }
    let(_,g,mut j,a)=seed(&run.join("decline"),rt.clone(),owner.clone(),room.clone(),&account(&participant),false)?;let op=invite(&mut j,&g,&a,now()+300)?;let response=reply(&rt,&participant,&a,&op,false)?;let declined=apply_reply(&a,&response)?;if declined.confirmed()!=0||declined.held()!=0{return Err("拒绝流程失败".into());}
    let(_,g,mut j,a)=seed(&run.join("expiry"),rt.clone(),owner.clone(),room.clone(),&account(&participant),false)?;let op=invite(&mut j,&g,&a,now()+2)?;
    let forged=reply(&rt,&owner,&a,&op,true)?;if apply_reply(&a,&forged).is_ok(){return Err("他人回复错误占位".into());}
    std::thread::sleep(Duration::from_secs(2));let late=reply(&rt,&participant,&a,&op,true)?;if apply_reply(&a,&late).is_ok(){return Err("过期回复错误占位".into());}
    let mut s=Store::open(&a.state)?;let state=s.load()?.unwrap();let expired=s.update(state.revision,|a|{a.expire(now());Ok(())})?;if expired.confirmed()!=0||expired.held()!=0{return Err("过期清理失败".into());}
    let(_,g,mut j,a)=seed(&run.join("lost-ack"),rt.clone(),owner.clone(),room.clone(),&account(&participant),true)?;let op=invite(&mut j,&g,&a,now()+300)?;if op.status!=Status::Unknown{return Err("不确定状态未保留".into());}
    drop(j);let mut recovered=Journal::open(run.join("lost-ack/operations.db")).map_err(|_|"执行记录重启恢复失败")?;
    let fresh_authority=Authority::default();fresh_authority.set_account(Some(&account(&owner)));let fresh=fresh_authority.grant("buwei",&["invite"],now(),3600).map_err(|_|"恢复授权失败")?;
    let verified=recovered.reconcile(&fresh,&op.id,&a,now()).map_err(|_|"服务端回执恢复失败")?;if verified.status!=Status::Confirmed || verified.id!=op.id{return Err("原操作编号未恢复".into());}
    if Store::open(&a.state)?.load()?.unwrap().held()!=1{return Err("待回复名额丢失".into());}
    let article=article::verify(&run,rt.clone(),owner.clone(),room.clone())?;
    Ok(json!({"article":article,"version":env!("CARGO_PKG_VERSION"),"scope":"native SDK private-loopback integration; official Rinx UI not yet connected","sdk_commit":"6892cb217ae4a886571e928c8efcccfbec5490a6","organizer":account(&owner),"participant":account(&participant),"room":room,"normal_rounds":rounds,"decline_verified":true,"wrong_sender_rejected":true,"expired_reply_rejected":true,"expired_hold_released":true,"lost_ack_recovered_original_operation":true,"lost_ack_fault_injection":"discard local acknowledgement after real server commit","lost_ack_operation_id":op.id,"recovery_resends":0,"end_to_end_encryption":false}))
}
fn main() {
    let _=rustls::crypto::ring::default_provider().install_default();
    let root=std::env::args().nth(1).map(PathBuf::from).unwrap_or_else(||std::env::current_dir().unwrap());
    #[cfg(feature="desktop")]
    if std::env::args().any(|a|a=="--gui") {
        let root=root.canonicalize().expect("native host root must exist");
        if root.join("migration-failed.local.json").exists(){eprintln!("迁移尚未通过验证，请使用保留的旧版入口；新版已停止写入。");std::process::exit(1);}
        #[cfg(windows)]
        let _profile_lock={
            use std::os::windows::fs::OpenOptionsExt;
            match std::fs::OpenOptions::new().create(true).truncate(false).read(true).write(true).share_mode(0).open(root.join(".run.lock")){
                Ok(file)=>file,Err(_)=>{eprintln!("本资料目录已由另一个宿主使用；请先关闭原窗口。");std::process::exit(1);}
            }
        };
        let state=root.join("data").join(format!("v{}",env!("CARGO_PKG_VERSION"))).join("shell");
        unsafe{std::env::set_var("OCTOSENSE_HOME",&state);std::env::set_var("OCTOS_APP_CORE_DIR",state.join("octos-home/.octos"));std::env::set_var("RINX_DATA_DIR",root.join("data").join(format!("v{}",env!("CARGO_PKG_VERSION"))).join("rinx"));}
        #[cfg(feature="full-host")] if rinx_bridge::official_mode(){let _=rinx_bridge::record_status(&root,None,false,false);}
        host::configure(root);shell_app::run();return;
    }
    match run(&root) {Ok(report)=>{let dir=root.join(format!("evidence/v{}",env!("CARGO_PKG_VERSION")));if std::fs::create_dir_all(&dir).is_err()||std::fs::write(dir.join("matrix-sdk-live.json"),serde_json::to_vec_pretty(&report).unwrap()).is_err(){eprintln!("验收完成，但证据文件不可写");std::process::exit(1);}println!("{}",report);},Err(message)=>{eprintln!("{}",json!({"verified":false,"reason":message}));std::process::exit(1);}}
}
