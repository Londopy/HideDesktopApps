use anyhow::Result;
use windows::{
    core::HSTRING,
    Data::Xml::Dom::XmlDocument,
    UI::Notifications::{ToastNotification, ToastNotificationManager},
};

const APP_ID: &str = "HideDesktopApps";

/// Show a Windows toast notification.
pub fn show_toast(title: &str, body: &str) -> Result<()> {
    let xml_str = format!(
        r#"<toast><visual><binding template="ToastGeneric"><text>{}</text><text>{}</text></binding></visual></toast>"#,
        escape_xml(title),
        escape_xml(body)
    );

    // SAFETY: WinRT API calls that create COM objects on the current thread.
    // These are safe to call from any thread that has COM initialized.
    let xml = XmlDocument::new()?;
    xml.LoadXml(&HSTRING::from(xml_str))?;
    let toast = ToastNotification::CreateToastNotification(&xml)?;
    let notifier = ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(APP_ID))?;
    notifier.Show(&toast)?;
    Ok(())
}

/// Escape characters that would break the toast XML.
fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// Notify about an available update if notifications are enabled.
pub fn notify_update_available(
    version: &str,
    config: &crate::config::NotificationsConfig,
) {
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

/// Notify about a hotkey registration failure.
pub fn notify_hotkey_failed(
    hotkey: &str,
    config: &crate::config::NotificationsConfig,
) {
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

/// Notify when a profile is activated.
pub fn notify_profile_switch(
    profile: &str,
    config: &crate::config::NotificationsConfig,
) {
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
