use anyhow::{bail, Result};
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use winreg::enums::*;
use winreg::RegKey;

const TASK_NAME: &str = "HideDesktopApps";
const APP_NAME: &str = "HideDesktopApps";
// hide the powershell window that would otherwise flash on screen
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

// runs a powershell command hidden
fn powershell(script: &str) -> Result<()> {
    let status = std::process::Command::new("powershell.exe")
        .args([
            "-WindowStyle",
            "Hidden",
            "-NonInteractive",
            "-Command",
            script,
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .status()?;
    if !status.success() {
        bail!("PowerShell exited with code {:?}", status.code());
    }
    Ok(())
}

// path of the old vbs launcher from the python version
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

// clean up old startup entries from older versions of the app
fn cleanup_legacy() {
    // old vbs file
    let vbs = legacy_vbs_path();
    if vbs.exists() {
        let _ = std::fs::remove_file(&vbs);
    }
    // old registry Run key
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    if let Ok(key) = hkcu.open_subkey_with_flags(
        r"Software\Microsoft\Windows\CurrentVersion\Run",
        KEY_SET_VALUE,
    ) {
        let _ = key.delete_value(APP_NAME);
    }
}

// register a scheduled task to run the app at login with an optional delay
pub fn register(exe_path: &str, delay_s: u32) -> Result<()> {
    // delay format is PT30S for 30 seconds, PT0S for no delay
    let delay = format!("PT{}S", delay_s);
    // escape quotes in the path for powershell
    let escaped = exe_path.replace('\'', "''");
    let script = format!(
        "$t = New-ScheduledTaskTrigger -AtLogOn; \
         $t.Delay = '{delay}'; \
         $a = New-ScheduledTaskAction -Execute '\"{escaped}\"'; \
         $s = New-ScheduledTaskSettingsSet -ExecutionTimeLimit 0 -AllowStartIfOnBatteries $true; \
         Register-ScheduledTask -TaskName '{TASK_NAME}' -Trigger $t -Action $a -Settings $s -Force | Out-Null"
    );
    powershell(&script)?;
    cleanup_legacy();
    Ok(())
}

// remove the startup task
pub fn unregister() -> Result<()> {
    let script = format!(
        "Unregister-ScheduledTask -TaskName '{TASK_NAME}' -Confirm:$false -ErrorAction SilentlyContinue"
    );
    powershell(&script)?;
    cleanup_legacy();
    Ok(())
}

// check if the startup task is registered
pub fn is_registered() -> bool {
    std::process::Command::new("schtasks.exe")
        .args(["/query", "/tn", TASK_NAME])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

// make sure the startup task matches what's in the config
pub fn sync_startup(config: &crate::config::StartupConfig, exe_path: &str) {
    crate::dlog!("sync_startup: enabled={}, exe={}", config.enabled, exe_path);
    if config.enabled {
        match register(exe_path, config.delay_s) {
            Ok(()) => crate::dlog!("startup: task registered OK"),
            Err(e) => {
                crate::dlog!("startup: register failed: {}", e);
                eprintln!("Failed to register startup task: {e}");
            }
        }
    } else if is_registered() {
        match unregister() {
            Ok(()) => crate::dlog!("startup: task unregistered OK"),
            Err(e) => {
                crate::dlog!("startup: unregister failed: {}", e);
                eprintln!("Failed to unregister startup task: {e}");
            }
        }
    } else {
        crate::dlog!("startup: disabled and task not present, nothing to do");
    }
}

// register the app id so windows toast notifications work
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
