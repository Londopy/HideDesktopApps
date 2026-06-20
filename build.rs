fn main() {
    // Ask PowerShell for today's date and bake it into the binary.
    // Falls back to "unknown" if the shell isn't available (CI without PowerShell).
    let date = std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", "Get-Date -Format 'yyyy-MM-dd'"])
        .output()
        .ok()
        .and_then(|o| {
            let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if s.is_empty() {
                None
            } else {
                Some(s)
            }
        })
        .unwrap_or_else(|| "unknown".to_string());

    println!("cargo:rustc-env=BUILD_DATE={date}");

    // Embed the app icon into the Windows exe so it isn't a blank icon in
    // Explorer / the taskbar. Non-fatal if the resource compiler is unavailable.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("hide_desktop.ico");
        if let Err(e) = res.compile() {
            println!("cargo:warning=Failed to embed exe icon: {e}");
        }
    }
}
