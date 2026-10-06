//! Explicitly enabled native tray. Its menu only requests host/UI actions.
use std::sync::{
    Mutex,
    atomic::{AtomicBool, AtomicIsize, Ordering},
};
#[derive(Clone, Copy)]
pub(crate) enum TrayAction {
    Open,
    Todos,
    Pause,
    Exit,
}
static ACTIONS: Mutex<Vec<TrayAction>> = Mutex::new(Vec::new());
static ENABLED: AtomicBool = AtomicBool::new(false);
static MAIN: AtomicIsize = AtomicIsize::new(0);
static ICON: AtomicIsize = AtomicIsize::new(0);
pub(crate) fn enabled() -> bool {
    ENABLED.load(Ordering::SeqCst)
}
pub(crate) fn take_actions() -> Vec<TrayAction> {
    std::mem::take(&mut *ACTIONS.lock().unwrap())
}
fn push(a: TrayAction) {
    ACTIONS.lock().unwrap().push(a);
    makepad_widgets::SignalToUI::set_ui_signal();
}
fn dispatch(a: TrayAction) {
    match a {
        TrayAction::Pause => super::host::pause_background(),
        TrayAction::Exit => super::host::stop(),
        _ => {}
    }
    push(a);
}
#[cfg(feature = "acceptance")]
pub(crate) fn test_action(name: &str) -> Result<(), String> {
    if !std::env::args().any(|a| a == "--acceptance") {
        return Err("未开启隔离验收入口".into());
    }
    if name == "close_window" {
        #[cfg(windows)]
        return if native::request_close() {
            Ok(())
        } else {
            Err("本人的值守窗口不可关闭".into())
        };
        #[cfg(not(windows))]
        return Err("此验收需要 Windows".into());
    }
    dispatch(match name {
        "open" => TrayAction::Open,
        "todos" => TrayAction::Todos,
        "pause" => TrayAction::Pause,
        "exit" => TrayAction::Exit,
        _ => return Err("托盘动作不符合约定".into()),
    });
    Ok(())
}
#[cfg(windows)]
mod native {
    use super::*;
    use windows_sys::Win32::{
        Foundation::*,
        System::LibraryLoader::GetModuleHandleW,
        UI::{Input::KeyboardAndMouse::GetActiveWindow, Shell::*, WindowsAndMessaging::*},
    };
    const CALLBACK: u32 = WM_APP + 87;
    // shellapi.h defines this composite callback code as NIN_SELECT | NINF_KEY.
    const NIN_KEYSELECT_CODE: u32 = NIN_SELECT | 1;
    static TASKBAR_MESSAGE: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    unsafe fn add_icon(hwnd: HWND) -> bool {
        unsafe {
            let mut d = data(hwnd);
            d.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
            d.uCallbackMessage = CALLBACK;
            d.hIcon = LoadIconW(std::ptr::null_mut(), IDI_APPLICATION);
            copy(&mut d.szTip, "补位 · 已开启值守（授权到期停止）");
            if Shell_NotifyIconW(NIM_ADD, &d) == 0 {
                return false;
            }
            d.Anonymous.uVersion = NOTIFYICON_VERSION_4;
            Shell_NotifyIconW(NIM_SETVERSION, &d);
            true
        }
    }
    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(Some(0)).collect()
    }
    fn copy<const N: usize>(out: &mut [u16; N], s: &str) {
        for (o, c) in out.iter_mut().take(N - 1).zip(s.encode_utf16()) {
            *o = c;
        }
    }
    unsafe fn data(hwnd: HWND) -> NOTIFYICONDATAW {
        let mut d: NOTIFYICONDATAW = unsafe { std::mem::zeroed() };
        d.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
        d.hWnd = hwnd;
        d.uID = 1;
        d
    }
    unsafe extern "system" fn window(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
        unsafe {
            if msg != 0 && msg == TASKBAR_MESSAGE.load(Ordering::SeqCst) {
                if !add_icon(hwnd) {
                    show();
                    ENABLED.store(false, Ordering::SeqCst);
                    super::super::host::set_background(false);
                }
                return 0;
            }
            if msg == CALLBACK {
                match l as u32 & 0xffff {
                    WM_LBUTTONUP | WM_LBUTTONDBLCLK | NIN_SELECT | NIN_KEYSELECT_CODE => {
                        dispatch(TrayAction::Open)
                    }
                    WM_RBUTTONUP | WM_CONTEXTMENU => {
                        let menu = CreatePopupMenu();
                        if !menu.is_null() {
                            for (id, label) in [
                                (1, "打开补位"),
                                (2, "查看待办"),
                                (3, "暂停值守并撤销授权"),
                                (4, "退出补位"),
                            ] {
                                AppendMenuW(menu, MF_STRING, id, wide(label).as_ptr());
                            }
                            let mut p: POINT = std::mem::zeroed();
                            GetCursorPos(&mut p);
                            SetForegroundWindow(hwnd);
                            let picked = TrackPopupMenu(
                                menu,
                                TPM_RETURNCMD | TPM_NONOTIFY,
                                p.x,
                                p.y,
                                0,
                                hwnd,
                                std::ptr::null(),
                            ) as u32;
                            DestroyMenu(menu);
                            match picked {
                                1 => dispatch(TrayAction::Open),
                                2 => dispatch(TrayAction::Todos),
                                3 => dispatch(TrayAction::Pause),
                                4 => dispatch(TrayAction::Exit),
                                _ => {}
                            }
                            PostMessageW(hwnd, WM_NULL, 0, 0);
                        }
                    }
                    _ => {}
                }
                return 0;
            }
            if msg == WM_CLOSE {
                DestroyWindow(hwnd);
                return 0;
            }
            if msg == WM_DESTROY {
                Shell_NotifyIconW(NIM_DELETE, &data(hwnd));
                ICON.store(0, Ordering::SeqCst);
                ENABLED.store(false, Ordering::SeqCst);
                PostQuitMessage(0);
                return 0;
            }
            DefWindowProcW(hwnd, msg, w, l)
        }
    }
    pub(super) fn enable() -> bool {
        let main = unsafe { GetActiveWindow() };
        if main.is_null() {
            return false;
        }
        MAIN.store(main as isize, Ordering::SeqCst);
        if ICON.load(Ordering::SeqCst) != 0 {
            ENABLED.store(true, Ordering::SeqCst);
            return true;
        }
        let (tx, rx) = std::sync::mpsc::channel();
        let cancelled = std::sync::Arc::new(AtomicBool::new(false));
        let thread_cancelled = cancelled.clone();
        std::thread::spawn(move || unsafe {
            TASKBAR_MESSAGE.store(
                RegisterWindowMessageW(wide("TaskbarCreated").as_ptr()),
                Ordering::SeqCst,
            );
            let class = wide("BuWeiTray2026");
            let instance = GetModuleHandleW(std::ptr::null());
            let wc = WNDCLASSW {
                lpfnWndProc: Some(window),
                hInstance: instance,
                lpszClassName: class.as_ptr(),
                ..std::mem::zeroed()
            };
            RegisterClassW(&wc);
            let hwnd = CreateWindowExW(
                0,
                class.as_ptr(),
                wide("补位值守").as_ptr(),
                0,
                0,
                0,
                0,
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                instance,
                std::ptr::null(),
            );
            if hwnd.is_null() {
                let _ = tx.send(false);
                return;
            }
            if thread_cancelled.load(Ordering::SeqCst) || !add_icon(hwnd) {
                DestroyWindow(hwnd);
                let _ = tx.send(false);
                return;
            }
            ICON.store(hwnd as isize, Ordering::SeqCst);
            ENABLED.store(true, Ordering::SeqCst);
            if thread_cancelled.load(Ordering::SeqCst) || tx.send(true).is_err() {
                DestroyWindow(hwnd);
                return;
            }
            let mut msg: MSG = std::mem::zeroed();
            while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        });
        match rx.recv_timeout(std::time::Duration::from_secs(3)) {
            Ok(ok) => ok,
            Err(_) => {
                cancelled.store(true, Ordering::SeqCst);
                disable();
                false
            }
        }
    }
    pub(super) fn hide() -> bool {
        let h = MAIN.load(Ordering::SeqCst) as HWND;
        if !enabled() || h.is_null() {
            return false;
        }
        unsafe {
            let mut identifier: NOTIFYICONIDENTIFIER = std::mem::zeroed();
            identifier.cbSize = std::mem::size_of::<NOTIFYICONIDENTIFIER>() as u32;
            identifier.hWnd = ICON.load(Ordering::SeqCst) as HWND;
            identifier.uID = 1;
            let mut rect: RECT = std::mem::zeroed();
            if Shell_NotifyIconGetRect(&identifier, &mut rect) != 0 {
                return false;
            }
            ShowWindow(h, SW_HIDE);
        }
        true
    }
    pub(super) fn show() {
        let h = MAIN.load(Ordering::SeqCst) as HWND;
        if !h.is_null() {
            unsafe {
                ShowWindow(h, SW_RESTORE);
                SetForegroundWindow(h);
            }
        }
    }
    #[cfg(feature = "acceptance")]
    pub(super) fn request_close() -> bool {
        let h = MAIN.load(Ordering::SeqCst) as HWND;
        enabled()
            && !h.is_null()
            && unsafe { IsWindow(h) != 0 && PostMessageW(h, WM_CLOSE, 0, 0) != 0 }
    }
    pub(super) fn hidden() -> bool {
        let h = MAIN.load(Ordering::SeqCst) as HWND;
        enabled() && !h.is_null() && unsafe { IsWindow(h) != 0 && IsWindowVisible(h) == 0 }
    }
    pub(super) fn disable() {
        ENABLED.store(false, Ordering::SeqCst);
        let h = ICON.load(Ordering::SeqCst) as HWND;
        if !h.is_null() {
            unsafe {
                PostMessageW(h, WM_CLOSE, 0, 0);
            }
        }
        show();
    }
    pub(super) fn notify(title: &str, text: &str) -> bool {
        let h = ICON.load(Ordering::SeqCst) as HWND;
        if h.is_null() || !enabled() {
            return false;
        }
        unsafe {
            let mut d = data(h);
            d.uFlags = NIF_INFO;
            d.dwInfoFlags = NIIF_INFO | NIIF_RESPECT_QUIET_TIME;
            copy(&mut d.szInfoTitle, title);
            copy(&mut d.szInfo, text);
            Shell_NotifyIconW(NIM_MODIFY, &d) != 0
        }
    }
}
pub(crate) fn enable() -> bool {
    #[cfg(windows)]
    {
        native::enable()
    }
    #[cfg(not(windows))]
    {
        false
    }
}
pub(crate) fn hide() -> bool {
    #[cfg(windows)]
    {
        native::hide()
    }
    #[cfg(not(windows))]
    {
        false
    }
}
pub(crate) fn show() {
    #[cfg(windows)]
    native::show();
}
#[cfg(feature = "acceptance")]
pub(crate) fn hidden() -> bool {
    #[cfg(windows)]
    {
        native::hidden()
    }
    #[cfg(not(windows))]
    {
        false
    }
}
pub(crate) fn disable() {
    #[cfg(windows)]
    native::disable();
}
pub(crate) fn notify(title: &str, text: &str) -> bool {
    #[cfg(windows)]
    {
        native::notify(title, text)
    }
    #[cfg(not(windows))]
    {
        let _ = (title, text);
        false
    }
}
