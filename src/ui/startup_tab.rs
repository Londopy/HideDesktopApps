use super::SettingsApp;
use egui::Ui;

impl SettingsApp {
    pub fn startup_tab(&mut self, ui: &mut Ui) {
        ui.heading("Startup");
        ui.add_space(8.0);

        ui.checkbox(
            &mut self.config.startup.enabled,
            "Start HideDesktopApps at Windows logon",
        );
        ui.label("Adds the app to the Windows registry Run key (HKCU).");

        ui.add_space(8.0);
        ui.separator();

        let registered = self.startup_registered;
        ui.label(if registered {
            "Status: Registered in startup."
        } else {
            "Status: Not registered in startup."
        });

        ui.add_space(4.0);
        ui.horizontal(|ui| {
            if ui.button("Register Now").clicked() {
                let exe = crate::win_ops::current_exe_path();
                if let Err(e) = crate::startup::register(&exe, 0) {
                    eprintln!("Startup register error: {e}");
                } else {
                    self.startup_registered = true;
                }
            }
            if ui.button("Unregister").clicked() {
                if let Err(e) = crate::startup::unregister() {
                    eprintln!("Startup unregister error: {e}");
                } else {
                    self.startup_registered = false;
                }
            }
        });
    }
}
