use super::hotkeys_tab::hotkey_editor_widget;
use super::{HotkeyField, SettingsApp};
use crate::config::ProfileConfig;
use egui::Ui;

impl SettingsApp {
    pub fn profiles_tab(&mut self, ui: &mut Ui) {
        ui.heading("Profiles");
        ui.label("Define preset hide/show combinations activated by a hotkey.");
        ui.add_space(8.0);

        // Esc cancels an in-progress hotkey recording.
        if self.recording_hotkey.is_some() && ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.recording_hotkey = None;
        }

        let mut to_delete: Option<usize> = None;
        let count = self.config.profiles.len();
        for i in 0..count {
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.label("Name:");
                    let r = ui.text_edit_singleline(&mut self.config.profiles[i].name);
                    if r.changed() {
                        self.dirty = true;
                    }
                    let remove = ui.add(egui::Button::new("Remove").fill(egui::Color32::DARK_RED));
                    if remove.clicked() {
                        to_delete = Some(i);
                    }
                });

                // hotkey recorder (same widget as the Hotkeys tab)
                let current = self.config.profiles[i].hotkey.clone();
                let new_hk = hotkey_editor_widget(
                    ui,
                    &mut self.recording_hotkey,
                    HotkeyField::Profile(i),
                    "Hotkey",
                    &current,
                );
                if new_hk != self.config.profiles[i].hotkey {
                    self.config.profiles[i].hotkey = new_hk;
                    self.dirty = true;
                }
                if ui.button("Clear hotkey").clicked() {
                    self.config.profiles[i].hotkey.clear();
                    self.dirty = true;
                }

                ui.horizontal(|ui| {
                    let ric = ui.checkbox(&mut self.config.profiles[i].icons, "Hide Icons");
                    if ric.changed() {
                        self.dirty = true;
                    }
                    let rtb = ui.checkbox(&mut self.config.profiles[i].taskbar, "Hide Taskbar");
                    if rtb.changed() {
                        self.dirty = true;
                    }
                    let rwn = ui.checkbox(&mut self.config.profiles[i].windows, "Hide Windows");
                    if rwn.changed() {
                        self.dirty = true;
                    }
                });
            });
            ui.add_space(4.0);
        }

        if let Some(idx) = to_delete {
            self.config.profiles.remove(idx);
            self.dirty = true;
        }

        if ui.button("+ Add Profile").clicked() {
            self.config.profiles.push(ProfileConfig::default());
            self.dirty = true;
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

        let combo = egui::ComboBox::from_label("Default profile")
            .selected_text(&current)
            .show_ui(ui, |ui| {
                let mut changed = false;
                for name in &profile_names {
                    let is_none = name == "(none)";
                    let value = if is_none { "" } else { name.as_str() };
                    if ui
                        .selectable_value(
                            &mut self.config.defaults.profile,
                            value.to_string(),
                            name,
                        )
                        .changed()
                    {
                        changed = true;
                    }
                }
                changed
            });
        if combo.inner == Some(true) {
            self.dirty = true;
        }
    }
}
