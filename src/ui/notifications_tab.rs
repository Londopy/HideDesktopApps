use super::SettingsApp;
use egui::Ui;

impl SettingsApp {
    pub fn notifications_tab(&mut self, ui: &mut Ui) {
        ui.heading("Notifications");
        ui.add_space(8.0);

        if ui
            .checkbox(
                &mut self.config.notifications.enabled,
                "Enable Windows toast notifications",
            )
            .changed()
        {
            self.dirty = true;
        }

        ui.add_space(8.0);
        ui.add_enabled_ui(self.config.notifications.enabled, |ui| {
            ui.group(|ui| {
                ui.label("Show notifications for:");
                if ui
                    .checkbox(
                        &mut self.config.notifications.on_update,
                        "Available updates",
                    )
                    .changed()
                {
                    self.dirty = true;
                }
                if ui
                    .checkbox(
                        &mut self.config.notifications.on_hotkey_fail,
                        "Hotkey registration failures",
                    )
                    .changed()
                {
                    self.dirty = true;
                }
                if ui
                    .checkbox(
                        &mut self.config.notifications.on_profile_switch,
                        "Profile switches",
                    )
                    .changed()
                {
                    self.dirty = true;
                }
            });
        });

        ui.add_space(8.0);
        if ui.button("Send Test Notification").clicked() {
            let _ = crate::notifications::show_toast(
                "HideDesktopApps",
                "Test notification — it works!",
            );
        }
    }
}
