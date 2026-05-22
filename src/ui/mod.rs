mod about_tab;
mod discord_tab;
mod hotkeys_tab;
mod notifications_tab;
mod profiles_tab;
mod startup_tab;
mod updater_tab;

use crate::config::AppConfig;
use crate::Cmd;
use std::sync::{Arc, Mutex};

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
    pub hotkey_error: Option<String>,
    pub update_status: Option<String>,
    pub startup_registered: bool,
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
            hotkey_error: None,
            update_status: None,
            startup_registered,
        }
    }

    fn apply(&mut self) {
        // Validate hotkeys are unique
        let h = &self.config.hotkeys;
        if h.icons == h.taskbar || h.icons == h.windows || h.taskbar == h.windows {
            self.hotkey_error = Some("All three hotkeys must be unique.".to_string());
            return;
        }
        self.hotkey_error = None;

        // Persist config to disk
        if let Err(e) = crate::config::save_config(&self.config) {
            eprintln!("Failed to save config: {e}");
        }

        // Update the shared config so other threads can see it
        {
            let mut shared = self.config_shared.lock().unwrap();
            *shared = self.config.clone();
        }

        // Notify main loop
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

            ui.separator();
            if let Some(ref err) = self.hotkey_error.clone() {
                ui.colored_label(egui::Color32::RED, err);
            }
            ui.horizontal(|ui| {
                if ui.button("Apply & Save").clicked() {
                    self.apply();
                }
                if ui.button("Cancel").clicked() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
        });
    }
}

/// Open the Settings window in a new thread.
pub fn open_settings(config_shared: Arc<Mutex<AppConfig>>, cmd_tx: std::sync::mpsc::Sender<Cmd>) {
    std::thread::spawn(move || {
        let native_options = eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_title("HideDesktopApps Settings")
                .with_inner_size([600.0, 480.0])
                .with_resizable(true),
            ..Default::default()
        };

        let result = eframe::run_native(
            "HideDesktopApps Settings",
            native_options,
            Box::new(move |_cc| Ok(Box::new(SettingsApp::new(config_shared, cmd_tx)))),
        );

        if let Err(e) = result {
            crate::dlog!("Settings window error: {e}");
            eprintln!("Settings window error: {e}");
        }
    });
}
