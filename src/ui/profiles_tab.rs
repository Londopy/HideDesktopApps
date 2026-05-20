use super::SettingsApp;
use crate::config::ProfileConfig;
use egui::Ui;

impl SettingsApp {
    pub fn profiles_tab(&mut self, ui: &mut Ui) {
        ui.heading("Profiles");
        ui.label("Define preset hide/show combinations activated by a hotkey.");
        ui.add_space(8.0);

        let mut to_delete: Option<usize> = None;

        for (i, profile) in self.config.profiles.iter_mut().enumerate() {
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.label("Name:");
                    ui.text_edit_singleline(&mut profile.name);
                    if ui
                        .add(egui::Button::new("Remove").fill(egui::Color32::DARK_RED))
                        .clicked()
                    {
                        to_delete = Some(i);
                    }
                });

                ui.horizontal(|ui| {
                    ui.label("Hotkey:");
                    ui.text_edit_singleline(&mut profile.hotkey);
                    ui.label("(empty = none, e.g. ctrl+alt+1)");
                });

                ui.horizontal(|ui| {
                    ui.checkbox(&mut profile.icons, "Hide Icons");
                    ui.checkbox(&mut profile.taskbar, "Hide Taskbar");
                    ui.checkbox(&mut profile.windows, "Hide Windows");
                });
            });
            ui.add_space(4.0);
        }

        if let Some(idx) = to_delete {
            self.config.profiles.remove(idx);
        }

        if ui.button("+ Add Profile").clicked() {
            self.config.profiles.push(ProfileConfig::default());
        }

        ui.add_space(8.0);
        ui.label("Default profile on startup:");
        let profile_names: Vec<String> = std::iter::once(String::from("(none)"))
            .chain(self.config.profiles.iter().map(|p| p.name.clone()))
            .collect();

        let current = if self.config.defaults.profile.is_empty() {
            "(none)".to_string()
        } else {
            self.config.defaults.profile.clone()
        };

        egui::ComboBox::from_label("Default profile")
            .selected_text(&current)
            .show_ui(ui, |ui| {
                for name in &profile_names {
                    let is_none = name == "(none)";
                    let value = if is_none { "" } else { name.as_str() };
                    ui.selectable_value(&mut self.config.defaults.profile, value.to_string(), name);
                }
            });
    }
}
