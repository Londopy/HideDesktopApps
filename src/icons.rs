use anyhow::Result;
use windows::core::w;
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{FindWindowW, SendMessageTimeoutW, SMTO_ABORTIFHUNG};

const WM_COMMAND: u32 = 0x0111;
const SHCMD_TOGGLE_DESKTOP_ICONS: u32 = 0x7402;

fn get_progman() -> HWND {
    unsafe {
        FindWindowW(w!("Progman"), windows::core::PCWSTR::null())
            .unwrap_or(HWND(std::ptr::null_mut()))
    }
}

fn get_shell_defview() -> Option<HWND> {
    unsafe {
        let progman = get_progman();
        if !progman.0.is_null() {
            let dv = windows::Win32::UI::WindowsAndMessaging::FindWindowExW(
                progman,
                HWND(std::ptr::null_mut()),
                w!("SHELLDLL_DefView"),
                windows::core::PCWSTR::null(),
            )
            .unwrap_or(HWND(std::ptr::null_mut()));
            if !dv.0.is_null() {
                return Some(dv);
            }
        }

        // on win11 it might be inside a WorkerW instead of Progman
        let mut found = HWND(std::ptr::null_mut());

        unsafe extern "system" fn find_defview_cb(
            hwnd: HWND,
            lparam: windows::Win32::Foundation::LPARAM,
        ) -> windows::Win32::Foundation::BOOL {
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

// check if desktop icons are currently visible
pub fn are_icons_visible() -> bool {
    unsafe {
        let defview = match get_shell_defview() {
            Some(dv) => dv,
            None => {
                crate::dlog!("are_icons_visible: SHELLDLL_DefView not found, assuming visible");
                return true;
            }
        };

        let listview = windows::Win32::UI::WindowsAndMessaging::FindWindowExW(
            defview,
            HWND(std::ptr::null_mut()),
            w!("SysListView32"),
            windows::core::PCWSTR::null(),
        )
        .unwrap_or(HWND(std::ptr::null_mut()));

        if listview.0.is_null() {
            crate::dlog!("are_icons_visible: SysListView32 not found, assuming visible");
            return true;
        }

        let visible = windows::Win32::UI::WindowsAndMessaging::IsWindowVisible(listview).as_bool();
        crate::dlog!("are_icons_visible: SysListView32 visible = {visible}");
        visible
    }
}

// toggle icons by sending WM_COMMAND to the shell window
pub fn toggle_icons() -> Result<()> {
    unsafe {
        let target = match get_shell_defview() {
            Some(dv) => {
                crate::dlog!("toggle_icons: target = SHELLDLL_DefView");
                dv
            }
            None => {
                let progman = get_progman();
                anyhow::ensure!(!progman.0.is_null(), "Could not find Progman window");
                crate::dlog!("toggle_icons: target = Progman (fallback)");
                progman
            }
        };

        let mut result = 0usize;
        SendMessageTimeoutW(
            target,
            WM_COMMAND,
            windows::Win32::Foundation::WPARAM(SHCMD_TOGGLE_DESKTOP_ICONS as usize),
            windows::Win32::Foundation::LPARAM(0),
            SMTO_ABORTIFHUNG,
            1000,
            Some(&mut result),
        );
        crate::dlog!("toggle_icons: WM_COMMAND sent, result = {result}");
    }
    Ok(())
}

// hide icons, skip if already hidden
pub fn hide_icons() -> Result<()> {
    crate::dlog!("hide_icons called");
    if are_icons_visible() {
        toggle_icons()?;
    } else {
        crate::dlog!("hide_icons: already hidden, skipping");
    }
    Ok(())
}

// show icons, skip if already visible
pub fn show_icons() -> Result<()> {
    crate::dlog!("show_icons called");
    if !are_icons_visible() {
        toggle_icons()?;
    } else {
        crate::dlog!("show_icons: already visible, skipping");
    }
    Ok(())
}
