use super::SettingsApp;
use egui::Ui;

impl SettingsApp {
    pub fn notifications_tab(&mut self, ui: &mut Ui) {
        ui.heading("Notifications");
        ui.add_space(8.0);

        ui.checkbox(
            &mut self.config.notifications.enabled,
            "Enable Windows toast notifications",
        );

        ui.add_space(8.0);
        ui.add_enabled_ui(self.config.notifications.enabled, |ui| {
            ui.group(|ui| {
                ui.label("Show notifications for:");
                ui.checkbox(
                    &mut self.config.notifications.on_update,
                    "Available updates",
                );
                ui.checkbox(
                    &mut self.config.notifications.on_hotkey_fail,
                    "Hotkey registration failures",
                );
                ui.checkbox(
                    &mut self.config.notifications.on_profile_switch,
                    "Profile switches",
                );
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
