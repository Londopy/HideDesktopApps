use std::fs::OpenOptions;
use std::io::Write;

/// Write a line to %APPDATA%\HideDesktopApps\debug.log.
/// Does nothing if the path can't be opened — never panics.
pub fn write(msg: &str) {
    let log_path = match std::env::var("APPDATA") {
        Ok(appdata) => std::path::PathBuf::from(appdata)
            .join("HideDesktopApps")
            .join("debug.log"),
        Err(_) => return,
    };

    // Make sure the directory exists before trying to open the file.
    if let Some(parent) = log_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&log_path) {
        let now = chrono::Local::now();
        let _ = writeln!(file, "[{}] {}", now.format("%H:%M:%S%.3f"), msg);
    }
}

/// Convenience macro so call sites look like eprintln!.
#[macro_export]
macro_rules! dlog {
    ($($arg:tt)*) => {
        $crate::log_util::write(&format!($($arg)*))
    };
}
