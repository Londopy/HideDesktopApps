use anyhow::Result;
use winreg::enums::*;
use winreg::RegKey;

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const APP_NAME: &str = "HideDesktopApps";

/// Add the app to HKCU\...\Run so it starts at login.
pub fn register(exe_path: &str, _delay_s: u32) -> Result<()> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (run_key, _) = hkcu.create_subkey(RUN_KEY)?;
    run_key.set_value(APP_NAME, &exe_path.to_string())?;
    Ok(())
}

/// Remove the app from HKCU\...\Run.
pub fn unregister() -> Result<()> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    if let Ok(run_key) = hkcu.open_subkey_with_flags(RUN_KEY, KEY_WRITE) {
        run_key.delete_value(APP_NAME).ok();
    }
    Ok(())
}

/// Returns true if the app is registered in HKCU\...\Run.
pub fn is_registered() -> bool {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    hkcu.open_subkey(RUN_KEY)
        .and_then(|k| k.get_value::<String, _>(APP_NAME))
        .is_ok()
}

/// Sync the startup entry to match the config: register if enabled, remove if disabled.
pub fn sync_startup(config: &crate::config::StartupConfig, exe_path: &str) {
    crate::dlog!(
        "sync_startup: enabled={}, exe={}",
        config.enabled,
        exe_path
    );
    if config.enabled {
        match register(exe_path, config.delay_s) {
            Ok(()) => crate::dlog!("startup: registered OK"),
            Err(e) => {
                crate::dlog!("s