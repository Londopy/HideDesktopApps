use super::SettingsApp;
use egui::Ui;

impl SettingsApp {
    pub fn updater_tab(&mut self, ui: &mut Ui) {
        ui.heading("Auto-Updater");
        ui.add_space(8.0);

        ui.checkbox(
            &mut self.config.updater.enabled,
            "Enable automatic update checks",
        );

        ui.add_space(8.0);
        ui.add_enabled_ui(self.config.updater.enabled, |ui| {
            ui.horizontal(|ui| {
                ui.label("Update channel:");
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
            });

            ui.horizontal(|ui| {
                ui.label("Check interval:");
                ui.add(
                    egui::Slider::new(&mut self.config.updater.check_interval_h, 1..=168)
                        .suffix(" hours"),
                );
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

        let current_version = env!("CARGO_PKG_VERSION");
        ui.label(format!("Current version: {}", current_version));

        if let Some(ref status) = self.update_status.clone() {
            ui.colored_label(egui::Color32::GREEN, status);
        }

        ui.add_space(4.0);
        ui.horizontal(|ui| {
            if ui.button("Check Now").clicked() {
                let channel = self.config.updater.channel.clone();
                let tx = self.cmd_tx.clone();
                std::thread::spawn(move || match crate::updater::check_for_update(&channel) {
                    Ok(Some(v)) => {
                        let _ = tx.send(crate::Cmd::UpdateAvailable(v));
                    }
                    Ok(None) => {
                        let _ = tx.send(crate::Cmd::UpToDate);
                    }
                    Err(e) => {
                        eprintln!("Update check failed: {e}");
                    }
                });
                self.update_status = Some("Checking...".to_string());
            }

            if ui.button("Download & Install Update").clicked() {
                let channel = self.config.updater.channel.clone();
                crate::updater::background_apply(channel);
                self.update_status = Some("Downloading update...".to_string());
            }
        });

        ui.add_space(8.0);
        ui.label("Updates are verified with SHA-256 before applying.");
    }
}
