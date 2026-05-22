use anyhow::Result;

/// Register a Task Scheduler ONLOGON task via PowerShell.
pub fn register(exe_path: &str, delay_s: u32) -> Result<()> {
    let delay_iso = format!("PT{}S", delay_s);
    let script = format!(
        r#"
$action = New-ScheduledTaskAction -Execute '{exe}'
$trigger = New-ScheduledTaskTrigger -AtLogOn
$trigger.Delay = '{delay}'
$settings = New-ScheduledTaskSettingsSet -StartWhenAvailable -ExecutionTimeLimit 0
$principal = New-ScheduledTaskPrincipal -UserId ([System.Security.Principal.WindowsIdentity]::GetCurrent().Name) -RunLevel Limited
Register-ScheduledTask -TaskName 'HideDesktopApps' -Action $action -Trigger $trigger -Settings $settings -Principal $principal -Force | Out-Null
"#,
        exe = exe_path,
        delay = delay_iso
    );

    let output = std::process::Command::new("powershell")
        .args([
            "-ExecutionPolicy",
            "Bypass",
            "-WindowStyle",
            "Hidden",
            "-NonInteractive",
            "-Command",
            &script,
        ])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        eprintln!("Startup register warning: {stderr}");
    }

    Ok(())
}

/// Unregister the Task Scheduler task.
pub fn unregister() -> Result<()> {
    let output = std::process::Command::new("powershell")
        .args([
            "-ExecutionPolicy",
            "Bypass",
            "-WindowStyle",
            "Hidden",
            "-NonInteractive",
            "-Command",
            "Unregister-ScheduledTask -TaskName 'HideDesktopApps' -Confirm:$false 2>$null",
        ])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        eprintln!("Startup unregister warning: {stderr}");
    }

    Ok(())
}

/// Check whether the task is currently registered.
pub fn is_registered() -> bool {
    let out = std::process::Command::new("powershell")
        .args([
            "-ExecutionPolicy",
            "Bypass",
            "-WindowStyle",
            "Hidden",
            "-NonInteractive",
            "-Command",
            "Get-ScheduledTask -TaskName 'HideDesktopApps' -ErrorAction SilentlyContinue",
        ])
        .output();

    out.map(|o| !o.stdout.is_empty()).unwrap_or(false)
}

/// Sync the startup task to match the config: register if enabled, unregister if disabled.
pub fn sync_startup(config: &crate::config::StartupConfig, exe_path: &str) {
    if config.enabled {
        if let Err(e) = register(exe_path, config.delay_s) {
            eprintln!("Failed to register startup task: {e}");
        }
    } else if is_registered() {
        if let Err(e) = unregister() {
            eprintln!("Failed to unregister startup task: {e}");
        }
    }
}
