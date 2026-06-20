use crate::state::HiddenWindow;
use anyhow::Result;
use std::path::PathBuf;
use windows::Win32::Foundation::{BOOL, HWND, LPARAM, TRUE};
use windows::Win32::UI::WindowsAndMessaging::*;

/// Shell window class names that should never be hidden.
const SHELL_CLASSES: &[&str] = &[
    "Shell_TrayWnd",
    "Shell_SecondaryTrayWnd",
    "Progman",
    "WorkerW",
    "Button",
    "DV2ControlHost",
    "Windows.UI.Core.CoreWindow",
    "XamlExplorerHostIslandWindow",
];

struct EnumData {
    hwnds: Vec<(HWND, u32)>, // (hwnd, show_cmd)
    my_pid: u32,
    excluded_processes: Vec<String>,
}

// HWNDs are stored as isize because HWND isn't Send
unsafe impl Send for EnumData {}

// get the window class name
pub fn get_class_name(hwnd: HWND) -> String {
    let mut buf = [0u16; 256];
    unsafe { GetClassNameW(hwnd, &mut buf) };
    let end = buf.iter().position(|&c| c == 0).unwrap_or(256);
    String::from_utf16_lossy(&buf[..end])
}

// get the exe filename for the process that owns this window
fn get_process_name(hwnd: HWND) -> String {
    let mut pid: u32 = 0;
    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
    if pid == 0 {
        return String::new();
    }

    unsafe {
        let handle = windows::Win32::System::Threading::OpenProcess(
            windows::Win32::System::Threading::PROCESS_QUERY_LIMITED_INFORMATION,
            false,
            pid,
        );
        let handle = match handle {
            Ok(h) => h,
            Err(_) => return String::new(),
        };

        let mut buf = vec![0u16; 260];
        let mut size = buf.len() as u32;
        let ok = windows::Win32::System::Threading::QueryFullProcessImageNameW(
            handle,
            windows::Win32::System::Threading::PROCESS_NAME_WIN32,
            windows::core::PWSTR(buf.as_mut_ptr()),
            &mut size,
        );

        let _ = windows::Win32::Foundation::CloseHandle(handle);

        if ok.is_ok() {
            let path = String::from_utf16_lossy(&buf[..size as usize]);
            std::path::Path::new(&path)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default()
        } else {
            String::new()
        }
    }
}

// EnumWindows callback — collects the windows we want to hide
unsafe extern "system" fn enum_windows_cb(hwnd: HWND, lparam: LPARAM) -> BOOL {
    // lparam is our EnumData struct
    let data = &mut *(lparam.0 as *mut EnumData);

    // Skip invisible windows
    if !IsWindowVisible(hwnd).as_bool() {
        return TRUE;
    }

    // Skip windows without a title
    let title_len = GetWindowTextLengthW(hwnd);
    if title_len == 0 {
        return TRUE;
    }

    // Skip shell windows
    let class = get_class_name(hwnd);
    if SHELL_CLASSES.iter().any(|&s| s == class) {
        return TRUE;
    }

    // Skip tool windows (floating toolbars, etc.)
    let ex_style = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
    if ex_style & WS_EX_TOOLWINDOW.0 != 0 {
        return TRUE;
    }

    // Skip windows belonging to our own process
    let mut pid: u32 = 0;
    GetWindowThreadProcessId(hwnd, Some(&mut pid));
    if pid == data.my_pid {
        return TRUE;
    }

    // Skip excluded processes
    let proc_name = get_process_name(hwnd);
    for excluded in &data.excluded_processes {
        if proc_name.eq_ignore_ascii_case(excluded) {
            return TRUE;
        }
    }

    // Only include top-level windows (no owner)
    let owner = GetWindow(hwnd, GW_OWNER);
    if let Ok(owner_hwnd) = owner {
        if !owner_hwnd.0.is_null() {
            return TRUE;
        }
    }

    // save the window state so we can restore it properly (normal/min/max)
    let mut placement = WINDOWPLACEMENT {
        length: std::mem::size_of::<WINDOWPLACEMENT>() as u32,
        ..Default::default()
    };
    let _ = GetWindowPlacement(hwnd, &mut placement);
    let show_cmd = placement.showCmd;

    data.hwnds.push((hwnd, show_cmd));
    TRUE
}

// get all windows that are candidates for hiding
pub fn enumerate_app_windows(excluded_processes: &[String]) -> Vec<(HWND, u32)> {
    let my_pid = unsafe { windows::Win32::System::Threading::GetCurrentProcessId() };

    let mut data = EnumData {
        hwnds: Vec::new(),
        my_pid,
        excluded_processes: excluded_processes.to_vec(),
    };

    unsafe {
        let _ = EnumWindows(
            Some(enum_windows_cb),
            LPARAM(&mut data as *mut EnumData as isize),
        );
    }

    data.hwnds
}

// hide all app windows and return a list so we can restore them later
pub fn hide_windows(excluded_processes: &[String]) -> Result<Vec<HiddenWindow>> {
    let windows = enumerate_app_windows(excluded_processes);
    let mut hidden = Vec::new();

    for (hwnd, show_cmd) in windows {
        unsafe {
            let _ = ShowWindow(hwnd, SW_HIDE);
        };
        hidden.push(HiddenWindow {
            hwnd: hwnd.0 as isize,
            show_cmd,
        });
    }

    // Persist the list so a crash can't strand hidden windows (see recovery fns).
    save_window_recovery(&hidden);
    Ok(hidden)
}

