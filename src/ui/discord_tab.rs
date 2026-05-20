use super::SettingsApp;
use egui::Ui;

impl SettingsApp {
    pub fn discord_tab(&mut self, ui: &mut Ui) {
        ui.heading("Discord Rich Presence");
        ui.add_space(8.0);

        ui.checkbox(
            &mut self.config.discord.enabled,
            "Show current hide state in Discord Rich Presence",
        );

        ui.add_space(8.0);
        ui.group(|ui| {
            ui.label("When enabled, Discord will show:");
            ui.label("  State: HideDesktopApps");
            ui.label("  Details: e.g. \"Icons hidden · Taskbar hidden\"");
        });

        ui.add_space(8.0);
        ui.label("Requires a Discord application to be registered at:");
        ui.hyperlink("https://discord.com/developers/applications");
        ui.label("Replace the placeholder App ID in src/discord.rs with your App ID.");

        ui.add_space(8.0);
        ui.label("Discord must be running for Rich Presence to work.");
    }
}
