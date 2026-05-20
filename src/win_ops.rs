use crate::state::HiddenWindow;
use anyhow::Result;
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

// SAFETY: We only use HWND values as isize for storage; actual window operations
// happen on the same thread that called EnumWindows.
unsafe impl Send for EnumData {}

/// Returns the class name of a window.
pub fn get_class_name(hwnd: HWND) -> String {
    let mut buf = [0u16; 256];
    // SAFETY: FFI call with a properly sized buffer
    unsafe { GetClassNameW(hwnd, &mut buf) };
    let end = buf.iter().position(|&c| c == 0).unwrap_or(256);
    String::from_utf16_lossy(&buf[..end])
}

/// Returns the process name (exe filename) for a window's owning process.
fn get_process_name(hwnd: HWND) -> String {
    // SAFETY: FFI call to get process ID for a window
    let mut pid: u32 = 0;
    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
    if pid == 0 {
        return String::new();
    }

    // SAFETY: Opening a process handle with limited rights to query its name
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

/// EnumWindows callback that collects visible, non-shell application windows.
unsafe extern "system" fn enum_windows_cb(hwnd: HWND, lparam: LPARAM) -> BOOL {
    // SAFETY: lparam is a valid pointer to EnumData that we passed in
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

    // Get the current show command so we can restore it correctly
    let mut placement = WINDOWPLACEMENT::default();
    placement.length = std::mem::size_of::<WINDOWPLACEMENT>() as u32;
    let _ = GetWindowPlacement(hwnd, &mut placement);
    let show_cmd = placement.showCmd;

    data.hwnds.push((hwnd, show_cmd));
    TRUE
}

/// Enumerate all application windows suitable for hiding.
pub fn enumerate_app_windows(excluded_processes: &[String]) -> Vec<(HWND, u32)> {
    // SAFETY: my_pid is obtained from the OS and is always valid
    let my_pid = unsafe { windows::Win32::System::Threading::GetCurrentProcessId() };

    let mut data = EnumData {
        hwnds: Vec::new(),
        my_pid,
        excluded_processes: excluded_processes.to_vec(),
    };

    // SAFETY: EnumWindows is called with a valid callback and data pointer;
    // callback only reads/writes the EnumData struct via the lparam pointer.
    unsafe {
        let _ = EnumWindows(
            Some(enum_windows_cb),
            LPARAM(&mut data as *mut EnumData as isize),
        );
    }

    data.hwnds
}

/// Hide a set of windows and return their HiddenWindow records.
pub fn hide_windows(excluded_processes: &[String]) -> Result<Vec<HiddenWindow>> {
    let windows = enumerate_app_windows(excluded_processes);
    let mut hidden = Vec::new();

    for (hwnd, show_cmd) in windows {
        // SAFETY: hwnd is a valid window handle obtained from EnumWindows
        unsafe { ShowWindow(hwnd, SW_HIDE) };
        hidden.push(HiddenWindow {
            hwnd: hwnd.0 as isize,
            show_cmd,
        });
    }

    Ok(hidden)
}

/// Restore previously hidden windows to their original show state.
pub fn restore_windows(hidden: &[HiddenWindow]) -> Result<()> {
    for hw in hidden {
        // SAFETY: We reconstruct HWND from isize; this is safe as long as we
        // call restore while the handles are still valid (which they should be
        // since we only hide, not destroy them).
        let hwnd = HWND(hw.hwnd as *mut core::ffi::c_void);

        let cmd = match hw.show_cmd {
            3 => SW_SHOWMAXIMIZED,
            2 => SW_SHOWMINIMIZED,
            _ => SW_SHOWNORMAL,
        };

        unsafe { ShowWindow(hwnd, cmd) };
    }
    Ok(())
}

/// Get the path to the current executable.
pub fn current_exe_path() -> String {
    let mut buf = vec![0u16; 260];
    // SAFETY: FFI call with a properly sized buffer; None means current module
    unsafe { windows::Win32::System::LibraryLoader::GetModuleFileNameW(None, &mut buf) };
    let end = buf.iter().position(|&c| c == 0).unwrap_or(260);
    String::from_utf16_lossy(&buf[..end])
}
