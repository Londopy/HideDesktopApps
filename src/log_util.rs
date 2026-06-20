use std::fs::OpenOptions;
use std::io::Write;

// writes a timestamped line to the debug log in appdata
// if the file can't be opened, just do nothing
pub fn write(msg: &str) {
    let log_path = match std::env::var("APPDATA") {
        Ok(appdata) => std::path::PathBuf::from(appdata)
            .join("HideDesktopApps")
            .join("debug.log"),
        Err(_) => return,
    };

    // make sure the folder exists first
    if let Some(parent) = log_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    // Rotate once the log grows past ~512 KB, keeping one previous file.
    const MAX_LOG_BYTES: u64 = 512 * 1024;
    if std::fs::metadata(&log_path).is_ok_and(|m| m.len() > MAX_LOG_BYTES) {
        let _ = std::fs::rename(&log_path, log_path.with_file_name("debug.log.1"));
    }

    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&log_path) {
        let now = chrono::Local::now();
        let _ = writeln!(file, "[{}] {}", now.format("%H:%M:%S%.3f"), msg);
    }
}

// macro so logging looks like eprintln
#[macro_export]
macro_rules! dlog {
    ($($arg:tt)*) => {
        $crate::log_util::write(&format!($($arg)*))
    };
}
