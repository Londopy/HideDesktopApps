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
    Ok(())
}

// get the path to the running exe
pub fn current_exe_path() -> String {
    let mut buf = vec![0u16; 260];
    unsafe { windows::Win32::System::LibraryLoader::GetModuleFileNameW(None, &mut buf) };
    let end = buf.iter().position(|&c| c == 0).unwrap_or(260);
    String::from_utf16_lossy(&buf[..end])
}
