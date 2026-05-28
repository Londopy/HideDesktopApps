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
use std::sync::{Arc, Mutex};

// Prevents opening a second settings window while one is already showing.
static SETTINGS_OPEN: AtomicBool = AtomicBool::new(false);

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
    pub cmd_tx: std::sync::mpsc::Sender<Cmd>,
    pub current_tab: Tab,
    pub update_status: Option<String>,
    pub startup_registered: bool,
    pub startup_error: Option<String>,
    /// Set to true by any tab that modifies config; triggers a save at end of frame.
    pub dirty: bool,
}

impl SettingsApp {
    pub fn new(config_shared: Arc<Mutex<AppConfig>>, cmd_tx: std::sync::mpsc::Sender<Cmd>) -> Self {
        let config = config_shared.lock().unwrap().clone();
        let startup_registered = crate::startup::is_registered();
        Self {
            config,
            config_shared,
            cmd_tx,
            current_tab: Tab::Hotkeys,
            update_status: None,
            startup_registered,
            startup_error: None,
            dirty: false,
        }
    }

    /// Persist the current config to disk and notify the main loop.
    /// Does not validate hotkeys — the hotkeys tab handles that inline.
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

        // Auto-save at end of frame if anything changed.
        // Skip if hotkeys are currently invalid (duplicate) — the tab shows a warning.
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

/// Open the settings window on a background thread so the main loop keeps running.
/// If a settings window is already open, this is a no-op.
pub fn open_settings(config_shared: Arc<Mutex<AppConfig>>, cmd_tx: std::sync::mpsc::Sender<Cmd>) {
    if SETTINGS_OPEN.swap(true, Ordering::SeqCst) {
        return;
    }

    std::thread::spawn(move || {
        // Resets the flag when dropped — even if run_native panics.
        struct OpenGuard;
        impl Drop for OpenGuard {
            fn drop(&mut self) {
                SETTINGS_OPEN.store(false, Ordering::SeqCst);
            }
        }
        let _guard = OpenGuard;

        let native_options = eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_title("HideDesktopApps Settings")
                .with_inner_size([600.0, 480.0])
                .with_resizable(true),
            event_loop_builder: Some(Box::new(|builder| {
                use winit::platform::windows::EventLoopBuilderExtWindows;
                builder.with_any_thread(true);
            })),
            ..Default::default()
        };

        let result = eframe::run_native(
            "HideDesktopApps Settings",
            native_options,
            Box::new(move |_cc| Ok(Box::new(SettingsApp::new(config_shared, cmd_tx)))),
        );

        if let Err(e) = result {
            crate::dlog!("Settings window error: {}", e);
            eprintln!("Settings window error: {e}");
        }
        // _guard drops here, resetting SETTINGS_OPEN
    });
}
