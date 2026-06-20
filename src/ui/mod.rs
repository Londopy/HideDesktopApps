mod about_tab;
mod discord_tab;
mod general_tab;
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
    General,
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
            Tab::General,
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
            Tab::General => "General",
            Tab::About => "About",
        }
    }
}

// which hotkey field the recorder is currently capturing
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotkeyField {
    Icons,
    Taskbar,
    Windows,
    Profile(usize),
}

// a small colored "shown/hidden" chip for the status header
fn status_chip(ui: &mut egui::Ui, name: &str, hidden: bool) {
    let (text, color) = if hidden {
        (
            format!("{name}: hidden"),
            egui::Color32::from_rgb(231, 76, 60),
        )
    } else {
        (
            format!("{name}: shown"),
            egui::Color32::from_rgb(46, 204, 113),
        )
    };
    ui.colored_label(color, text);
}

// available settings-window themes
pub const THEMES: &[&str] = &["Dark", "Light", "Ocean", "Rose", "Forest"];

// apply the chosen theme to the settings window's visuals
fn apply_theme(ctx: &egui::Context, theme: &str) {
    let mut v = if theme == "Light" {
        egui::Visuals::light()
    } else {
        egui::Visuals::dark()
    };
    match theme {
        "Ocean" => {
            v.hyperlink_color = egui::Color32::from_rgb(0, 170, 255);
            v.selection.bg_fill = egui::Color32::from_rgb(0, 90, 160);
        }
        "Rose" => {
            v.hyperlink_color = egui::Color32::from_rgb(255, 110, 170);
            v.selection.bg_fill = egui::Color32::from_rgb(150, 40, 90);
        }
        "Forest" => {
            v.hyperlink_color = egui::Color32::from_rgb(120, 220, 120);
            v.selection.bg_fill = egui::Color32::from_rgb(40, 120, 60);
        }
        _ => {}
    }
    ctx.set_visuals(v);
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
    /// Live app state, for the status header.
    pub state_shared: Arc<Mutex<crate::state::AppState>>,
    /// Which hotkey field (if any) is currently being recorded.
    pub recording_hotkey: Option<HotkeyField>,
    /// Status line for settings import/export.
    pub backup_status: Option<String>,
}

impl SettingsApp {
    pub fn new(
        config_shared: Arc<Mutex<AppConfig>>,
        state_shared: Arc<Mutex<crate::state::AppState>>,
        cmd_tx: mpsc::Sender<Cmd>,
    ) -> Self {
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
            state_shared,
            recording_hotkey: None,
            backup_status: None,
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
        let _ = self.cmd_tx.send(Cmd::ConfigUpdated(Box::new(self.config.clone())));
    }
}

impl eframe::App for SettingsApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        apply_theme(ctx, &self.config.behavior.theme);

        // if the main thread wants us to show, restore and focus
        if SETTINGS_SHOW.swap(false, Ordering::SeqCst) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        }

        // when the user clicks X, minimize instead of closing
        // thread stays alive so reopening is instant
        // window doesn't show in taskbar because we used with_taskbar(false)
        if ctx.input(|i| i.viewport().close_requested()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
            return;
        }

        // keep the live status header fresh while the window is open
        ctx.request_repaint_after(std::time::Duration::from_millis(500));

        egui::CentralPanel::default().show(ctx, |ui| {
            // live status of what's currently hidden
            {
                let st = self.state_shared.lock().unwrap();
                ui.horizontal(|ui| {
                    ui.label("Status:");
                    status_chip(ui, "Icons", st.icons_hidden);
                    status_chip(ui, "Taskbar", st.taskbar_hidden);
                    status_chip(ui, "Windows", st.windows_hidden);
                    if let Some(p) = &st.active_profile {
                        ui.separator();
                        ui.label(format!("Profile: {p}"));
                    }
                });
            }
            ui.separator();

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
                Tab::General => self.general_tab(ui),
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
pub fn open_settings(
    config_shared: Arc<Mutex<AppConfig>>,
    state_shared: Arc<Mutex<crate::state::AppState>>,
    cmd_tx: mpsc::Sender<Cmd>,
) {
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
                .with_icon(icon)
                // keep it out of the taskbar — we show/hide by minimizing
                .with_taskbar(false),
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
                Ok(Box::new(SettingsApp::new(
                    config_shared,
                    state_shared,
                    cmd_tx,
                )))
            }),
        );

        if let Err(e) = result {
            crate::dlog!("Settings window error: {}", e);
            eprintln!("Settings window error: {e}");
        }
    });
}
