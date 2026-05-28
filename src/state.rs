// tracks what's currently hidden
#[derive(Debug, Default)]
pub struct AppState {
    pub icons_hidden: bool,
    pub taskbar_hidden: bool,
    pub windows_hidden: bool,
    pub active_profile: Option<String>,
    pub hidden_windows: Vec<HiddenWindow>,
}

// a window we hid, saved so we can restore it to the right state
// (e.g. maximized window should come back maximized)
#[derive(Debug, Clone)]
pub struct HiddenWindow {
    // hwnd as isize because HWND isn't Send
    pub hwnd: isize,
    // original window state: 1=normal, 2=minimized, 3=maximized
    pub show_cmd: u32,
}
