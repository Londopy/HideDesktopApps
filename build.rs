fn main() {
    // Bake the build date into the binary so the About tab can show it.
    // Uses the SOURCE_DATE_EPOCH env var if set (reproducible builds),
    // otherwise falls back to the current date.
    let date = std::env::var("SOURCE_DATE_EPOCH")
        .ok()
        .and_then(|epoch| {
            epoch.parse::<i64>().ok().map(|secs| {
                // Simple YYYY-MM-DD from unix timestamp
                let days = secs / 86400;
                let y400 = days / 146097;
                let rem = days % 146097;
                let y100 = (rem / 36524).min(3);
                let rem = rem - y100 * 36524;
                let y4 = rem / 1461;
                let rem = rem % 1461;
                let y1 = (rem / 365).min(3);
                let year = y400 * 400 + y100 * 100 + y4 * 4 + y1 + 1970;
                let yday = rem - y1 * 365;
                let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
                let days_in_month: [i64; 12] = [31, if leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
                let mut month = 0usize;
                let mut d = yday;
                for (i, &dim) in days_in_month.iter().enumerate() {
                    if d < dim { month = i + 1; break; }
                    d -= dim;
                }
                format!("{year}-{month:02}-{:02}", d + 1)
            })
        })
        .unwrap_or_else(|| {
            // No SOURCE_DATE_EPOCH — use the system date via a PowerShell call.
            // Falls back to "unknown" if the shell isn't available.
            std::process::Command::new("powershell")
                .args(["-NoProfile", "-Command", "Get-Date -Format 'yyyy-MM-dd'"])
                .output()
                .ok()
                .and_then(|o| {
                    let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
                    if s.is_empty() { None } else { Some(s) }
                })
                .unwrap_or_else(|| "unknown".to_string())
        });

    println!("cargo:rustc-env=BUILD_DATE={date}");
}
