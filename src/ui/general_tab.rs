use super::{SettingsApp, THEMES};
use egui::Ui;

impl SettingsApp {
    pub fn general_tab(&mut self, ui: &mut Ui) {
        ui.heading("General");
        ui.add_space(8.0);

        // Theme picker (applies to this settings window).
        egui::ComboBox::from_label("Theme")
            .selected_text(self.config.behavior.theme.clone())
            .show_ui(ui, |ui| {
                for &t in THEMES {
                    if ui
                        .selectable_value(&mut self.config.behavior.theme, t.to_string(), t)
                        .clicked()
                    {
                        self.dirty = true;
                    }
                }
            });

        ui.add_space(8.0);
        ui.separator();

        // Auto-hide on fullscreen.
        if ui
            .checkbox(
                &mut self.config.behavior.auto_hide_fullscreen,
                "Auto-hide icons & taskbar when a fullscreen app is focused",
            )
            .changed()
        {
            self.dirty = true;
        }

        ui.add_space(8.0);
        ui.separator();

        // Settings backup / restore.
        ui.label("Settings backup:");
        ui.horizontal(|ui| {
            if ui.button("Export\u{2026}").clicked() {
                self.export_settings();
            }
            if ui.button("Import\u{2026}").clicked() {
                self.import_settings();
            }
        });
        if let Some(msg) = &self.backup_status {
            ui.add_space(4.0);
            ui.label(msg);
        }
    }

    fn export_settings(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .set_file_name("HideDesktopApps-config.toml")
            .add_filter("TOML", &["toml"])
            .save_file()
        else {
            return;
        };
        match toml::to_string_pretty(&self.config) {
            Ok(text) => match std::fs::write(&path, text) {
                Ok(()) => self.backup_status = Some(format!("Exported to {}", path.display())),
                Err(e) => self.backup_status = Some(format!("Export failed: {e}")),
            },
            Err(e) => self.backup_status = Some(format!("Serialize failed: {e}")),
        }
    }

    fn import_settings(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("TOML", &["toml"])
            .pick_file()
        else {
            return;
        };
        match std::fs::read_to_string(&path) {
            Ok(text) => match toml::from_str::<crate::config::AppConfig>(&text) {
                Ok(cfg) => {
                    self.config = cfg;
                    self.save_now();
                    self.backup_status = Some("Imported settings.".to_string());
                }
                Err(e) => self.backup_status = Some(format!("Import failed: {e}")),
            },
            Err(e) => self.backup_status = Some(format!("Read failed: {e}")),
        }
    }
}