// restore all the windows we hid back to their original state
pub fn restore_windows(hidden: &[HiddenWindow]) -> Result<()> {
    for hw in hidden {
        // reconstruct the hwnd from the saved isize
        let hwnd = HWND(hw.hwnd as *mut core::ffi::c_void);

        let cmd = match hw.show_cmd {
            3 => SW_SHOWMAXIMIZED,
            2 => SW_SHOWMINIMIZED,
            _ => SW_SHOWNORMAL,
        };

        unsafe {
            let _ = ShowWindow(hwnd, cmd);
        };
    }
    clear_window_recovery();
    Ok(())
}

// get the path to the running exe
pub fn current_exe_path() -> String {
    let mut buf = vec![0u16; 260];
    unsafe { windows::Win32::System::LibraryLoader::GetModuleFileNameW(None, &mut buf) };
    let end = buf.iter().position(|&c| c == 0).unwrap_or(260);
    String::from_utf16_lossy(&buf[..end])
}

// ── Crash recovery for hidden app windows ────────────────────────────────────
// When windows are hidden we record their handles to a small file. A clean
// restore deletes it; a crash leaves it, so the next launch (and the panic
// hook) can re-show anything that was stranded.

fn recovery_path() -> Option<PathBuf> {
    crate::config::config_dir()
        .ok()
        .map(|d| d.join("hidden_windows.json"))
}

// Save (or, when empty, delete) the recovery record of hidden windows.
pub fn save_window_recovery(hidden: &[HiddenWindow]) {
    let Some(path) = recovery_path() else {
        return;
    };
    if hidden.is_empty() {
        let _ = std::fs::remove_file(&path);
        return;
    }
    if let Ok(json) = serde_json::to_string(hidden) {
        let _ = std::fs::write(&path, json);
    }
}

// Delete the recovery record once windows are safely restored.
pub fn clear_window_recovery() {
    if let Some(path) = recovery_path() {
        let _ = std::fs::remove_file(path);
    }
}

// Re-show any windows left hidden by a previous crashed session, returning how
// many were recovered. Safe to call when there is nothing to do.
pub fn recover_hidden_windows() -> usize {
    let Some(path) = recovery_path() else {
        return 0;
    };
    let Ok(data) = std::fs::read_to_string(&path) else {
        return 0;
    };
    let count = match serde_json::from_str::<Vec<HiddenWindow>>(&data) {
        Ok(hidden) if !hidden.is_empty() => {
            let _ = restore_windows(&hidden);
            hidden.len()
        }
        _ => 0,
    };
    let _ = std::fs::remove_file(&path);
    count
}

// ── Single-instance guard ────────────────────────────────────────────────────
// A named mutex shared across processes. The first instance owns it; later
// launches see it already exists and bail, so we never run two trays at once.

pub struct InstanceGuard(windows::Win32::Foundation::HANDLE);

impl Drop for InstanceGuard {
    fn drop(&mut self) {
        unsafe {
            let _ = windows::Win32::Foundation::CloseHandle(self.0);
        }
    }
}

// Returns Some(guard) if this is the only instance, or None if another already
// holds the mutex. When `wait_for_release` is true (a restart relaunch), retry
// briefly so the old process has time to exit and release it.
pub fn acquire_single_instance(wait_for_release: bool) -> Option<InstanceGuard> {
    use windows::core::w;
    use windows::Win32::Foundation::{CloseHandle, GetLastError, ERROR_ALREADY_EXISTS};
    use windows::Win32::System::Threading::CreateMutexW;

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    loop {
        unsafe {
            match CreateMutexW(None, true, w!("HideDesktopApps_SingleInstance_Mutex")) {
                Ok(handle) => {
                    if GetLastError() != ERROR_ALREADY_EXISTS {
                        return Some(InstanceGuard(handle));
                    }
                    // Another instance owns it; release our handle to the existing one.
                    let _ = CloseHandle(handle);
                }
                Err(_) => return None,
            }
        }
        if !wait_for_release || std::time::Instant::now() >= deadline {
            return None;
        }
        std::thread::sleep(std::time::Duration::from_millis(150));
    }
}

// True if the foreground window covers its whole monitor (a fullscreen app).
pub fn foreground_is_fullscreen() -> bool {
    use windows::Win32::Foundation::RECT;
    use windows::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST,
    };
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.0.is_null() {
            return false;
        }
        // ignore the desktop and shell windows
        let class = get_class_name(hwnd);
        if SHELL_CLASSES.iter().any(|&s| s == class) {
            return false;
        }
        let mut wr = RECT::default();
        if GetWindowRect(hwnd, &mut wr).is_err() {
            return false;
        }
        let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        let mut mi = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if !GetMonitorInfoW(monitor, &mut mi).as_bool() {
            return false;
        }
        let m = mi.rcMonitor;
        wr.left <= m.left && wr.top <= m.top && wr.right >= m.right && wr.bottom >= m.bottom
    }
}
