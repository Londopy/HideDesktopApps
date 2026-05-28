use super::SettingsApp;
use egui::Ui;

impl SettingsApp {
    pub fn startup_tab(&mut self, ui: &mut Ui) {
        ui.heading("Startup");
        ui.add_space(8.0);

        if ui
            .checkbox(
                &mut self.config.startup.enabled,
                "Start HideDesktopApps at Windows logon",
            )
            .changed()
        {
            self.dirty = true;
        }

        // Big clear status banner so the user always knows the actual state.
        let (banner_text, banner_color) = if self.config.startup.enabled {
            ("Startup: ENABLED", egui::Color32::from_rgb(80, 200, 100))
        } else {
            ("Startup: DISABLED", egui::Color32::from_rgb(160, 160, 160))
        };
        ui.colored_label(banner_color, banner_text);

        ui.add_space(8.0);
        ui.separator();

        // Query the task scheduler directly so the status is always accurate.
        let registered = crate::startup::is_registered();
        self.startup_registered = registered;

        let (task_text, task_color) = if registered {
            (
                "Task registered: Yes",
                egui::Color32::from_rgb(80, 200, 100),
            )
        } else {
            ("Task registered: No", egui::Color32::from_rgb(220, 100, 80))
        };
        ui.colored_label(task_color, task_text);

        ui.add_space(4.0);
        ui.horizontal(|ui| {
            if ui.button("Register Now").clicked() {
                self.startup_error = None;
                let exe = crate::win_ops::current_exe_path();
                match crate::startup::register(&exe, 0) {
                    Ok(()) => self.startup_registered = true,
                    Err(e) => self.startup_error = Some(format!("Register failed: {e}")),
                }
            }
            if ui.button("Unregister").clicked() {
                self.startup_error = None;
                match crate::startup::unregister() {
                    Ok(()) => self.startup_registered = false,
                    Err(e) => self.startup_error = Some(format!("Unregister failed: {e}")),
                }
            }
        });

        if let Some(ref err) = self.startup_error {
            ui.add_space(4.0);
            ui.colored_label(egui::Color32::RED, err);
        }
    }
}
