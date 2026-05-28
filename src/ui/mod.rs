mod about_tab;
mod discord_tab;
mod hotkeys_tab;
mod notifications_tab;
mod profiles_tab;
mod startup_tab;
mod updater_tab;

use crate::config::AppConfig;
use crate::Cmd;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};

// the settings window's egui context, stored so we can wake it up later
// None = thread not started yet, Some = running (possibly hidden)
static SETTINGS_CTX: Mutex<Option<egui::Context>> = Mutex::new(None);
// set to true to tell the window to show itself
static SETTINGS_SHOW: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Hotkeys,
    Profiles,
    Startup,
    Notifications,
    Updater,
    Discord,
    About,
}

impl Tab {
    pub fn all() -> &'static [Tab] {
        &[
            Tab::Hotkeys,
            Tab::Profiles,
            Tab::Startup,
            Tab::Notifications,
            Tab::Updater,
            Tab::Discord,
            Tab::About,
        ]
    }

    pub fn label(self) -> &'static str {
        match self {
            Tab::Hotkeys => "Hotkeys",
            Tab::Profiles => "Profiles",
            Tab::Startup => "Startup",
            Tab::Notifications => "Notifications",
            Tab::Updater => "Updater",
            Tab::Discord => "Discord",
            Tab::About => "About",
        }
    }
}

pub struct SettingsApp {
    pub config: AppConfig,
    pub config_shared: Arc<Mutex<AppConfig>>,
    pub cmd_tx: mpsc::Sender<Cmd>,
    pub current_tab: Tab,
    pub update_status: Option<String>,
    /// Receives the result of a background update check so the UI can show it.
    pub update_check_rx: Option<mpsc::Receiver<Result<Option<String>, String>>>,
    pub startup_registered: bool,
    pub startup_error: Option<String>,
    /// Set to true by any tab that modifies config; triggers a save at end of frame.
    pub dirty: bool,
}

impl SettingsApp {
    pub fn new(config_shared: Arc<Mutex<AppConfig>>, cmd_tx: mpsc::Sender<Cmd>) -> Self {
        let config = config_shared.lock().unwrap().clone();
        let startup_registered = crate::startup::is_registered();
        Self {
            config,
            config_shared,
            cmd_tx,
            current_tab: Tab::Hotkeys,
            update_status: None,
            update_check_rx: None,
            startup_registered,
            startup_error: None,
            dirty: false,
        }
    }

    // save config to disk and tell the main loop about the change
    pub fn save_now(&mut self) {
        if let Err(e) = crate::config::save_config(&self.config) {
            eprintln!("Failed to save config: {e}");
        }
        {
            let mut shared = self.config_shared.lock().unwrap();
            *shared = self.config.clone();
        }
        let _ = self.cmd_tx.send(Cmd::ConfigUpdated(self.config.clone()));
    }
}

impl eframe::App for SettingsApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // if the main thread wants us to show, restore and focus
        if SETTINGS_SHOW.swap(false, Ordering::SeqCst) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        }

        // when the user clicks X, hide instead of closing
        // thread stays alive so reopening is instant and the window doesn't ghost in the taskbar
        if ctx.input(|i| i.viewport().close_requested()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
            return;
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                for &tab in Tab::all() {
                    ui.selectable_value(&mut self.current_tab, tab, tab.label());
                }
            });
            ui.separator();

            egui::ScrollArea::vertical().show(ui, |ui| match self.current_tab {
                Tab::Hotkeys => self.hotkeys_tab(ui),
                Tab::Profiles => self.profiles_tab(ui),
                Tab::Startup => self.startup_tab(ui),
                Tab::Notifications => self.notifications_tab(ui),
                Tab::Updater => self.updater_tab(ui),
                Tab::Discord => self.discord_tab(ui),
                Tab::About => self.about_tab(ui),
            });
        });

        // auto-save if something changed, but skip if hotkeys are duped
        if self.dirty {
            self.dirty = false;
            let h = &self.config.hotkeys;
            let hotkeys_ok = h.icons != h.taskbar && h.icons != h.windows && h.taskbar != h.windows;
            if hotkeys_ok {
                self.save_now();
            }
        }
    }
}

// open the settings window, or restore it if it's already running
pub fn open_settings(config_shared: Arc<Mutex<AppConfig>>, cmd_tx: mpsc::Sender<Cmd>) {
    // already running, just tell it to show itself
    let existing = SETTINGS_CTX.lock().unwrap().clone();
    if let Some(ctx) = existing {
        SETTINGS_SHOW.store(true, Ordering::SeqCst);
        ctx.request_repaint();
        return;
    }

    std::thread::spawn(move || {
        // Clear the stored context when this thread exits for any reason.
        struct Guard;
        impl Drop for Guard {
            fn drop(&mut self) {
                *SETTINGS_CTX.lock().unwrap() = None;
                SETTINGS_SHOW.store(false, Ordering::SeqCst);
            }
        }
        let _guard = Guard;

        let icon = {
            let rgba = crate::tray::build_icon_rgba(&crate::state::AppState::default());
            let size = crate::tray::ICON_SIZE as u32;
            std::sync::Arc::new(egui::IconData {
                rgba,
                width: size,
                height: size,
            })
        };

        let native_options = eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_title("HideDesktopApps Settings")
                .with_inner_size([600.0, 480.0])
                .with_resizable(true)
                .with_icon(icon),
            event_loop_builder: Some(Box::new(|builder| {
                use winit::platform::windows::EventLoopBuilderExtWindows;
                builder.with_any_thread(true);
            })),
            ..Default::default()
        };

        let result = eframe::run_native(
            "HideDesktopApps Settings",
            native_options,
            Box::new(move |cc| {
                // save the context so open_settings can wake us up later
                *SETTINGS_CTX.lock().unwrap() = Some(cc.egui_ctx.clone());
                Ok(Box::new(SettingsApp::new(config_shared, cmd_tx)))
            }),
        );

        if let Err(e) = result {
            crate::dlog!("Settings window error: {}", e);
            eprintln!("Settings window error: {e}");
        }
    });
}
