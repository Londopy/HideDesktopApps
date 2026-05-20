use crate::config::{AppConfig, ProfileConfig};
use crate::state::AppState;
use crate::{icons, taskbar, win_ops};
use anyhow::Result;

/// Apply a named profile: hide/show icons, taskbar, windows as specified.
pub fn apply_profile(name: &str, config: &AppConfig, state: &mut AppState) -> Result<()> {
    let profile = config
        .profiles
        .iter()
        .find(|p| p.name == name)
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("Profile '{}' not found", name))?;

    apply_profile_config(&profile, config, state)?;
    state.active_profile = Some(name.to_string());
    Ok(())
}

/// Apply a ProfileConfig directly.
pub fn apply_profile_config(
    profile: &ProfileConfig,
    config: &AppConfig,
    state: &mut AppState,
) -> Result<()> {
    // Icons
    if profile.icons && !state.icons_hidden {
        icons::hide_icons()?;
        state.icons_hidden = true;
    } else if !profile.icons && state.icons_hidden {
        icons::show_icons()?;
        state.icons_hidden = false;
    }

    // Taskbar
    if profile.taskbar && !state.taskbar_hidden {
        taskbar::hide_taskbar()?;
        state.taskbar_hidden = true;
    } else if !profile.taskbar && state.taskbar_hidden {
        taskbar::show_taskbar()?;
        state.taskbar_hidden = false;
    }

    // Windows
    if profile.windows && !state.windows_hidden {
        match win_ops::hide_windows(&config.window_filter.exclude_processes) {
            Ok(hidden) => {
                state.hidden_windows = hidden;
                state.windows_hidden = true;
            }
            Err(e) => {
                eprintln!("Failed to hide windows: {e}");
            }
        }
    } else if !profile.windows && state.windows_hidden {
        win_ops::restore_windows(&state.hidden_windows)?;
        state.hidden_windows.clear();
        state.windows_hidden = false;
    }

    Ok(())
}

/// Restore everything to visible state.
pub fn restore_all(state: &mut AppState) {
    if state.icons_hidden {
        if let Err(e) = icons::show_icons() {
            eprintln!("Error showing icons: {e}");
        }
        state.icons_hidden = false;
    }

    if state.taskbar_hidden {
        if let Err(e) = taskbar::show_taskbar() {
            eprintln!("Error showing taskbar: {e}");
        }
        state.taskbar_hidden = false;
    }

    if state.windows_hidden {
        if let Err(e) = win_ops::restore_windows(&state.hidden_windows) {
            eprintln!("Error restoring windows: {e}");
        }
        state.hidden_windows.clear();
        state.windows_hidden = false;
    }

    state.active_profile = None;
}
