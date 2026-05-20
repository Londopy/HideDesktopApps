use anyhow::Result;
use windows::core::w;
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{FindWindowW, SendMessageTimeoutW, SMTO_ABORTIFHUNG};

const WM_COMMAND: u32 = 0x0111;
// Shell command to toggle desktop icon visibility
const SHCMD_TOGGLE_DESKTOP_ICONS: u32 = 0x7402;

fn get_progman() -> HWND {
    // SAFETY: FFI call to find the Progman window which hosts desktop icons
    unsafe {
        FindWindowW(w!("Progman"), windows::core::PCWSTR::null())
            .unwrap_or(HWND(std::ptr::null_mut()))
    }
}

fn get_shell_defview() -> Option<HWND> {
    // SAFETY: FFI calls to traverse the window hierarchy to find ShellDefView
    unsafe {
        let progman = get_progman();
        if progman.0.is_null() {
            return None;
        }

        let defview = windows::Win32::UI::WindowsAndMessaging::FindWindowExW(
            progman,
            HWND(std::ptr::null_mut()),
            w!("SHELLDLL_DefView"),
            windows::core::PCWSTR::null(),
        )
        .unwrap_or(HWND(std::ptr::null_mut()));

        if !defview.0.is_null() {
            return Some(defview);
        }

        // Desktop icons may be hosted inside a WorkerW instead of Progman
        let mut found = HWND(std::ptr::null_mut());
        unsafe extern "system" fn find_defview_cb(
            hwnd: HWND,
            lparam: windows::Win32::Foundation::LPARAM,
        ) -> windows::Win32::Foundation::BOOL {
            // SAFETY: lparam is a valid pointer to HWND that we passed in
            let target = &mut *(lparam.0 as *mut HWND);
            let dv = windows::Win32::UI::WindowsAndMessaging::FindWindowExW(
                hwnd,
                HWND(std::ptr::null_mut()),
                w!("SHELLDLL_DefView"),
                windows::core::PCWSTR::null(),
            )
            .unwrap_or(HWND(std::ptr::null_mut()));
            if !dv.0.is_null() {
                *target = dv;
                return windows::Win32::Foundation::FALSE;
            }
            windows::Win32::Foundation::TRUE
        }

        let _ = windows::Win32::UI::WindowsAndMessaging::EnumWindows(
            Some(find_defview_cb),
            windows::Win32::Foundation::LPARAM(&mut found as *mut HWND as isize),
        );

        if !found.0.is_null() {
            Some(found)
        } else {
            None
        }
    }
}

/// Returns true if desktop icons are currently visible.
pub fn are_icons_visible() -> bool {
    // SAFETY: FFI call to find the SysListView32 which is the icon container
    unsafe {
        let defview = match get_shell_defview() {
            Some(dv) => dv,
            None => return true,
        };

        let listview = windows::Win32::UI::WindowsAndMessaging::FindWindowExW(
            defview,
            HWND(std::ptr::null_mut()),
            w!("SysListView32"),
            windows::core::PCWSTR::null(),
        )
        .unwrap_or(HWND(std::ptr::null_mut()));

        if listview.0.is_null() {
            return true;
        }

        windows::Win32::UI::WindowsAndMessaging::IsWindowVisible(listview).as_bool()
    }
}

/// Toggle desktop icon visibility by sending a WM_COMMAND to the ShellDefView.
pub fn toggle_icons() -> Result<()> {
    // SAFETY: FFI calls to send a shell command that toggles icon visibility
    unsafe {
        let progman = get_progman();
        anyhow::ensure!(!progman.0.is_null(), "Could not find Progman window");

        let mut result = 0usize;
        SendMessageTimeoutW(
            progman,
            WM_COMMAND,
            windows::Win32::Foundation::WPARAM(SHCMD_TOGGLE_DESKTOP_ICONS as usize),
            windows::Win32::Foundation::LPARAM(0),
            SMTO_ABORTIFHUNG,
            1000,
            Some(&mut result),
        );
    }
    Ok(())
}

/// Hide desktop icons. If already hidden, does nothing.
pub fn hide_icons() -> Result<()> {
    if are_icons_visible() {
        toggle_icons()?;
    }
    Ok(())
}

/// Show desktop icons. If already visible, does nothing.
pub fn show_icons() -> Result<()> {
    if !are_icons_visible() {
        toggle_icons()?;
    }
    Ok(())
}
