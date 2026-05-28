use anyhow::Result;
use windows::{
    core::HSTRING,
    Data::Xml::Dom::XmlDocument,
    UI::Notifications::{ToastNotification, ToastNotificationManager},
};

const APP_ID: &str = "HideDesktopApps";

// show a windows toast notification
pub fn show_toast(title: &str, body: &str) -> Result<()> {
    let xml_str = format!(
        r#"<toast><visual><binding template="ToastGeneric"><text>{}</text><text>{}</text></binding></visual></toast>"#,
        escape_xml(title),
        escape_xml(body)
    );

    let xml = XmlDocument::new()?;
    xml.LoadXml(&HSTRING::from(xml_str))?;
    let toast = ToastNotification::CreateToastNotification(&xml)?;
    let notifier = ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(APP_ID))?;
    notifier.Show(&toast)?;
    Ok(())
}

// escape special chars so the toast xml doesn't break
fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

// toast when a new version is available
pub fn notify_update_available(version: &str, config: &crate::config::NotificationsConfig) {
    if !config.enabled || !config.on_update {
        return;
    }
    if let Err(e) = show_toast(
        "HideDesktopApps Update",
        &format!("Version {} is available. Open Settings to update.", version),
    ) {
        eprintln!("Toast notification failed: {e}");
    }
}

// toast when the user checked for updates and is already up to date
pub fn notify_up_to_date(config: &crate::config::NotificationsConfig) {
    if !config.enabled || !config.on_update {
        return;
    }
    let version = env!("CARGO_PKG_VERSION");
    if let Err(e) = show_toast(
        "HideDesktopApps",
        &format!("You are up to date. (v{})", version),
    ) {
        eprintln!("Toast notification failed: {e}");
    }
}

// toast when a hotkey failed to register
pub fn notify_hotkey_failed(hotkey: &str, config: &crate::config::NotificationsConfig) {
    if !config.enabled || !config.on_hotkey_fail {
        return;
    }
    if let Err(e) = show_toast(
        "HideDesktopApps",
        &format!("Failed to register hotkey: {hotkey}"),
    ) {
        eprintln!("Toast notification failed: {e}");
    }
}

// toast when a profile is switched
pub fn notify_profile_switch(profile: &str, config: &crate::config::NotificationsConfig) {
    if !config.enabled || !config.on_profile_switch {
        return;
    }
    if let Err(e) = show_toast(
        "HideDesktopApps",
        &format!("Profile '{}' activated.", profile),
    ) {
        eprintln!("Toast notification failed: {e}");
    }
}
