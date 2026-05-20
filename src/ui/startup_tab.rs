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
        ui.label("Uses Windows Task Scheduler (runs without UAC prompt).");

        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.label("Startup delay:");
            ui.add(
                egui::Slider::new(&mut self.config.startup.delay_s, 0..=60)
                    .suffix(" seconds"),
            );
        });

        ui.add_space(8.0);
        ui.separator();

        let registered = self.startup_registered;
        ui.label(if registered {
            "Status: Task Scheduler task is registered."
        } else {
            "Status: Not registered in Task Scheduler."
        });

        ui.add_space(4.0);
        ui.horizontal(|ui| {
            if ui.button("Register Now").clicked() {
                let exe = crate::win_ops::current_exe_path();
                if let Err(e) = crate::startup::register(&exe, self.config.startup.delay_s) {
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
