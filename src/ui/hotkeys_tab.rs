use super::SettingsApp;
use egui::Ui;

/// Parse a hotkey string into (modifiers, key) for editing.
/// Returns (ctrl, alt, shift, win, key_char).
fn parse_for_edit(s: &str) -> (bool, bool, bool, bool, String) {
    let parts: Vec<&str> = s.split('+').collect();
    let key = parts.last().cloned().unwrap_or("").trim().to_uppercase();
    let mods = &parts[..parts.len().saturating_sub(1)];

    let ctrl = mods.iter().any(|m| m.trim().eq_ignore_ascii_case("ctrl"));
    let alt = mods.iter().any(|m| m.trim().eq_ignore_ascii_case("alt"));
    let shift = mods.iter().any(|m| m.trim().eq_ignore_ascii_case("shift"));
    let win = mods
        .iter()
        .any(|m| m.trim().eq_ignore_ascii_case("win") || m.trim().eq_ignore_ascii_case("super"));

    (ctrl, alt, shift, win, key)
}

/// Rebuild a hotkey string from components.
fn build_hotkey(ctrl: bool, alt: bool, shift: bool, win: bool, key: &str) -> String {
    let mut parts = Vec::new();
    if ctrl {
        parts.push("ctrl");
    }
    if alt {
        parts.push("alt");
    }
    if shift {
        parts.push("shift");
    }
    if win {
        parts.push("win");
    }
    parts.push(key);
    parts.join("+")
}

/// Show one hotkey editor block. Returns the new hotkey string.
fn hotkey_editor(ui: &mut Ui, label: &str, current: &str) -> String {
    let (mut ctrl, mut alt, mut shift, mut win, mut key) = parse_for_edit(current);

    ui.group(|ui| {
        ui.label(label);
        ui.horizontal(|ui| {
            ui.checkbox(&mut ctrl, "Ctrl");
            ui.checkbox(&mut alt, "Alt");
            ui.checkbox(&mut shift, "Shift");
            ui.checkbox(&mut win, "Win");
        });
        ui.horizontal(|ui| {
            ui.label("Key:");
            // Only allow a single A-Z character
            let mut key_input = key.clone();
            let response = ui.add(
                egui::TextEdit::singleline(&mut key_input)
                    .desired_width(40.0)
                    .char_limit(1),
            );
            if response.changed() {
                let filtered: String = key_input
                    .chars()
                    .filter(|c| c.is_ascii_alphabetic())
                    .map(|c| c.to_ascii_uppercase())
                    .collect();
                key = filtered;
            }
            let preview = build_hotkey(ctrl, alt, shift, win, &key.to_lowercase());
            ui.label(format!("→ {}", preview));
        });
    });

    build_hotkey(ctrl, alt, shift, win, &key.to_lowercase())
}

impl SettingsApp {
    pub fn hotkeys_tab(&mut self, ui: &mut Ui) {
        ui.heading("Global Hotkeys");
        ui.add_space(8.0);

        let new_icons = hotkey_editor(
            ui,
            "Toggle Desktop Icons",
            &self.config.hotkeys.icons.clone(),
        );
        ui.add_space(4.0);
        let new_taskbar = hotkey_editor(ui, "Toggle Taskbar", &self.config.hotkeys.taskbar.clone());
        ui.add_space(4.0);
        let new_windows = hotkey_editor(
            ui,
            "Toggle App Windows",
            &self.config.hotkeys.windows.clone(),
        );

        self.config.hotkeys.icons = new_icons;
        self.config.hotkeys.taskbar = new_taskbar;
        self.config.hotkeys.windows = new_windows;

        // Inline duplicate warning
        let h = &self.config.hotkeys;
        let icons = h.icons.clone();
        let taskbar = h.taskbar.clone();
        let windows = h.windows.clone();
        if icons == taskbar || icons == windows || taskbar == windows {
            ui.colored_label(
                egui::Color32::YELLOW,
                "Warning: duplicate hotkeys detected. All three must be unique.",
            );
        }
    }
}
