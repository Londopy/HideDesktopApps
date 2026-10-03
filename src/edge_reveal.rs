// Bring a hidden taskbar back while the mouse is pushed against the screen edge
// it was docked to, and hide it again once the mouse moves away, the way an
// auto-hiding taskbar behaves. The taskbar stays "hidden" as far as the rest of
// the app is concerned; this only peeks it in and out.
use std::time::{Duration, Instant};

use windows::Win32::Foundation::{HWND, POINT, RECT};
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST,
};
use windows::Win32::System::Threading::GetCurrentProcessId;
use windows::Win32::UI::WindowsAndMessaging::{
    GetAncestor, GetClassNameW, GetCursorPos, GetForegroundWindow, GetWindowRect,
    GetWindowThreadProcessId, WindowFromPoint, GA_ROOT,
};

// how long the mouse has to rest on the edge before the taskbar shows
const DWELL: Duration = Duration::from_millis(250);
// how long the taskbar stays after the mouse leaves it
const LINGER: Duration = Duration::from_millis(800);
// thickness of the strip along the screen edge that counts as "at the edge"
const EDGE_PX: i32 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl Rect {
    fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.left && x < self.right && y >= self.top && y < self.bottom
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Edge {
    Bottom,
    Top,
    Left,
    Right,
}

impl Edge {
    pub fn name(self) -> &'static str {
        match self {
            Edge::Bottom => "bottom",
            Edge::Top => "top",
            Edge::Left => "left",
            Edge::Right => "right",
        }
    }
}

// one taskbar: where it sits and which monitor edge it is docked to
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Zone {
    pub taskbar: Rect,
    pub monitor: Rect,
    pub edge: Edge,
}

impl Zone {
    // work out which edge of its monitor a taskbar is docked to
    pub fn new(taskbar: Rect, monitor: Rect) -> Option<Zone> {
        let wide = taskbar.right - taskbar.left >= taskbar.bottom - taskbar.top;
        let edge = if wide {
            if taskbar.bottom >= monitor.bottom {
                Edge::Bottom
            } else if taskbar.top <= monitor.top {
                Edge::Top
            } else {
                return None;
            }
        } else if taskbar.left <= monitor.left {
            Edge::Left
        } else if taskbar.right >= monitor.right {
            Edge::Right
        } else {
            return None;
        };
        Some(Zone {
            taskbar,
            monitor,
            edge,
        })
    }

    // is the mouse pushed against the screen edge along this taskbar?
    pub fn at_edge(&self, x: i32, y: i32) -> bool {
        let (m, t) = (self.monitor, self.taskbar);
        let along_x = x >= t.left.max(m.left) && x < t.right.min(m.right);
        let along_y = y >= t.top.max(m.top) && y < t.bottom.min(m.bottom);
        match self.edge {
            Edge::Bottom => along_x && y >= m.bottom - EDGE_PX && y < m.bottom,
            Edge::Top => along_x && y >= m.top && y < m.top + EDGE_PX,
            Edge::Left => along_y && x >= m.left && x < m.left + EDGE_PX,
            Edge::Right => along_y && x >= m.right - EDGE_PX && x < m.right,
        }
    }

    // is the mouse over the taskbar itself?
    pub fn over_taskbar(&self, x: i32, y: i32) -> bool {
        self.taskbar.contains(x, y) || self.at_edge(x, y)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Show,
    Hide,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    // just hidden: wait for the mouse to leave the edge so it doesn't pop straight back
    Waiting,
    // ready; `since` is when the mouse reached the edge
    Armed { since: Option<Instant> },
    // peeked in; `kept` is the last time something held it open
    Shown { kept: Instant },
}

pub struct EdgeReveal {
    phase: Phase,
}

impl Default for EdgeReveal {
    fn default() -> Self {
        Self {
            phase: Phase::Waiting,
        }
    }
}

impl EdgeReveal {
    // start over, e.g. right after the taskbar was hidden on purpose
    pub fn reset(&mut self) {
        self.phase = Phase::Waiting;
    }

    pub fn is_shown(&self) -> bool {
        matches!(self.phase, Phase::Shown { .. })
    }

