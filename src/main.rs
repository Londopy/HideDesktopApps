#![windows_subsystem = "windows"]

mod config;
mod discord;
mod hotkeys;
mod icons;
#[macro_use]
mod log_util;
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

// commands the main loop can receive from tray, hotkeys, or the settings window
pub enum Cmd {
    ToggleIcons,
    ToggleTaskbar,
    ToggleWindows,
    ApplyProfile(String),
    ConfigUpdated(AppConfig),
    OpenSettings,
    Restart,
    Exit,
    UpdateAvailable(String),
    UpToDate,
    HotkeyFailed(String),
}

fn main() {
    // if we panic, try to restore icons/taskbar before crashing
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        dlog!("PANIC: {info}");
        eprintln!("PANIC: {info}");
        let _ = icons::show_icons();
        let _ = taskbar::show_taskbar();
        // Re-show any app windows we'd hidden, so a crash can't strand them.
        let _ = win_ops::recover_hidden_windows();
        default_hook(info);
    }));

    if let Err(e) = run() {
        let _ = icons::show_icons();
        let _ = taskbar::show_taskbar();
        dlog!("Fatal error: {e:?}");
        eprintln!("Fatal error: {e:?}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    // Register AppUserModelId so Windows toast notifications work.
    startup::setup_aumid();

    dlog!("--- HideDesktopApps starting ---");
    let config = config::load_config()?;
    let config_shared = Arc::new(Mutex::new(config.clone()));
    let state_shared = Arc::new(Mutex::new(AppState::default()));

    // Re-show any app windows stranded hidden by a previous crashed session.
    let recovered = win_ops::recover_hidden_windows();
    if recovered > 0 {
        dlog!("Recovered {recovered} hidden window(s) from a previous session");
    }

    // Sync initial state from the actual desktop. Windows persists desktop-icon
    // visibility across reboots, so if we boot with icons already hidden the tray
    // icon must reflect that instead of the default "visible" state.
    {
        let mut state = state_shared.lock().unwrap();
        state.icons_hidden = !icons::are_icons_visible();
    }

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
        .ok();
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
            dlog!("Default profile error: {e}");
            eprintln!("Default profile error: {e}");
        }
        // Refresh the tray so it reflects the profile we just applied.
        tray::update_tray(&tray_handle, &state);
    } else {
        // No default profile, but the synced boot state may differ from the
        // tray icon built from AppState::default(); refresh to match.
        let state = state_shared.lock().unwrap();
        tray::update_tray(&tray_handle, &state);
    }

    // Schedule first update check.
    let interval = Duration::from_secs(config.updater.check_interval_h as u64 * 3600);
    let mut last_update_check = Instant::now()
        .checked_sub(interval)
        .unwrap_or_else(Instant::now);

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
    let mut tray_handle = tray_handle;

    // debounce hotkeys — windows repeats WM_HOTKEY while you hold a key down
    let debounce = Duration::from_millis(400);
    let epoch = Instant::now()
        .checked_sub(debounce)
        .unwrap_or_else(Instant::now);
    let mut last_icons_fire = epoch;
    let mut last_taskbar_fire = epoch;
    let mut last_windows_fire = epoch;

    loop {
        // pump win32 messages so tray and hotkeys work
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

        std::thread::sleep(Duration::from_millis(16));

        // check hotkey events
        while let Some(event) = hotkeys::poll_hotkey_event() {
            let id = event.id();
            if id == hotkey_reg.icons_id {
                if last_icons_fire.elapsed() >= debounce {
                    last_icons_fire = Instant::now();
                    let _ = cmd_tx.send(Cmd::ToggleIcons);
                }
            } else if id == hotkey_reg.taskbar_id {
                if last_taskbar_fire.elapsed() >= debounce {
                    last_taskbar_fire = Instant::now();
                    let _ = cmd_tx.send(Cmd::ToggleTaskbar);
                }
            } else if id == hotkey_reg.windows_id {
                if last_windows_fire.elapsed() >= debounce {
                    last_windows_fire = Instant::now();
                    let _ = cmd_tx.send(Cmd::ToggleWindows);
                }
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

        // check tray menu clicks
        while let Some(event) = tray::poll_menu_event() {
            dlog!("menu event: id={:?}", event.id);
            let id = &event.id;
            if id == &tray_handle.ids.toggle_icons {
                let _ = cmd_tx.send(Cmd::ToggleIcons);
            } else if id == &tray_handle.ids.toggle_taskbar {
                let _ = cmd_tx.send(Cmd::ToggleTaskbar);
            } else if id == &tray_handle.ids.toggle_windows {
                let _ = cmd_tx.send(Cmd::ToggleWindows);
            } else if id == &tray_handle.ids.settings {
                let _ = cmd_tx.send(Cmd::OpenSettings);
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

        // tray left click = toggle icons
        while let Some(event) = tray::poll_tray_event() {
            use tray_icon::{MouseButton, MouseButtonState, TrayIconEvent};
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                dlog!("tray left-click: ToggleIcons");
                let _ = cmd_tx.send(Cmd::ToggleIcons);
            }
        }

        // run auto update check if enough time has passed
        {
            let cfg = config_shared.lock().unwrap().clone();
            let interval = Duration::from_secs(cfg.updater.check_interval_h as u64 * 3600);
            if cfg.updater.enabled && last_update_check.elapsed() >= interval {
                *last_update_check = Instant::now();
                updater::background_check(cfg.updater.clone(), cmd_tx.clone(), false);
            }
        }

        // handle commands from tray, hotkeys, and settings
        while let Ok(cmd) = cmd_rx.try_recv() {
            match cmd {
                Cmd::ToggleIcons => {
                    dlog!("Cmd::ToggleIcons received");
                    let mut state = state_shared.lock().unwrap();
                    if state.icons_hidden {
                        if let Err(e) = icons::show_icons() {
                            dlog!("show_icons error: {e}");
                            eprintln!("show_icons error: {e}");
                        } else {
                            state.icons_hidden = false;
                        }
                    } else {
                        if let Err(e) = icons::hide_icons() {
                            dlog!("hide_icons error: {e}");
                            eprintln!("hide_icons error: {e}");
                        } else {
                            state.icons_hidden = true;
                        }
                    }
                    tray::update_tray(&tray_handle, &state);
                    update_discord(&state, &config_shared.lock().unwrap());
                }

                Cmd::ToggleTaskbar => {
                    dlog!("Cmd::ToggleTaskbar received");
                    let mut state = state_shared.lock().unwrap();
                    if state.taskbar_hidden {
                        if let Err(e) = taskbar::show_taskbar() {
                            dlog!("show_taskbar error: {e}");
                            eprintln!("show_taskbar error: {e}");
                        } else {
                            state.taskbar_hidden = false;
                        }
                    } else {
                        if let Err(e) = taskbar::hide_taskbar() {
                            dlog!("hide_taskbar error: {e}");
                            eprintln!("hide_taskbar error: {e}");
                        } else {
                            state.taskbar_hidden = true;
                        }
                    }
                    tray::update_tray(&tray_handle, &state);
                    update_discord(&state, &config_shared.lock().unwrap());
                }

                Cmd::ToggleWindows => {
                    dlog!("Cmd::ToggleWindows received");
                    let mut state = state_shared.lock().unwrap();
                    let cfg = config_shared.lock().unwrap().clone();
                    if state.windows_hidden {
                        if let Err(e) = win_ops::restore_windows(&state.hidden_windows) {
                            dlog!("restore_windows error: {e}");
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
                            Err(e) => {
                                dlog!("hide_windows error: {e}");
                                eprintln!("hide_windows error: {e}");
                            }
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
                        Err(e) => {
                            dlog!("apply_profile error: {e}");
                            eprintln!("apply_profile error: {e}");
                        }
                    }
                    tray::update_tray(&tray_handle, &state);
                    update_discord(&state, &config_shared.lock().unwrap());
                }

                Cmd::ConfigUpdated(new_cfg) => {
                    {
                        let mut shared = config_shared.lock().unwrap();
                        *shared = new_cfg.clone();
                    }
                    // update hotkeys
                    hotkeys::reregister_hotkeys(hotkey_reg, &new_cfg.hotkeys, &cmd_tx);

                    // update startup task
                    let exe_path = win_ops::current_exe_path();
                    startup::sync_startup(&new_cfg.startup, &exe_path);

                    // rebuild tray to update the profiles submenu
                    let state = state_shared.lock().unwrap();
                    if let Ok(new_tray) = tray::build_tray(&state, &new_cfg.profiles) {
                        tray_handle = new_tray;
                    }
                }

                Cmd::OpenSettings => {
                    dlog!("Cmd::OpenSettings received");
                    // runs in a background thread, main loop keeps going
                    ui::open_settings(Arc::clone(&config_shared), cmd_tx.clone());
                }

                Cmd::Restart => {
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
                    let mut state = state_shared.lock().unwrap();
                    profiles::restore_all(&mut state);
                    return Ok(());
                }

                Cmd::UpdateAvailable(version) => {
                    let cfg = config_shared.lock().unwrap().clone();
                    notifications::notify_update_available(&version, &cfg.notifications);
                    dlog!("Update available: {version}");
                    eprintln!("Update available: {version}");
                }

                Cmd::UpToDate => {
                    let cfg = config_shared.lock().unwrap().clone();
                    notifications::notify_up_to_date(&cfg.notifications);
                }

                Cmd::HotkeyFailed(hotkey) => {
                    let cfg = config_shared.lock().unwrap().clone();
                    notifications::notify_hotkey_failed(&hotkey, &cfg.notifications);
                }
            }
        }
    }
}

// update discord rich presence if it's enabled
fn update_discord(state: &AppState, config: &AppConfig) {
    if config.discord.enabled {
        discord::set_rich_presence(
            state.icons_hidden,
            state.taskbar_hidden,
            state.windows_hidden,
            state.active_profile.clone(),
        );
    }
}
