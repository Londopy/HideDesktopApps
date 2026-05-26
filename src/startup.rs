use anyhow::Result;
use std::path::PathBuf;
use winreg::enums::*;
use winreg::RegKey;

const APP_NAME: &str = "HideDesktopApps";
const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";

/// Old VBS launcher path — used only to clean up on migration.
fn legacy_vbs_path() -> PathBuf {
    let appdata = std::env::var("APPDATA").unwrap_or_default();
    PathBuf::from(appdata)
        .join("Microsoft")
        .join("Windows")
        .join("Start Menu")
        .join("Programs")
        .join("Startup")
        .join("HideDesktopApps.vbs")
}

/// Remove the old VBS launcher if it exists (left over from a previous install).
fn remove_legacy_vbs() {
    let path = legacy_vbs_path();
    if path.exists() {
        let _ = std::fs::remove_file(&path);
    }
}

/// Write a registry Run entry so the app launches at login.
/// The exe path is quoted to handle spaces in the path.
pub fn register(exe_path: &str, _delay_s: u32) -> Result<()> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let key = hkcu.open_subkey_with_flags(RUN_KEY, KEY_SET_VALUE)?;
    // Quote the path so spaces are handled correctly.
    let value = format!("\"{}\"", exe_path);
    key.set_value(APP_NAME, &value)?;
    // Clean up the old VBS launcher if it's still around.
    remove_legacy_vbs();
    Ok(())
}

/// Remove the registry Run entry.
pub fn unregister() -> Result<()> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let key = hkcu.open_subkey_with_flags(RUN_KEY, KEY_SET_VALUE)?;
    // delete_value returns an error if the value doesn't exist; that's fine.
    let _ = key.delete_value(APP_NAME);
    remove_legacy_vbs();
    Ok(())
}

/// Returns true if the registry Run entry exists.
pub fn is_registered() -> bool {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    if let Ok(key) = hkcu.open_subkey(RUN_KEY) {
        let val: Result<String, _> = key.get_value(APP_NAME);
        val.is_ok()
    } else {
        false
    }
}

/// Sync the startup entry to match the config.
pub fn sync_startup(config: &crate::config::StartupConfig, exe_path: &str) {
    crate::dlog!("sync_startup: enabled={}, exe={}", config.enabled, exe_path);
    if config.enabled {
        match register(exe_path, config.delay_s) {
            Ok(()) => crate::dlog!("startup: registered OK"),
            Err(e) => {
                crate::dlog!("startup: register failed: {}", e);
                eprintln!("Failed to register startup: {e}");
            }
        }
    } else if is_registered() {
        match unregister() {
            Ok(()) => crate::dlog!("startup: unregistered OK"),
            Err(e) => {
                crate::dlog!("startup: unregister failed: {}", e);
                eprintln!("Failed to unregister startup: {e}");
            }
        }
    } else {
        crate::dlog!("startup: disabled and not registered, nothing to do");
    }
}

/// Register the AppUserModelId in the registry and set it for the current process.
/// Windows requires this for toast notifications to work reliably.
pub fn setup_aumid() {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    if let Ok((key, _)) = hkcu.create_subkey(r"SOFTWARE\Classes\AppUserModelId\HideDesktopApps") {
        let _ = key.set_value("DisplayName", &APP_NAME.to_string());
    }
    unsafe {
        use windows::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID;
        let mut id: Vec<u16> = APP_NAME.encode_utf16().collect();
        id.push(0);
        let _ = SetCurrentProcessExplicitAppUserModelID(windows::core::PCWSTR(id.as_ptr()));
    }
}