    // advance one tick. `at_edge`: the mouse is pushed against the edge.
    // `keep`: something wants a peeked taskbar to stay (mouse over it, Start open, ...)
    pub fn tick(&mut self, now: Instant, at_edge: bool, keep: bool) -> Option<Action> {
        match self.phase {
            Phase::Waiting => {
                if !at_edge {
                    self.phase = Phase::Armed { since: None };
                }
                None
            }
            Phase::Armed { since } => match (at_edge, since) {
                (false, _) => {
                    self.phase = Phase::Armed { since: None };
                    None
                }
                (true, None) => {
                    self.phase = Phase::Armed { since: Some(now) };
                    None
                }
                (true, Some(t)) if now.duration_since(t) >= DWELL => {
                    self.phase = Phase::Shown { kept: now };
                    Some(Action::Show)
                }
                (true, Some(_)) => None,
            },
            Phase::Shown { kept } => {
                if at_edge || keep {
                    self.phase = Phase::Shown { kept: now };
                    None
                } else if now.duration_since(kept) >= LINGER {
                    self.phase = Phase::Armed { since: None };
                    Some(Action::Hide)
                } else {
                    None
                }
            }
        }
    }
}

fn to_rect(r: RECT) -> Rect {
    Rect {
        left: r.left,
        top: r.top,
        right: r.right,
        bottom: r.bottom,
    }
}

// where every taskbar is, primary first; works while they are hidden
pub fn taskbar_zones() -> Vec<Zone> {
    crate::taskbar::all_taskbars()
        .into_iter()
        .filter_map(|hwnd| unsafe {
            let mut r = RECT::default();
            GetWindowRect(hwnd, &mut r).ok()?;
            let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
            let mut info = MONITORINFO {
                cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                ..Default::default()
            };
            if !GetMonitorInfoW(monitor, &mut info).as_bool() {
                return None;
            }
            Zone::new(to_rect(r), to_rect(info.rcMonitor))
        })
        .collect()
}

// the edge the main taskbar is docked to, for telling the user where to look
pub fn primary_edge() -> Option<Edge> {
    taskbar_zones().first().map(|z| z.edge)
}

pub fn cursor_pos() -> Option<(i32, i32)> {
    let mut p = POINT::default();
    unsafe { GetCursorPos(&mut p).ok()? };
    Some((p.x, p.y))
}

// window classes that belong to the taskbar's own UI: the taskbar, the tray
// overflow, thumbnails, jump lists, Start and search, and popup menus
const SHELL_CLASSES: &[&str] = &[
    "Shell_TrayWnd",
    "Shell_SecondaryTrayWnd",
    "NotifyIconOverflowWindow",
    "TopLevelWindowForOverflowXamlIsland",
    "Xaml_WindowedPopupClass",
    "TaskListThumbnailWnd",
    "Windows.UI.Core.CoreWindow",
    "#32768",
];

fn is_shell_ui(hwnd: HWND) -> bool {
    if hwnd.0.is_null() {
        return false;
    }
    unsafe {
        let root = GetAncestor(hwnd, GA_ROOT);
        let hwnd = if root.0.is_null() { hwnd } else { root };
        // our own windows count too: the tray menu and the settings window
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid as *mut u32));
        if pid == GetCurrentProcessId() {
            return true;
        }
        let mut buf = [0u16; 128];
        let len = GetClassNameW(hwnd, &mut buf);
        if len <= 0 {
            return false;
        }
        let class = String::from_utf16_lossy(&buf[..len as usize]);
        SHELL_CLASSES.contains(&class.as_str())
    }
}

