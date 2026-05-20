use anyhow::Result;
use windows::core::w;
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{
    FindWindowW, IsWindowVisible, ShowWindow, SW_HIDE, SW_SHOW,
};

/// Get the main taskbar HWND.
fn get_taskbar() -> HWND {
    // SAFETY: FFI call to find the Shell_TrayWnd (the main taskbar)
    unsafe { FindWindowW(w!("Shell_TrayWnd"), windows::core::PCWSTR::null()) }
}

/// Get the secondary taskbar HWNDs (for multi-monitor setups).
fn get_secondary_taskbars() -> Vec<HWND> {
    let mut hwnds = Vec::new();

    // SAFETY: FFI EnumWindows call with a valid callback and data pointer
    unsafe {
        unsafe extern "system" fn cb(
            hwnd: HWND,
            lparam: windows::Win32::Foundation::LPARAM,
        ) -> windows::Win32::Foundation::BOOL {
            // SAFETY: lparam is a valid pointer to Vec<HWND> that we passed in
            let list = &mut *(lparam.0 as *mut Vec<HWND>);
            let mut class = [0u16; 256];
            windows::Win32::UI::WindowsAndMessaging::GetClassNameW(hwnd, &mut class);
            let name = String::from_utf16_lossy(
                &class[..class.iter().position(|&c| c == 0).unwrap_or(256)],
            );
            if name == "Shell_SecondaryTrayWnd" {
                list.push(hwnd);
            }
            windows::Win32::Foundation::TRUE
        }

        let _ = windows::Win32::UI::WindowsAndMessaging::EnumWindows(
            Some(cb),
            windows::Win32::Foundation::LPARAM(&mut hwnds as *mut Vec<HWND> as isize),
        );
    }

    hwnds
}

/// Returns true if the taskbar is currently visible.
pub fn is_taskbar_visible() -> bool {
    let hwnd = get_taskbar();
    if hwnd.0 == 0 {
        return true;
    }
    // SAFETY: FFI call to check window visibility
    unsafe { IsWindowVisible(hwnd).as_bool() }
}

/// Hide the taskbar (main + all secondary monitors).
pub fn hide_taskbar() -> Result<()> {
    let primary = get_taskbar();
    anyhow::ensure!(primary.0 != 0, "Could not find taskbar window");

    // SAFETY: FFI ShowWindow calls are safe when the HWND is valid
    unsafe {
        ShowWindow(primary, SW_HIDE);
        for secondary in get_secondary_taskbars() {
            ShowWindow(secondary, SW_HIDE);
        }
    }
    Ok(())
}

/// Show the taskbar (main + all secondary monitors).
pub fn show_taskbar() -> Result<()> {
    let primary = get_taskbar();
    anyhow::ensure!(primary.0 != 0, "Could not find taskbar window");

    // SAFETY: FFI ShowWindow calls are safe when the HWND is valid
    unsafe {
        ShowWindow(primary, SW_SHOW);
        for secondary in get_secondary_taskbars() {
            ShowWindow(secondary, SW_SHOW);
        }
    }
    Ok(())
}

/// Toggle taskbar visibility.
pub fn toggle_taskbar() -> Result<bool> {
    if is_taskbar_visible() {
        hide_taskbar()?;
        Ok(false)
    } else {
        show_taskbar()?;
        Ok(true)
    }
}
