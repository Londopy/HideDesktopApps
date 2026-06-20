use anyhow::Result;
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder, TrayIconEvent};

use crate::config::ProfileConfig;
use crate::state::AppState;

// holds menu item ids, including one per profile
#[derive(Clone)]
pub struct TrayMenuIds {
    pub toggle_icons: tray_icon::menu::MenuId,
    pub toggle_taskbar: tray_icon::menu::MenuId,
    pub toggle_windows: tray_icon::menu::MenuId,
    pub show_everything: tray_icon::menu::MenuId,
    pub settings: tray_icon::menu::MenuId,
    pub restart: tray_icon::menu::MenuId,
    pub exit: tray_icon::menu::MenuId,
    // (menu-item id, profile name) pairs for the Profiles submenu
    pub profiles: Vec<(tray_icon::menu::MenuId, String)>,
}

pub struct TrayHandle {
    pub tray: TrayIcon,
    pub ids: TrayMenuIds,
}

pub const ICON_SIZE: usize = 64;

// pixel drawing helpers

fn set_pixel(buf: &mut [u8], x: usize, y: usize, color: [u8; 4]) {
    if x >= ICON_SIZE || y >= ICON_SIZE {
        return;
    }
    let idx = (y * ICON_SIZE + x) * 4;
    buf[idx..idx + 4].copy_from_slice(&color);
}

fn fill_rect(buf: &mut [u8], x: usize, y: usize, w: usize, h: usize, color: [u8; 4]) {
    for dy in 0..h {
        for dx in 0..w {
            set_pixel(buf, x + dx, y + dy, color);
        }
    }
}

// bresenham's line algorithm
fn draw_line(buf: &mut [u8], x0: isize, y0: isize, x1: isize, y1: isize, color: [u8; 4]) {
    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let sx: isize = if x0 < x1 { 1 } else { -1 };
    let sy: isize = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    let (mut x, mut y) = (x0, y0);

    loop {
        if x >= 0 && y >= 0 {
            set_pixel(buf, x as usize, y as usize, color);
        }
        if x == x1 && y == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
    }
}

// fills a circle
fn fill_circle(buf: &mut [u8], cx: isize, cy: isize, r: isize, color: [u8; 4]) {
    for dy in -r..=r {
        for dx in -r..=r {
            if dx * dx + dy * dy <= r * r {
                let px = cx + dx;
                let py = cy + dy;
                if px >= 0 && py >= 0 {
                    set_pixel(buf, px as usize, py as usize, color);
                }
            }
        }
    }
}

// colors

const BLUE: [u8; 4] = [52, 152, 219, 255];
const GREEN: [u8; 4] = [46, 204, 113, 255];
const RED_QUAD: [u8; 4] = [231, 76, 60, 255];
const ORANGE: [u8; 4] = [243, 156, 18, 255];
const GREY: [u8; 4] = [127, 140, 141, 255];
const WHITE: [u8; 4] = [255, 255, 255, 255];
const X_RED: [u8; 4] = [231, 76, 60, 255];
const ORANGE_DOT: [u8; 4] = [230, 126, 34, 255];
const TRANSPARENT: [u8; 4] = [0, 0, 0, 0];

// draw the big red X when things are hidden (3px wide)
fn draw_x(buf: &mut [u8]) {
    let s = ICON_SIZE as isize;
    for offset in -1isize..=1 {
        draw_line(buf, 0, offset, s - 1, s - 1 + offset, X_RED);
        draw_line(buf, offset, 0, s - 1 + offset, s - 1, X_RED);
        draw_line(buf, 0, s - 1 - offset, s - 1, -offset, X_RED);
        draw_line(buf, offset, s - 1, s - 1 + offset, 0, X_RED);
    }
}

// generate the icon pixels based on what's currently hidden
pub fn build_icon_rgba(state: &AppState) -> Vec<u8> {
    let total = ICON_SIZE * ICON_SIZE * 4;
    let mut buf = vec![0u8; total];

    for chunk in buf.chunks_mut(4) {
        chunk.copy_from_slice(&TRANSPARENT);
    }

    let half = ICON_SIZE / 2;
    let border = 2;

    if state.icons_hidden || state.windows_hidden {
        // grey + X when stuff is hidden
        fill_rect(&mut buf, 0, 0, ICON_SIZE, ICON_SIZE, GREY);
        draw_x(&mut buf);
    } else {
        // 4 colored blocks (normal state)
        // Top-left: Blue
        fill_rect(&mut buf, 0, 0, half - border, half - border, BLUE);
        // Top-right: Green
        fill_rect(
            &mut buf,
            half + border,
            0,
            half - border,
            half - border,
            GREEN,
        );
        // Bottom-left: Red
        fill_rect(
            &mut buf,
            0,
            half + border,
            half - border,
            half - border,
            RED_QUAD,
        );
        // Bottom-right: Orange
        fill_rect(
            &mut buf,
            half + border,
            half + border,
            half - border,
            half - border,
            ORANGE,
        );

        // White cross between quadrants
        fill_rect(&mut buf, half - border, 0, border * 2, ICON_SIZE, WHITE);
        fill_rect(&mut buf, 0, half - border, ICON_SIZE, border * 2, WHITE);
    }

    // orange dot = taskbar hidden
    if state.taskbar_hidden {
        fill_circle(
            &mut buf,
            ICON_SIZE as isize - 12,
            ICON_SIZE as isize - 12,
            10,
            ORANGE_DOT,
        );
    }

    buf
}