// is part of the taskbar's UI focused or under the mouse?
pub fn shell_ui_active(x: i32, y: i32) -> bool {
    unsafe { is_shell_ui(GetForegroundWindow()) || is_shell_ui(WindowFromPoint(POINT { x, y })) }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MONITOR: Rect = Rect {
        left: 0,
        top: 0,
        right: 1920,
        bottom: 1080,
    };

    fn rect(left: i32, top: i32, right: i32, bottom: i32) -> Rect {
        Rect {
            left,
            top,
            right,
            bottom,
        }
    }

    #[test]
    fn zone_finds_the_docked_edge() {
        let bottom = Zone::new(rect(0, 1032, 1920, 1080), MONITOR).unwrap();
        assert_eq!(bottom.edge, Edge::Bottom);
        let top = Zone::new(rect(0, 0, 1920, 48), MONITOR).unwrap();
        assert_eq!(top.edge, Edge::Top);
        let left = Zone::new(rect(0, 0, 62, 1080), MONITOR).unwrap();
        assert_eq!(left.edge, Edge::Left);
        let right = Zone::new(rect(1858, 0, 1920, 1080), MONITOR).unwrap();
        assert_eq!(right.edge, Edge::Right);
        // floating in the middle of the screen: not docked anywhere
        assert!(Zone::new(rect(100, 500, 1800, 548), MONITOR).is_none());
    }

    #[test]
    fn at_edge_is_only_the_outer_strip_along_the_taskbar() {
        let z = Zone::new(rect(0, 1032, 1920, 1080), MONITOR).unwrap();
        assert!(z.at_edge(960, 1079));
        assert!(z.at_edge(0, 1078));
        assert!(!z.at_edge(960, 1077));
        assert!(!z.at_edge(960, 1040)); // over the taskbar, but not pushed to the edge
        assert!(!z.at_edge(960, 0)); // the opposite edge
        assert!(z.over_taskbar(960, 1040));
        assert!(!z.over_taskbar(960, 1000));
    }

    #[test]
    fn at_edge_works_on_a_second_monitor_and_a_side_taskbar() {
        let second = rect(1920, 0, 3840, 1080);
        let z = Zone::new(rect(1920, 1032, 3840, 1080), second).unwrap();
        assert!(z.at_edge(2500, 1079));
        assert!(!z.at_edge(500, 1079)); // first monitor's bottom edge
        let side = Zone::new(rect(3778, 0, 3840, 1080), second).unwrap();
        assert_eq!(side.edge, Edge::Right);
        assert!(side.at_edge(3839, 500));
        assert!(!side.at_edge(3700, 500));
    }

    fn ms(start: Instant, n: u64) -> Instant {
        start + Duration::from_millis(n)
    }

    #[test]
    fn shows_after_resting_on_the_edge_and_hides_after_leaving() {
        let t0 = Instant::now();
        let mut e = EdgeReveal::default();
        assert_eq!(e.tick(t0, false, false), None); // waiting -> armed
        assert_eq!(e.tick(ms(t0, 100), true, false), None); // reached the edge
        assert_eq!(e.tick(ms(t0, 200), true, false), None); // not long enough yet
        assert_eq!(e.tick(ms(t0, 350), true, false), Some(Action::Show));
        assert!(e.is_shown());
        // over the taskbar: stays
        assert_eq!(e.tick(ms(t0, 2000), false, true), None);
        // moved away: lingers, then hides
        assert_eq!(e.tick(ms(t0, 2500), false, false), None);
        assert_eq!(e.tick(ms(t0, 2800), false, false), Some(Action::Hide));
        assert!(!e.is_shown());
    }

    #[test]
    fn a_quick_pass_over_the_edge_does_nothing() {
        let t0 = Instant::now();
        let mut e = EdgeReveal::default();
        e.tick(t0, false, false);
        assert_eq!(e.tick(ms(t0, 10), true, false), None);
        assert_eq!(e.tick(ms(t0, 100), false, false), None);
        assert_eq!(e.tick(ms(t0, 300), true, false), None); // timer restarted
        assert_eq!(e.tick(ms(t0, 400), true, false), None);
        assert_eq!(e.tick(ms(t0, 560), true, false), Some(Action::Show));
    }

    #[test]
    fn hiding_with_the_mouse_on_the_edge_waits_for_it_to_leave() {
        let t0 = Instant::now();
        let mut e = EdgeReveal::default();
        // the mouse sits on the edge the whole time: never pops back
        for n in 0..20 {
            assert_eq!(e.tick(ms(t0, n * 100), true, false), None);
        }
        assert!(!e.is_shown());
        // after it leaves and comes back, it works again
        assert_eq!(e.tick(ms(t0, 2100), false, false), None);
        assert_eq!(e.tick(ms(t0, 2200), true, false), None);
        assert_eq!(e.tick(ms(t0, 2500), true, false), Some(Action::Show));
    }

    #[test]
    fn reset_puts_it_back_to_waiting() {
        let t0 = Instant::now();
        let mut e = EdgeReveal::default();
        e.tick(t0, false, false);
        e.tick(ms(t0, 10), true, false);
        assert_eq!(e.tick(ms(t0, 300), true, false), Some(Action::Show));
        e.reset();
        assert!(!e.is_shown());
        assert_eq!(e.tick(ms(t0, 900), true, false), None);
    }
}
