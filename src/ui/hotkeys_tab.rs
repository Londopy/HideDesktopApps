use super::{HotkeyField, SettingsApp};
use egui::Ui;

// split a hotkey string so we can show it in the editor
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

// put the hotkey string back together from parts
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

// map an egui key to the token our parser understands (a-z, 0-9, f1-f12)
fn egui_key_token(key: egui::Key) -> Option<&'static str> {
    use egui::Key::*;
    Some(match key {
        A => "a",
        B => "b",
        C => "c",
        D => "d",
        E => "e",
        F => "f",
        G => "g",
        H => "h",
        I => "i",
        J => "j",
        K => "k",
        L => "l",
        M => "m",
        N => "n",
        O => "o",
        P => "p",
        Q => "q",
        R => "r",
        S => "s",
        T => "t",
        U => "u",
        V => "v",
        W => "w",
        X => "x",
        Y => "y",
        Z => "z",
        Num0 => "0",
        Num1 => "1",
        Num2 => "2",
        Num3 => "3",
        Num4 => "4",
        Num5 => "5",
        Num6 => "6",
        Num7 => "7",
        Num8 => "8",
        Num9 => "9",
        F1 => "f1",
        F2 => "f2",
        F3 => "f3",
        F4 => "f4",
        F5 => "f5",
        F6 => "f6",
        F7 => "f7",
        F8 => "f8",
        F9 => "f9",
        F10 => "f10",
        F11 => "f11",
        F12 => "f12",
        _ => return None,
    })
}

// look for a "modifier(s)+key" press in this frame's events
fn capture_combo(input: &egui::InputState) -> Option<String> {
    for ev in &input.events {
        if let egui::Event::Key {
            key,
            pressed: true,
            modifiers,
            ..
        } = ev
        {
            if let Some(tok) = egui_key_token(*key) {
                let mut parts: Vec<&str> = Vec::new();
                if modifiers.ctrl {
                    parts.push("ctrl");
                }
                if modifiers.alt {
                    parts.push("alt");
                }
                if modifiers.shift {
                    parts.push("shift");
                }
                // require at least one modifier (a bare key is not a valid hotkey)
                if !parts.is_empty() {
                    parts.push(tok);
                    return Some(parts.join("+"));
                }
            }
        }
    }
    None
}

// Reusable hotkey editor: checkboxes + key, or a Record button that captures the
// real keypress. `recording` tracks which field is mid-capture (shared state so
// only one records at a time). Returns the (possibly unchanged) hotkey string.
pub(crate) fn hotkey_editor_widget(
    ui: &mut Ui,
    recording: &mut Option<HotkeyField>,
    field: HotkeyField,
    label: &str,
    current: &str,
) -> String {
    let (mut ctrl, mut alt, mut shift, mut win, mut key) = parse_for_edit(current);
    let is_recording = *recording == Some(field);

    let mut captured: Option<String> = None;
    if is_recording {
        ui.ctx().request_repaint();
        if let Some(combo) = ui.input(capture_combo) {
            captured = Some(combo);
            *recording = None;
        }
    }

    let mut toggle_record = false;
    ui.group(|ui| {
        ui.horizontal(|ui| {
            ui.label(label);
            let btn = if is_recording {
                "Press a combo\u{2026} (Esc to cancel)"
            } else {
                "\u{2328} Record"
            };
            if ui.button(btn).clicked() {
                toggle_record = true;
            }
        });
        ui.horizontal(|ui| {
            ui.checkbox(&mut ctrl, "Ctrl");
            ui.checkbox(&mut alt, "Alt");
            ui.checkbox(&mut shift, "Shift");
            ui.checkbox(&mut win, "Win");
        });
        ui.horizontal(|ui| {
            ui.label("Key:");
            let mut key_input = key.clone();
            let response = ui.add(
                egui::TextEdit::singleline(&mut key_input)
                    .desired_width(40.0)
                    .char_limit(1),
            );
            if response.changed() {
                key = key_input
                    .chars()
                    .filter(|c| c.is_ascii_alphabetic())
                    .map(|c| c.to_ascii_uppercase())
                    .collect();
            }
            let preview = build_hotkey(ctrl, alt, shift, win, &key.to_lowercase());
            ui.label(format!("\u{2192} {}", preview));
        });
    });

    if toggle_record {
        *recording = if is_recording { None } else { Some(field) };
    }

    if let Some(combo) = captured {
        return combo;
    }
    build_hotkey(ctrl, alt, shift, win, &key.to_lowercase())
}

impl SettingsApp {
    pub fn hotkeys_tab(&mut self, ui: &mut Ui) {
        ui.heading("Global Hotkeys");
        ui.label("Click Record and press your combo, or use the checkboxes + key.");
        ui.weak("Defaults \u{2014} Icons: Ctrl+Alt+H, Taskbar: Ctrl+Alt+T, Windows: Ctrl+Alt+W");
        ui.add_space(8.0);

        if self.recording_hotkey.is_some() && ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.recording_hotkey = None;
        }

        let icons = self.config.hotkeys.icons.clone();
        let new_icons = hotkey_editor_widget(
            ui,
            &mut self.recording_hotkey,
            HotkeyField::Icons,
            "Toggle Desktop Icons",
            &icons,
        );
        ui.add_space(4.0);
        let taskbar = self.config.hotkeys.taskbar.clone();
        let new_taskbar = hotkey_editor_widget(
            ui,
            &mut self.recording_hotkey,
            HotkeyField::Taskbar,
            "Toggle Taskbar",
            &taskbar,
        );
        ui.add_space(4.0);
        let windows = self.config.hotkeys.windows.clone();
        let new_windows = hotkey_editor_widget(
            ui,
            &mut self.recording_hotkey,
            HotkeyField::Windows,
            "Toggle App Windows",
            &windows,
        );

        if new_icons != self.config.hotkeys.icons
            || new_taskbar != self.config.hotkeys.taskbar
            || new_windows != self.config.hotkeys.windows
        {
            self.config.hotkeys.icons = new_icons;
            self.config.hotkeys.taskbar = new_taskbar;
            self.config.hotkeys.windows = new_windows;
            self.dirty = true;
        }

        let h = &self.config.hotkeys;
        if h.icons == h.taskbar || h.icons == h.windows || h.taskbar == h.windows {
            ui.colored_label(
                egui::Color32::YELLOW,
                "Warning: duplicate hotkeys \u{2014} all three must be unique.",
            );
        }
    }
}
