use super::SettingsApp;
use egui::Ui;

// compile-time constants
const BUILD_TARGET: &str = if cfg!(target_arch = "x86_64") {
    "x64"
} else if cfg!(target_arch = "aarch64") {
    "ARM64"
} else if cfg!(target_arch = "x86") {
    "x86"
} else {
    "unknown"
};

const BUILD_DATE: &str = env!("BUILD_DATE");

impl SettingsApp {
    pub fn about_tab(&mut self, ui: &mut Ui) {
        ui.heading("HideDesktopApps");
        ui.add_space(8.0);

        ui.horizontal(|ui| {
            ui.label("Version:");
            ui.label(env!("CARGO_PKG_VERSION"));
        });

        ui.horizontal(|ui| {
            ui.label("Build target:");
            ui.label(BUILD_TARGET);
        });

        ui.horizontal(|ui| {
            ui.label("Build date:");
            ui.label(BUILD_DATE);
        });

        ui.add_space(8.0);
        ui.label(
            "System tray utility for hiding and showing desktop icons, \
             the taskbar, and open application windows via global hotkeys.",
        );

        ui.add_space(8.0);
        ui.hyperlink_to(
            "GitHub Repository",
            "https://github.com/Londopy/HideDesktopApps",
        );
        ui.hyperlink_to(
            "Report an Issue",
            "https://github.com/Londopy/HideDesktopApps/issues",
        );
        ui.hyperlink_to(
            "Latest Releases",
            "https://github.com/Londopy/HideDesktopApps/releases",
        );

        ui.add_space(8.0);
        ui.separator();

        ui.label("License: MIT (see LICENSE-COMMERCIAL for commercial use)");

        ui.add_space(8.0);
        ui.label("Built with:");
        ui.label("  • Rust");
        ui.label("  • egui / eframe (UI)");
        ui.label("  • tray-icon (system tray)");
        ui.label("  • global-hotkey (hotkeys)");
        ui.label("  • windows-rs (Win32 API)");

        ui.add_space(8.0);
        ui.separator();

        ui.horizontal(|ui| {
            if ui.button("Copy debug info").clicked() {
                let info = format!(
                    "HideDesktopApps v{} ({BUILD_TARGET}) built {BUILD_DATE}\nRepo: https://github.com/Londopy/HideDesktopApps",
                    env!("CARGO_PKG_VERSION"),
                );
                ui.output_mut(|o| o.copied_text = info);
            }

            if ui.button("Restart").clicked() {
                let _ = self.cmd_tx.send(crate::Cmd::Restart);
            }
            if ui.button("Exit").clicked() {
                let _ = self.cmd_tx.send(crate::Cmd::Exit);
            }
        });
    }
}
