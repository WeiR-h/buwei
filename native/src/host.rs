//! Host package owns workers, clients, files and model settings. The embedded
//! module receives only bounded command/update channels and its native scope.
use super::*;
use controller::{Command, Controller, View};
use makepad_app_module::InstanceScope;
use makepad_widgets::Timer;
use std::sync::{
    Mutex, OnceLock,
    atomic::{AtomicBool, Ordering},
    mpsc::{Receiver, SyncSender, channel, sync_channel},
};
static ROOT: OnceLock<PathBuf> = OnceLock::new();
static BACKGROUND: AtomicBool = AtomicBool::new(false);
static PAUSED: AtomicBool = AtomicBool::new(false);
#[derive(Clone)]
struct Updates {
    target: Arc<Mutex<Option<std::sync::mpsc::Sender<Result<View>>>>>,
    last: Arc<Mutex<Option<View>>>,
}
impl Updates {
    fn send(&self, value: Result<View>) -> std::result::Result<(), ()> {
        if let Ok(view) = &value {
            *self.last.lock().unwrap() = Some(view.clone());
        }
        let mut target = self.target.lock().unwrap();
        if target.as_ref().is_some_and(|tx| tx.send(value).is_err()) {
            *target = None;
        }
        Ok(())
    }
}
struct Lease {
    scope: InstanceScope,
    alive: Arc<AtomicBool>,
    timer: Option<Timer>,
    commands: SyncSender<Command>,
    interlock: automation_guard::Interlock,
    updates: Updates,
}
static LEASE: Mutex<Option<Lease>> = Mutex::new(None);
static CARD: Mutex<Option<(String, String)>> = Mutex::new(None);
pub(crate) fn configure(root: PathBuf) {
    let _ = ROOT.set(root);
}
pub(crate) fn open(scope: InstanceScope) -> (SyncSender<Command>, Receiver<Result<View>>) {
    let (tx, commands) = sync_channel(2);
    let (target, rx) = channel();
    let updates = Updates {
        target: Arc::new(Mutex::new(Some(target))),
        last: Default::default(),
    };
    let alive = Arc::new(AtomicBool::new(true));
    let interlock = automation_guard::Interlock::default();
    {
        let mut lease = LEASE.lock().unwrap();
        if lease
            .as_ref()
            .is_some_and(|l| l.alive.load(Ordering::SeqCst))
        {
            let l = lease.as_mut().unwrap();
            l.scope = scope;
            let sender = updates.target.lock().unwrap().take();
            *l.updates.target.lock().unwrap() = sender;
            let cached = l.updates.last.lock().unwrap().clone();
            if let Some(v) = cached {
                let _ = l.updates.send(Ok(v));
            }
            return (l.commands.clone(), rx);
        }
        *lease = Some(Lease {
            scope,
            alive: alive.clone(),
            timer: None,
            commands: tx.clone(),
            interlock: interlock.clone(),
            updates: updates.clone(),
        });
    }
    let root = ROOT.get().cloned();
    std::thread::spawn(move || {
        let root = match root {
            Some(r) => r,
            None => {
                let _ = updates.send(Err("宿主配置尚未完成".into()));
                return;
            }
        };
        while alive.load(Ordering::SeqCst) {
            // Discard actions queued while logged out or while a new account is binding.
            for _ in commands.try_iter() {}
            match Controller::open(&root) {
                Ok(mut c) => {
                    c.automation_interlock = interlock.clone();
                    for _ in commands.try_iter() {}
                    let mut last_view_at = now();
                    let mut last_authorized = c.is_authorized();
                    let mut last_sync_at = std::time::Instant::now();
                    let mut last_notice = std::time::Instant::now();
                    let _ = updates.send(Ok(c.view()));
                    if let Some((room, activity_id)) = CARD.lock().unwrap().take() {
                        let _ = updates.send(Ok(c.handle(Command::OpenCard { room, activity_id })));
                    }
                    while alive.load(Ordering::SeqCst) {
                        if PAUSED.load(Ordering::SeqCst) && c.is_authorized() {
                            let view = c.handle(Command::Revoke);
                            let _ = updates.send(Ok(view));
                        }
                        if !c.host_session_current() {
                            c.shutdown();
                            let _ = updates.send(Ok(View {
                                account: "未登录".into(),
                                message: "Rinx 会话已变化，旧授权已撤销；正在等待本人重新登录。"
                                    .into(),
                                ..Default::default()
                            }));
                            break;
                        }
                        if !PAUSED.load(Ordering::SeqCst)
                            && last_notice.elapsed() >= Duration::from_secs(10)
                        {
                            c.assistance_notifications();
                            last_notice = std::time::Instant::now();
                        }
                        #[cfg(feature = "acceptance")]
                        if let Some(view) = super::acceptance::poll(&root, &mut c) {
                            let _ = updates.send(Ok(view));
                        }
                        // User changes and revocation queued during a network
                        // read take priority over the next automatic send.
                        if let Ok(command) = commands.try_recv() {
                            if matches!(command, Command::OpenCard { .. }) {
                                CARD.lock().unwrap().take();
                            }
                            let result = c.handle(command);
                            last_view_at = now();
                            last_authorized = result.authorized;
                            if updates.send(Ok(result)).is_err() {
                                break;
                            }
                            continue;
                        }
                        if !PAUSED.load(Ordering::SeqCst)
                            && last_sync_at.elapsed() >= c.sync_interval()
                        {
                            last_sync_at = std::time::Instant::now();
                            if let Some(view) = c.automatic_sync() {
                                let _ = updates.send(Ok(view));
                                last_view_at = now();
                            }
                        }
                        match commands.recv_timeout(Duration::from_millis(250)) {
                            Ok(command) => {
                                if matches!(command, Command::OpenCard { .. }) {
                                    CARD.lock().unwrap().take();
                                }
                                if !alive.load(Ordering::SeqCst) {
                                    break;
                                }
                                let result = c.handle(command);
                                last_view_at = now();
                                last_authorized = result.authorized;
                                if alive.load(Ordering::SeqCst) && updates.send(Ok(result)).is_err()
                                {
                                    break;
                                }
                            }
                            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                                if (last_authorized && !c.is_authorized())
                                    || now().saturating_sub(last_view_at) >= 10
                                {
                                    last_authorized = c.is_authorized();
                                    last_view_at = now();
                                    let _ = updates.send(Ok(c.view()));
                                }
                            }
                            Err(_) => {
                                alive.store(false, Ordering::SeqCst);
                                break;
                            }
                        }
                    }
                    c.shutdown();
                }
                Err(message) => {
                    #[cfg(feature = "full-host")]
                    if super::rinx_bridge::official_mode() {
                        let _ = super::rinx_bridge::record_status(&root, None, false, false);
                    }
                    let _ = updates.send(Ok(View {
                        account: "未登录".into(),
                        message,
                        ..Default::default()
                    }));
                }
            }
            for _ in 0..10 {
                if !alive.load(Ordering::SeqCst) {
                    break;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
        }
        alive.store(false, Ordering::SeqCst);
    });
    (tx, rx)
}
pub(crate) fn timer(scope: InstanceScope, timer: Timer) {
    if let Some(l) = LEASE.lock().unwrap().as_mut() {
        if l.scope == scope {
            l.timer = Some(timer);
        }
    }
}
pub(crate) fn close(scope: InstanceScope) -> Option<Timer> {
    let mut lease = LEASE.lock().unwrap();
    if lease.as_ref().is_some_and(|l| l.scope == scope) {
        if BACKGROUND.load(Ordering::SeqCst) {
            return lease.as_mut().unwrap().timer.take();
        }
        let l = lease.take().unwrap();
        l.interlock.invalidate_all();
        l.alive.store(false, Ordering::SeqCst);
        l.timer
    } else {
        None
    }
}
pub(crate) fn background() -> bool {
    BACKGROUND.load(Ordering::SeqCst)
}
pub(crate) fn paused() -> bool {
    PAUSED.load(Ordering::SeqCst)
}
pub(crate) fn set_background(enabled: bool) {
    BACKGROUND.store(enabled, Ordering::SeqCst);
    PAUSED.store(false, Ordering::SeqCst);
}
pub(crate) fn pause_background() {
    PAUSED.store(true, Ordering::SeqCst);
    if let Some(l) = LEASE.lock().unwrap().as_ref() {
        l.interlock.invalidate_all();
    }
}
pub(crate) fn stop() {
    BACKGROUND.store(false, Ordering::SeqCst);
    PAUSED.store(true, Ordering::SeqCst);
    if let Some(l) = LEASE.lock().unwrap().as_ref() {
        l.interlock.invalidate_all();
        l.alive.store(false, Ordering::SeqCst);
    }
}
pub(crate) fn request(command: Command) {
    if let Some(l) = LEASE.lock().unwrap().as_ref() {
        let _ = l.commands.try_send(command);
    }
}
pub(crate) fn invalidate_automation(scope: InstanceScope, activity: Option<&str>) {
    if let Some(l) = LEASE.lock().unwrap().as_ref().filter(|l| l.scope == scope) {
        if let Some(id) = activity {
            l.interlock.invalidate(id);
        } else {
            l.interlock.invalidate_all();
        }
    }
}
pub(crate) fn open_card(room: String, activity_id: String) {
    *CARD.lock().unwrap() = Some((room.clone(), activity_id.clone()));
    if let Some(lease) = LEASE.lock().unwrap().as_ref() {
        let _ = lease
            .commands
            .try_send(Command::OpenCard { room, activity_id });
    }
}
