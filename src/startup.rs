use anyhow::Result;
use std::path::PathBuf;
use winreg::enums::*;
use winreg::RegKey;

const APP_NAME: &str = "HideDesktopApps";

fn vbs_path() -> PathBuf {
    let appdata = std::env::var("APPDATA").unwrap_or_default();
    PathBuf::from(appdata)
        .join("Microsoft")
        .join("Windows")
        .join("Start Menu")
        .join("Programs")
        .join("Startup")
        .join("HideDesktopApps.vbs")
}

/// Write a VBS launcher to the Startup folder so the app runs silently at login.
pub fn register(exe_path: &str, _delay_s: u32) -> Result<()> {
    let path = vbs_path();
    // Double-quote the exe path inside the VBS string literal
    let escaped = exe_path.replace('"', "\"\"");
    let content = format!(
        "CreateObject(\"WScript.Shell\").Run \"\"\"{}\"\"\", 0, False\n",
        escaped
    );
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, content)?;
    Ok(())
}

/// Remove the VBS launcher from the Startup folder.
pub fn unregister() -> Result<()> {
    let path = vbs_path();
    if path.exists() {
        std::fs::remove_file(&path)?;
    }
    Ok(())
}

/// Returns true if the VBS launcher exists in the Startup folder.
pub fn is_registered() -> bool {
    vbs_path().exists()
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
        // encode_utf16 gives code units; append null terminator
        let mut id: Vec<u16> = APP_NAME.encode_utf16().collect();
        id.push(0);
        let _ = SetCurrentProcessExplicitAppUserModelID(windows::core::PCWSTR(id.as_ptr()));
    }
}
