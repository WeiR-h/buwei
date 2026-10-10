//! Official desktop packaging seam, following the phone package's wrapper.
use makepad_app_module::AppModule;
use makepad_widgets::*;
use octosense_shell::App as ShellApp;
script_mod! {use mod.prelude.widgets.* use mod.widgets.* startup() do #(App::script_component(vm)){ui:mod.widgets.OctoSenseRoot{}}}
#[derive(Script, ScriptHook)]
pub struct App {
    #[deref]
    shell: ShellApp,
}
fn linked_modules() -> Vec<&'static dyn AppModule> {
    vec![&super::gui::MODULE]
}
fn trusted(module: &dyn AppModule) -> bool {
    std::ptr::eq(module, &super::gui::MODULE as &dyn AppModule)
}
impl App {
    fn focus_buwei(&mut self, cx: &mut Cx) {
        let existing = self.shell.state.as_ref().and_then(|state| {
            state
                .clients
                .iter()
                .find(|(_, slot)| slot.app == "buwei" && slot.closing.is_none() && !slot.warm)
                .map(|(id, _)| *id)
        });
        if let Some(client) = existing {
            if let Some(window) = self
                .shell
                .state
                .as_mut()
                .and_then(|state| state.layout.desktop.get_mut(client))
            {
                window.minimized = false;
            }
            self.shell.activate_client(cx, client);
        } else {
            let app = octosense_shell::clients::AppDef {
                id: "buwei".into(),
                label: "补位".into(),
                bin: "buwei-matrix-host".into(),
                package: "buwei-matrix-host".into(),
                dir: ".".into(),
                manifest: None,
                args: vec![],
                policy: octosense_shell::clients::LaunchPolicy::OrFocus,
                target_dir: None,
            };
            self.shell.launch_module_as(cx, &super::gui::MODULE, &app);
        }
    }
}
impl AppMain for App {
    fn script_mod(vm: &mut ScriptVm) -> ScriptValue {
        octosense_shell::ext::install(octosense_shell::ext::Ext {
            linked_modules,
            trusted_module: trusted,
        });
        ShellApp::shell_script_mod(vm);
        self::script_mod(vm)
    }
    fn handle_event(&mut self, cx: &mut Cx, event: &Event) {
        if matches!(event, Event::Shutdown) {
            super::host::stop();
            super::tray::disable();
        }
        if let Event::WindowCloseRequested(e) = event {
            if self.shell.ui.window(cx, ids!(main_window)).window_id() == Some(e.window_id) {
                if super::host::background() {
                    e.accept_close.set(false);
                    if !super::tray::hide() {
                        super::host::set_background(false);
                        super::host::pause_background();
                        super::host::request(super::controller::Command::Revoke);
                        super::tray::disable();
                        log!("[buwei-tray] window kept open; background permission paused");
                    }
                    return;
                }
                super::host::stop();
                super::tray::disable();
            }
        }
        if matches!(event, Event::Signal) {
            for action in super::tray::take_actions() {
                match action {
                    super::tray::TrayAction::Open => {
                        super::tray::show();
                        self.focus_buwei(cx);
                    }
                    super::tray::TrayAction::Todos => {
                        super::tray::show();
                        self.focus_buwei(cx);
                        super::host::request(super::controller::Command::ShowTodos);
                    }
                    super::tray::TrayAction::Pause => {
                        super::host::request(super::controller::Command::Revoke);
                        super::tray::show();
                    }
                    super::tray::TrayAction::Exit => {
                        super::host::stop();
                        super::tray::disable();
                        cx.quit();
                        return;
                    }
                }
            }
        }
        self.shell.shell_handle_event(cx, event);
        #[cfg(feature = "full-host")]
        if let Event::Actions(actions) = event {
            for action in actions {
                if let Some(rinx::mini_app::MiniAppAction::BuWeiOpen { room, activity_id }) =
                    action.downcast_ref::<rinx::mini_app::MiniAppAction>()
                {
                    let app = octosense_shell::clients::AppDef {
                        id: "buwei".into(),
                        label: "补位".into(),
                        bin: "buwei-matrix-host".into(),
                        package: "buwei-matrix-host".into(),
                        dir: ".".into(),
                        manifest: None,
                        args: vec![],
                        policy: octosense_shell::clients::LaunchPolicy::OrFocus,
                        target_dir: None,
                    };
                    let existing = self.shell.state.as_ref().and_then(|state| {
                        state
                            .clients
                            .iter()
                            .find(|(_, slot)| {
                                slot.app == "buwei" && slot.closing.is_none() && !slot.warm
                            })
                            .map(|(id, _)| *id)
                    });
                    if let Some(client) = existing {
                        if let Some(window) = self
                            .shell
                            .state
                            .as_mut()
                            .and_then(|state| state.layout.desktop.get_mut(client))
                        {
                            window.minimized = false;
                        }
                        self.shell.activate_client(cx, client);
                    } else {
                        self.shell.launch_module_as(cx, &super::gui::MODULE, &app);
                    }
                    super::host::open_card(room.clone(), activity_id.clone());
                }
            }
        }
        if matches!(event, Event::Startup) {
            #[cfg(feature = "full-host")]
            if super::rinx_bridge::official_mode() {
                let registry = octosense_shell::apps::AppRegistry::default();
                if let Some(module) = registry.module("rinx") {
                    let app = octosense_shell::clients::AppDef {
                        id: "rinx".into(),
                        label: "Rinx · 正式账号登录".into(),
                        bin: "rinx".into(),
                        package: "rinx".into(),
                        dir: ".".into(),
                        manifest: None,
                        args: vec![],
                        policy: octosense_shell::clients::LaunchPolicy::OrFocus,
                        target_dir: None,
                    };
                    self.shell.launch_module_as(cx, module, &app);
                }
            }
            let app = octosense_shell::clients::AppDef {
                id: "buwei".into(),
                label: "补位".into(),
                bin: "buwei-matrix-host".into(),
                package: "buwei-matrix-host".into(),
                dir: ".".into(),
                manifest: None,
                args: vec![],
                policy: octosense_shell::clients::LaunchPolicy::OrFocus,
                target_dir: None,
            };
            self.shell.launch_module_as(cx, &super::gui::MODULE, &app);
            if let Some(state) = self.shell.state.as_mut() {
                state
                    .layout
                    .toggle_fullscreen_mode(octosense_shell::layout::FullscreenMode::Maximized);
            }
            self.shell.ui.redraw(cx);
        }
    }
}
octosense_shell::octosense_main!();
pub(crate) fn run() {
    main()
}
