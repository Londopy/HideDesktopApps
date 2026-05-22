#![windows_subsystem = "windows"]

mod config;
mod discord;
mod hotkeys;
mod icons;
mod notifications;
mod profiles;
mod startup;
mod state;
mod taskbar;
mod tray;
mod ui;
mod updater;
mod win_ops;

use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::Result;
use config::AppConfig;
use state::AppState;

/// Commands sent to the main loop from tray, hotkey, or settings threads.
pub enum Cmd {
    ToggleIcons,
    ToggleTaskbar,
    ToggleWindows,
    ApplyProfile(String),
    ConfigUpdated(AppConfig),
    OpenSettings,
    CheckForUpdates,
    Restart,
    Exit,
    UpdateAvailable(String),
    HotkeyFailed(String),
}

fn main() {
    // Install a panic handler that restores the desktop before propagating.
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        eprintln!("PANIC: {info}");
        // Attempt best-effort restore; we cannot access state here so we call
        // the Win32 functions directly.
        let _ = icons::show_icons();
        let _ = taskbar::show_taskbar();
        default_hook(info);
    }));

    if let Err(e) = run() {
        // Restore everything before exiting on a fatal error.
        let _ = icons::show_icons();
        let _ = taskbar::show_taskbar();
        eprintln!("Fatal error: {e:?}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let config = config::load_config()?;
    let config_shared = Arc::new(Mutex::new(config.clone()));
    let state_shared = Arc::new(Mutex::new(AppState::default()));

    let (cmd_tx, cmd_rx) = mpsc::channel::<Cmd>();

    // Ctrl+C / SIGTERM: restore the desktop before the process dies.
    {
        let state_for_ctrlc = Arc::clone(&state_shared);
        let tx_for_ctrlc = cmd_tx.clone();
        ctrlc::set_handler(move || {
            let mut state = state_for_ctrlc.lock().unwrap();
            profiles::restore_all(&mut state);
            drop(state);
            let _ = tx_for_ctrlc.send(Cmd::Exit);
        })
        .ok(); // non-fatal if it fails
    }

    // Register hotkeys
    let mut hotkey_reg = hotkeys::register_hotkeys(&config.hotkeys, &cmd_tx)?;

    // Build tray (pass profiles so the Profiles submenu is populated)
    let state_snapshot = state_shared.lock().unwrap();
    let tray_handle = tray::build_tray(&state_snapshot, &config.profiles)?;
    drop(state_snapshot);

    // Apply startup config
    let exe_path = win_ops::current_exe_path();
    startup::sync_startup(&config.startup, &exe_path);

    // Apply default profile if configured
    if !config.defaults.profile.is_empty() {
        let mut state = state_shared.lock().unwrap();
        let cfg = config_shared.lock().unwrap().clone();
        if let Err(e) = profiles::apply_profile(&config.defaults.profile, &cfg, &mut state) {
            eprintln!("Default profile error: {e}");
        }
    }

    // Schedule first update check
    let mut last_update_check =
        Instant::now() - Duration::from_secs(config.updater.check_interval_h as u64 * 3600);

    main_loop(
        cmd_rx,
        cmd_tx.clone(),
        config_shared,
        state_shared,
        tray_handle,
        &mut hotkey_reg,
        &mut last_update_check,
    )
}

#[allow(clippy::too_many_arguments)]
fn main_loop(
    cmd_rx: mpsc::Receiver<Cmd>,
    cmd_tx: mpsc::Sender<Cmd>,
    config_shared: Arc<Mutex<AppConfig>>,
    state_shared: Arc<Mutex<AppState>>,
    tray_handle: tray::TrayHandle,
    hotkey_reg: &mut hotkeys::RegisteredHotkeys,
    last_update_check: &mut Instant,
) -> Result<()> {
    // Owned so we can swap it out when the profile list changes.
    let mut tray_handle = tray_handle;

    loop {
        // Pump Win32 messages — tray_icon and global_hotkey both rely on a
        // message loop on Windows. Without this the tray icon never appears
        // and hotkeys never fire.
        #[cfg(target_os = "windows")]
        unsafe {
            use windows::Win32::UI::WindowsAndMessaging::{
                DispatchMessageW, PeekMessageW, TranslateMessage, MSG, PM_REMOVE,
            };
            let mut msg = MSG::default();
            while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }

        std::thread::sleep(Duration::from_millis(50));

        // ── Poll hotkey events ────────────────────────────────────────────
        while let Some(event) = hotkeys::poll_hotkey_event() {
            let id = event.id();
            if id == hotkey_reg.icons_id {
                let _ = cmd_tx.send(Cmd::ToggleIcons);
            } else if id == hotkey_reg.taskbar_id {
                let _ = cmd_tx.send(Cmd::ToggleTaskbar);
            } else if id == hotkey_reg.windows_id {
                let _ = cmd_tx.send(Cmd::ToggleWindows);
            } else {
                // Check profile hotkeys
                let cfg = config_shared.lock().unwrap().clone();
                for profile in &cfg.profiles {
                    if !profile.hotkey.is_empty() {
                        if let Ok(hk) = hotkeys::parse_hotkey(&profile.hotkey) {
                            if hk.id() == id {
                                let _ = cmd_tx.send(Cmd::ApplyProfile(profile.name.clone()));
                                break;
                            }
                        }
                    }
                }
            }
        }

        // ── Poll tray menu events ─────────────────────────────────────────
        while let Some(event) = tray::poll_menu_event() {
            let id = &event.id;
            if id == &tray_handle.ids.toggle_icons {
                let _ = cmd_tx.send(Cmd::ToggleIcons);
            } else if id == &tray_handle.ids.toggle_taskbar {
                let _ = cmd_tx.send(Cmd::ToggleTaskbar);
            } else if id == &tray_handle.ids.toggle_windows {
                let _ = cmd_tx.send(Cmd::ToggleWindows);
            } else if id == &tray_handle.ids.settings {
                let _ = cmd_tx.send(Cmd::OpenSettings);
            } else if id == &tray_handle.ids.check_updates {
                let _ = cmd_tx.send(Cmd::CheckForUpdates);
            } else if id == &tray_handle.ids.restart {
                let _ = cmd_tx.send(Cmd::Restart);
            } else if id == &tray_handle.ids.exit {
                let _ = cmd_tx.send(Cmd::Exit);
            } else {
                // Check if it's a profile from the submenu.
                for (profile_id, profile_name) in &tray_handle.ids.profiles {
                    if id == profile_id {
                        let _ = cmd_tx.send(Cmd::ApplyProfile(profile_name.clone()));
                        break;
                    }
                }
            }
        }

        // ── Poll tray icon events (double-click opens settings) ───────────
        while let Some(event) = tray::poll_tray_event() {
            use tray_icon::{MouseButton, MouseButtonState, TrayIconEvent};
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let _ = cmd_tx.send(Cmd::ToggleIcons);
            }
            if let TrayIconEvent::DoubleClick {
                button: MouseButton::Left,
                ..
            } = event
            {
                let _ = cmd_tx.send(Cmd::OpenSettings);
            }
        }

        // ── Scheduled update check ────────────────────────────────────────
        {
            let cfg = config_shared.lock().unwrap().clone();
            let interval = Duration::from_secs(cfg.updater.check_interval_h as u64 * 3600);
            if cfg.updater.enabled && last_update_check.elapsed() >= interval {
                *last_update_check = Instant::now();
                updater::background_check(cfg.updater.clone(), cmd_tx.clone());
            }
        }

        // ── Process commands ──────────────────────────────────────────────
        while let Ok(cmd) = cmd_rx.try_recv() {
            match cmd {
                Cmd::ToggleIcons => {
                    let mut state = state_shared.lock().unwrap();
                    if state.icons_hidden {
                        if let Err(e) = icons::show_icons() {
                            eprintln!("show_icons error: {e}");
                        } else {
                            state.icons_hidden = false;
                        }
                    } else {
                        if let Err(e) = icons::hide_icons() {
                            eprintln!("hide_icons error: {e}");
                        } else {
                            state.icons_hidden = true;
                        }
                    }
                    tray::update_tray(&tray_handle, &state);
                    update_discord(&state, &config_shared.lock().unwrap());
                }

                Cmd::ToggleTaskbar => {
                    let mut state = state_shared.lock().unwrap();
                    if state.taskbar_hidden {
                        if let Err(e) = taskbar::show_taskbar() {
                            eprintln!("show_taskbar error: {e}");
                        } else {
                            state.taskbar_hidden = false;
                        }
                    } else {
                        if let Err(e) = taskbar::hide_taskbar() {
                            eprintln!("hide_taskbar error: {e}");
                        } else {
                            state.taskbar_hidden = true;
                        }
                    }
                    tray::update_tray(&tray_handle, &state);
                    update_discord(&state, &config_shared.lock().unwrap());
                }

                Cmd::ToggleWindows => {
                    let mut state = state_shared.lock().unwrap();
                    let cfg = config_shared.lock().unwrap().clone();
                    if state.windows_hidden {
                        if let Err(e) = win_ops::restore_windows(&state.hidden_windows) {
                            eprintln!("restore_windows error: {e}");
                        } else {
                            state.hidden_windows.clear();
                            state.windows_hidden = false;
                        }
                    } else {
                        match win_ops::hide_windows(&cfg.window_filter.exclude_processes) {
                            Ok(hidden) => {
                                state.hidden_windows = hidden;
                                state.windows_hidden = true;
                            }
                            Err(e) => eprintln!("hide_windows error: {e}"),
                        }
                    }
                    tray::update_tray(&tray_handle, &state);
                    update_discord(&state, &config_shared.lock().unwrap());
                }

                Cmd::ApplyProfile(name) => {
                    let mut state = state_shared.lock().unwrap();
                    let cfg = config_shared.lock().unwrap().clone();
                    match profiles::apply_profile(&name, &cfg, &mut state) {
                        Ok(()) => {
                            notifications::notify_profile_switch(&name, &cfg.notifications);
                        }
                        Err(e) => eprintln!("apply_profile error: {e}"),
                    }
                    tray::update_tray(&tray_handle, &state);
                    update_discord(&state, &config_shared.lock().unwrap());
                }

                Cmd::ConfigUpdated(new_cfg) => {
                    {
                        let mut shared = config_shared.lock().unwrap();
                        *shared = new_cfg.clone();
                    }
                    // Re-register hotkeys with new config
                    hotkeys::reregister_hotkeys(hotkey_reg, &new_cfg.hotkeys, &cmd_tx);

                    // Sync startup setting
                    let exe_path = win_ops::current_exe_path();
                    startup::sync_startup(&new_cfg.startup, &exe_path);

                    // Rebuild tray so the Profiles submenu reflects the new list.
                    let state = state_shared.lock().unwrap();
                    if let Ok(new_tray) = tray::build_tray(&state, &new_cfg.profiles) {
                        tray_handle = new_tray;
                    }
                }

                Cmd::OpenSettings => {
                    ui::open_settings(Arc::clone(&config_shared), cmd_tx.clone());
                }

                Cmd::CheckForUpdates => {
                    let cfg = config_shared.lock().unwrap().clone();
                    updater::background_check(cfg.updater, cmd_tx.clone());
                }

                Cmd::Restart => {
                    // Restore all before restarting
                    {
                        let mut state = state_shared.lock().unwrap();
                        profiles::restore_all(&mut state);
                    }
                    let exe = std::env::current_exe()
                        .unwrap_or_else(|_| std::path::PathBuf::from("HideDesktopApps.exe"));
                    let _ = std::process::Command::new(exe).spawn();
                    return Ok(());
                }

                Cmd::Exit => {
                    // Restore everything cleanly before exiting
                    let mut state = state_shared.lock().unwrap();
                    profiles::restore_all(&mut state);
                    return Ok(());
                }

                Cmd::UpdateAvailable(version) => {
                    let cfg = config_shared.lock().unwrap().clone();
                    notifications::notify_update_available(&version, &cfg.notifications);
                    eprintln!("Upd