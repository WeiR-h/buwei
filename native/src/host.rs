//! Host package owns workers, clients, files and model settings. The embedded
//! module receives only bounded command/update channels and its native scope.
use super::*;
use controller::{Command,Controller,View};
use makepad_app_module::InstanceScope;
use makepad_widgets::Timer;
use std::sync::{OnceLock,Mutex,atomic::{AtomicBool,Ordering},mpsc::{SyncSender,Receiver,sync_channel,channel}};
static ROOT:OnceLock<PathBuf>=OnceLock::new();
struct Lease{scope:InstanceScope,alive:Arc<AtomicBool>,timer:Option<Timer>}
static LEASE:Mutex<Option<Lease>>=Mutex::new(None);
pub(crate) fn configure(root:PathBuf){let _=ROOT.set(root);}
pub(crate) fn open(scope:InstanceScope)->(SyncSender<Command>,Receiver<Result<View>>){
    let(tx,commands)=sync_channel(2);let(updates,rx)=channel();let alive=Arc::new(AtomicBool::new(true));
    {let mut lease=LEASE.lock().unwrap();if lease.as_ref().is_some_and(|l|l.alive.load(Ordering::SeqCst)){let _=updates.send(Err("补位已在另一宿主窗口运行。请先关闭原窗口。".into()));return(tx,rx);}
    *lease=Some(Lease{scope,alive:alive.clone(),timer:None});}
    let root=ROOT.get().cloned();
    std::thread::spawn(move||{
        let root=match root{Some(r)=>r,None=>{let _=updates.send(Err("宿主配置尚未完成".into()));return;}};
        while alive.load(Ordering::SeqCst){
            // Discard actions queued while logged out or while a new account is binding.
            for _ in commands.try_iter(){}
            match Controller::open(&root){
                Ok(mut c)=>{
                    for _ in commands.try_iter(){}
                    let mut last_view_at=now();let mut last_authorized=c.is_authorized();let _=updates.send(Ok(c.view()));
                    while alive.load(Ordering::SeqCst){
                        if !c.host_session_current(){c.shutdown();let _=updates.send(Ok(View{account:"未登录".into(),message:"Rinx 会话已变化，旧授权已撤销；正在等待本人重新登录。".into(),..Default::default()}));break;}
                        match commands.recv_timeout(Duration::from_millis(250)){
                            Ok(command)=>{if !alive.load(Ordering::SeqCst){break;}let result=c.handle(command);last_view_at=now();last_authorized=result.authorized;if alive.load(Ordering::SeqCst)&&updates.send(Ok(result)).is_err(){break;}},
                            Err(std::sync::mpsc::RecvTimeoutError::Timeout)=>{if (last_authorized&&!c.is_authorized())||now().saturating_sub(last_view_at)>=10{last_authorized=c.is_authorized();last_view_at=now();let _=updates.send(Ok(c.view()));}},Err(_)=>{alive.store(false,Ordering::SeqCst);break;}
                        }
                    }
                    c.shutdown();
                },
                Err(message)=>{#[cfg(feature="full-host")] if super::rinx_bridge::official_mode(){let _=super::rinx_bridge::record_status(&root,None,false,false);}let _=updates.send(Ok(View{account:"未登录".into(),message,..Default::default()}));}
            }
            for _ in 0..10{if !alive.load(Ordering::SeqCst){break;}std::thread::sleep(Duration::from_millis(100));}
        }
        alive.store(false,Ordering::SeqCst);
    });(tx,rx)
}
pub(crate) fn timer(scope:InstanceScope,timer:Timer){if let Some(l)=LEASE.lock().unwrap().as_mut(){if l.scope==scope{l.timer=Some(timer);}}}
pub(crate) fn close(scope:InstanceScope)->Option<Timer>{let mut lease=LEASE.lock().unwrap();if lease.as_ref().is_some_and(|l|l.scope==scope){let l=lease.take().unwrap();l.alive.store(false,Ordering::SeqCst);l.timer}else{None}}