// build the icon from current state
fn make_icon(state: &AppState) -> Result<Icon> {
    let rgba = build_icon_rgba(state);
    Ok(Icon::from_rgba(rgba, ICON_SIZE as u32, ICON_SIZE as u32)?)
}

// build the hover tooltip text
fn build_tooltip(state: &AppState) -> String {
    // Easter egg: icons, taskbar AND windows all gone — the desktop has fully
    // vanished, so reward the curious with ninja mode instead of a dry list.
    if state.icons_hidden && state.taskbar_hidden && state.windows_hidden {
        return "HideDesktopApps — 🥷 Ninja mode: poof, your desktop vanished".to_string();
    }

    let mut parts = Vec::new();
    if state.icons_hidden {
        parts.push("Icons: hidden");
    }
    if state.taskbar_hidden {
        parts.push("Taskbar: hidden");
    }
    if state.windows_hidden {
        parts.push("Windows: hidden");
    }
    if parts.is_empty() {
        "HideDesktopApps — all visible".to_string()
    } else {
        format!("HideDesktopApps — {}", parts.join(", "))
    }
}

pub fn build_tray(state: &AppState, profiles: &[ProfileConfig]) -> Result<TrayHandle> {
    let toggle_icons_item = MenuItem::new("Toggle Desktop Icons\tCtrl+Alt+H", true, None);
    let toggle_taskbar_item = MenuItem::new("Toggle Taskbar\tCtrl+Alt+T", true, None);
    let toggle_windows_item = MenuItem::new("Toggle App Windows\tCtrl+Alt+W", true, None);
    let show_everything_item = MenuItem::new("Show Everything", true, None);
    let settings_item = MenuItem::new("Settings...", true, None);
    let restart_item = MenuItem::new("Restart", true, None);
    let exit_item = MenuItem::new("Exit", true, None);

    // add a profiles submenu, one item per profile
    let profiles_submenu = Submenu::new("Profiles", true);
    let mut profile_ids: Vec<(tray_icon::menu::MenuId, String)> = Vec::new();

    if profiles.is_empty() {
        // need at least one item in the submenu or it breaks
        let none_item = MenuItem::new("(no profiles)", false, None);
        profiles_submenu.append(&none_item)?;
    } else {
        for profile in profiles {
            let item = MenuItem::new(&profile.name, true, None);
            profile_ids.push((item.id().clone(), profile.name.clone()));
            profiles_submenu.append(&item)?;
        }
    }

    let ids = TrayMenuIds {
        toggle_icons: toggle_icons_item.id().clone(),
        toggle_taskbar: toggle_taskbar_item.id().clone(),
        toggle_windows: toggle_windows_item.id().clone(),
        show_everything: show_everything_item.id().clone(),
        settings: settings_item.id().clone(),
        restart: restart_item.id().clone(),
        exit: exit_item.id().clone(),
        profiles: profile_ids,
    };

    let menu = Menu::new();
    menu.append(&toggle_icons_item)?;
    menu.append(&toggle_taskbar_item)?;
    menu.append(&toggle_windows_item)?;
    menu.append(&show_everything_item)?;
    menu.append(&PredefinedMenuItem::separator())?;
    menu.append(&profiles_submenu)?;
    menu.append(&PredefinedMenuItem::separator())?;
    menu.append(&settings_item)?;
    menu.append(&PredefinedMenuItem::separator())?;
    menu.append(&restart_item)?;
    menu.append(&exit_item)?;

    let icon = make_icon(state)?;
    let tooltip = build_tooltip(state);

    let tray = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_icon(icon)
        .with_tooltip(tooltip)
        .with_menu_on_left_click(false)
        .build()?;

    Ok(TrayHandle { tray, ids })
}

// refresh the tray icon and tooltip
pub fn update_tray(handle: &TrayHandle, state: &AppState) {
    if let Ok(icon) = make_icon(state) {
        let _ = handle.tray.set_icon(Some(icon));
    }
    let tooltip = build_tooltip(state);
    let _ = handle.tray.set_tooltip(Some(tooltip));
}

// check for menu clicks (non-blocking)
pub fn poll_menu_event() -> Option<MenuEvent> {
    MenuEvent::receiver().try_recv().ok()
}

// check for tray icon clicks (non-blocking)
pub fn poll_tray_event() -> Option<TrayIconEvent> {
    TrayIconEvent::receiver().try_recv().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::AppState;

    #[test]
    fn tooltip_all_hidden_triggers_ninja_easter_egg() {
        let state = AppState {
            icons_hidden: true,
            taskbar_hidden: true,
            windows_hidden: true,
            ..Default::default()
        };
        assert!(build_tooltip(&state).contains("Ninja mode"));
    }

    #[test]
    fn tooltip_partial_hidden_is_normal() {
        let state = AppState {
            icons_hidden: true,
            taskbar_hidden: true,
            windows_hidden: false,
            ..Default::default()
        };
        let tip = build_tooltip(&state);
        assert!(!tip.contains("Ninja"));
        assert!(tip.contains("Icons: hidden"));
        assert!(tip.contains("Taskbar: hidden"));
    }

    #[test]
    fn tooltip_nothing_hidden_says_all_visible() {
        assert!(build_tooltip(&AppState::default()).contains("all visible"));
    }
}
