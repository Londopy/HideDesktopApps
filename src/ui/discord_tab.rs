use super::SettingsApp;
use egui::Ui;

impl SettingsApp {
    pub fn discord_tab(&mut self, ui: &mut Ui) {
        ui.heading("Discord Rich Presence");
        ui.add_space(8.0);

        if ui
            .checkbox(
                &mut self.config.discord.enabled,
                "Show what's hidden on my Discord profile",
            )
            .changed()
        {
            self.dirty = true;
        }

        ui.add_space(8.0);

        // live preview of what friends actually see
        let (details, state) = {
            let st = self.state_shared.lock().unwrap();
            let mut parts = Vec::new();
            if st.icons_hidden {
                parts.push("desktop icons");
            }
            if st.taskbar_hidden {
                parts.push("the taskbar");
            }
            if st.windows_hidden {
                parts.push("app windows");
            }
            let list = match parts.len() {
                0 => String::new(),
                1 => parts[0].to_string(),
                2 => format!("{} & {}", parts[0], parts[1]),
                _ => format!("{}, {} & {}", parts[0], parts[1], parts[2]),
            };
            let details = if list.is_empty() {
                "(nothing hidden — presence is cleared)".to_string()
            } else {
                format!("Hiding {list}")
            };
            let state = match &st.active_profile {
                Some(name) => format!("Profile: {name}"),
                None => "Custom setup".to_string(),
            };
            (details, state)
        };

        ui.group(|ui| {
            ui.label("Preview — this is what your Discord profile shows:");
            ui.add_space(4.0);
            ui.strong("HideDesktopApps");
            ui.label(details);
            ui.label(state);
            ui.weak("00:12 elapsed");
            ui.add_space(4.0);
            ui.label("[ Get HideDesktopApps ]");
        });

        ui.add_space(8.0);
        ui.weak(
            "The button links to the GitHub page, so anyone who asks \"what is that?\" \
             can find it without you explaining.",
        );
    }
}
