use super::SettingsApp;
use egui::Ui;

impl SettingsApp {
    pub fn updater_tab(&mut self, ui: &mut Ui) {
        ui.heading("Auto-Updater");
        ui.add_space(8.0);

        if ui
            .checkbox(
                &mut self.config.updater.enabled,
                "Enable automatic update checks",
            )
            .changed()
        {
            self.dirty = true;
        }

        ui.add_space(8.0);
        ui.add_enabled_ui(self.config.updater.enabled, |ui| {
            ui.horizontal(|ui| {
                ui.label("Update channel:");
                let before = self.config.updater.channel.clone();
                egui::ComboBox::from_label("")
                    .selected_text(&self.config.updater.channel)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut self.config.updater.channel,
                            "stable".to_string(),
                            "Stable",
                        );
                        ui.selectable_value(
                            &mut self.config.updater.channel,
                            "beta".to_string(),
                            "Beta (pre-releases)",
                        );
                    });
                if self.config.updater.channel != before {
                    self.dirty = true;
                }
            });

            ui.horizontal(|ui| {
                ui.label("Check interval:");
                if ui
                    .add(
                        egui::Slider::new(&mut self.config.updater.check_interval_h, 1..=168)
                            .suffix(" hours"),
                    )
                    .changed()
                {
                    self.dirty = true;
                }
            });

            if !self.config.updater.last_checked.is_empty() {
                ui.label(format!(
                    "Last checked: {}",
                    self.config.updater.last_checked
                ));
            }
        });

        ui.add_space(8.0);
        ui.separator();

        ui.label(format!("Current version: v{}", env!("CARGO_PKG_VERSION")));

        ui.add_space(4.0);

        // Poll the background check result each frame and update the status label.
        if let Some(rx) = &self.update_check_rx {
            if let Ok(result) = rx.try_recv() {
                self.update_check_rx = None;
                match result {
                    Ok(Some(v)) => {
                        self.update_status = Some(format!("Update available: v{v}"));
                        // Also let the main loop fire a tray notification.
                        let _ = self.cmd_tx.send(crate::Cmd::UpdateAvailable(v));
                    }
                    Ok(None) => {
                        self.update_status = Some("You are up to date!".to_string());
                    }
                    Err(e) => {
                        self.update_status = Some(format!("Check failed: {e}"));
                    }
                }
            }
        }

        if let Some(ref status) = self.update_status.clone() {
            let color = if status.starts_with("Update available") {
                egui::Color32::YELLOW
            } else if status.starts_with("Check failed") {
                egui::Color32::RED
            } else {
                egui::Color32::GREEN
            };
            ui.colored_label(color, status);
        }

        ui.add_space(4.0);
        ui.horizontal(|ui| {
            let checking = self.update_check_rx.is_some();

            if ui
                .add_enabled(!checking, egui::Button::new("Check Now"))
                .clicked()
            {
                let channel = self.config.updater.channel.clone();
                let (tx, rx) = std::sync::mpsc::channel();
                self.update_check_rx = Some(rx);
                std::thread::spawn(move || {
                    let result =
                        crate::updater::check_for_update(&channel).map_err(|e| e.to_string());
                    let _ = tx.send(result);
                });
                self.update_status = Some("Checking...".to_string());
            }

            let update_ready = self
                .update_status
                .as_deref()
                .map_or(false, |s| s.starts_with("Update available"));

            if update_ready && ui.button("Download & Install Update").clicked() {
                let channel = self.config.updater.channel.clone();
                crate::updater::background_apply(channel);
                self.update_status = Some("Downloading update...".to_string());
            }
        });

        if self.config.updater.enabled {
            ui.add_space(2.0);
            ui.weak(format!(
                "Auto-checks every {} hour{}.",
                self.config.updater.check_interval_h,
                if self.config.updater.check_interval_h == 1 { "" } else { "s" }
            ));
        }

        ui.add_space(8.0);
        ui.label("Updates are verified with SHA-256 before applying.");
    }
}
