use anyhow::{bail, Result};
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use winreg::enums::*;
use winreg::RegKey;

const TASK_NAME: &str = "HideDesktopApps";
const APP_NAME: &str = "HideDesktopApps";
// Suppress the console window that PowerShell/schtasks would otherwise flash.
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Run a hidden PowerShell command and return an error if it fails.
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

/// Path of the old VBS launcher — cleaned up on migration.
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

/// Remove any leftover startup entries from older installs.
fn cleanup_legacy() {
    // Old VBS launcher
    let vbs = legacy_vbs_path();
    if vbs.exists() {
        let _ = std::fs::remove_file(&vbs);
    }
    // Old registry Run entry
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    if let Ok(key) = hkcu.open_subkey_with_flags(
        r"Software\Microsoft\Windows\CurrentVersion\Run",
        KEY_SET_VALUE,
    ) {
        let _ = key.delete_value(APP_NAME);
    }
}

/// Register a scheduled task that runs the app at logon with an optional delay.
/// Uses the same PowerShell approach as the installer so both agree on the task name.
pub fn register(exe_path: &str, delay_s: u32) -> Result<()> {
    // ISO 8601 duration, e.g. PT30S for 30 seconds.
    let delay = format!("PT{}S", delay_s);
    // Escape single quotes inside the exe path for use in a PowerShell string.
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

/// Remove the scheduled task (and any legacy startup entries).
pub fn unregister() -> Result<()> {
    let script = format!(
        "Unregister-ScheduledTask -TaskName '{TASK_NAME}' -Confirm:$false -ErrorAction SilentlyContinue"
    );
    powershell(&script)?;
    cleanup_legacy();
    Ok(())
}

/// Returns true if the scheduled task exists.
pub fn is_registered() -> bool {
    std::process::Command::new("schtasks.exe")
        .args(["/query", "/tn", TASK_NAME])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Sync the startup task to match config — called on every app startup.
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
