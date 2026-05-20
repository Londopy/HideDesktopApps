/// Stores the current visibility state of desktop elements and hidden windows.
#[derive(Debug, Default)]
pub struct AppState {
    pub icons_hidden: bool,
    pub taskbar_hidden: bool,
    pub windows_hidden: bool,
    pub active_profile: Option<String>,
    pub hidden_windows: Vec<HiddenWindow>,
}

/// A window that has been hidden by the app, with its original show command saved
/// so it can be restored correctly (e.g. maximized → restored maximized).
#[derive(Debug, Clone)]
pub struct HiddenWindow {
    /// Raw HWND value stored as isize because HWND is !Send.
    pub hwnd: isize,
    /// Original SW_SHOW* command: SW_SHOWNORMAL=1, SW_SHOWMINIMIZED=2, SW_SHOWMAXIMIZED=3.
    pub show_cmd: u32,
}
