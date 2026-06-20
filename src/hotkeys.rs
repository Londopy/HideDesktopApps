use anyhow::{bail, Result};
use global_hotkey::hotkey::{Code, HotKey, Modifiers};
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager};

// turns "ctrl+alt+h" into a HotKey
pub fn parse_hotkey(s: &str) -> Result<HotKey> {
    let parts: Vec<&str> = s.split('+').collect();
    if parts.len() < 2 {
        bail!("Hotkey '{}' must have at least one modifier and one key", s);
    }

    let key_part = parts.last().unwrap().trim().to_lowercase();
    let mod_parts = &parts[..parts.len() - 1];

    let mut modifiers = Modifiers::empty();
    for m in mod_parts {
        match m.trim().to_lowercase().as_str() {
            "ctrl" | "control" => modifiers |= Modifiers::CONTROL,
            "alt" => modifiers |= Modifiers::ALT,
            "shift" => modifiers |= Modifiers::SHIFT,
            "win" | "super" | "meta" => modifiers |= Modifiers::SUPER,
            other => bail!("Unknown modifier: '{}'", other),
        }
    }

    if modifiers.is_empty() {
        bail!("Hotkey '{}' must have at least one modifier", s);
    }

    let code = parse_key_code(&key_part)?;
    Ok(HotKey::new(Some(modifiers), code))
}

// maps a key name like "h" to a Code
fn parse_key_code(key: &str) -> Result<Code> {
    let code = match key {
        "a" => Code::KeyA,
        "b" => Code::KeyB,
        "c" => Code::KeyC,
        "d" => Code::KeyD,
        "e" => Code::KeyE,
        "f" => Code::KeyF,
        "g" => Code::KeyG,
        "h" => Code::KeyH,
        "i" => Code::KeyI,
        "j" => Code::KeyJ,
        "k" => Code::KeyK,
        "l" => Code::KeyL,
        "m" => Code::KeyM,
        "n" => Code::KeyN,
        "o" => Code::KeyO,
        "p" => Code::KeyP,
        "q" => Code::KeyQ,
        "r" => Code::KeyR,
        "s" => Code::KeyS,
        "t" => Code::KeyT,
        "u" => Code::KeyU,
        "v" => Code::KeyV,
        "w" => Code::KeyW,
        "x" => Code::KeyX,
        "y" => Code::KeyY,
        "z" => Code::KeyZ,
        "0" | "digit0" => Code::Digit0,
        "1" | "digit1" => Code::Digit1,
        "2" | "digit2" => Code::Digit2,
        "3" | "digit3" => Code::Digit3,
        "4" | "digit4" => Code::Digit4,
        "5" | "digit5" => Code::Digit5,
        "6" | "digit6" => Code::Digit6,
        "7" | "digit7" => Code::Digit7,
        "8" | "digit8" => Code::Digit8,
        "9" | "digit9" => Code::Digit9,
        "f1" => Code::F1,
        "f2" => Code::F2,
        "f3" => Code::F3,
        "f4" => Code::F4,
        "f5" => Code::F5,
        "f6" => Code::F6,
        "f7" => Code::F7,
        "f8" => Code::F8,
        "f9" => Code::F9,
        "f10" => Code::F10,
        "f11" => Code::F11,
        "f12" => Code::F12,
        other => bail!("Unknown key: '{}'", other),
    };
    Ok(code)
}

// stores the registered hotkeys so we can unregister them later
pub struct RegisteredHotkeys {
    pub manager: GlobalHotKeyManager,
    pub icons_id: u32,
    pub taskbar_id: u32,
    pub windows_id: u32,
    icons_hk: HotKey,
    taskbar_hk: HotKey,
    windows_hk: HotKey,
}

// parse a hotkey string, fall back to the default if it fails
fn parse_or_default(
    s: &str,
    fallback: &str,
    cmd_tx: &std::sync::mpsc::Sender<crate::Cmd>,
) -> HotKey {
    match parse_hotkey(s) {
        Ok(hk) => hk,
        Err(e) => {
            eprintln!("Failed to parse hotkey '{}': {e}", s);
            let _ = cmd_tx.send(crate::Cmd::HotkeyFailed(s.to_string()));
            parse_hotkey(fallback).unwrap()
        }
    }
}

// register all three hotkeys from config
pub fn register_hotkeys(
    hotkeys_config: &crate::config::HotkeysConfig,
    cmd_tx: &std::sync::mpsc::Sender<crate::Cmd>,
) -> anyhow::Result<RegisteredHotkeys> {
    let manager = GlobalHotKeyManager::new()?;

    let icons_hk = parse_or_default(&hotkeys_config.icons, "ctrl+alt+h", cmd_tx);
    let taskbar_hk = parse_or_default(&hotkeys_config.taskbar, "ctrl+alt+t", cmd_tx);
    let windows_hk = parse_or_default(&hotkeys_config.windows, "ctrl+alt+w", cmd_tx);

    let icons_id = icons_hk.id();
    let taskbar_id = taskbar_hk.id();
    let windows_id = windows_hk.id();

    if let Err(e) = manager.register(icons_hk) {
        eprintln!("Failed to register icons hotkey: {e}");
        let _ = cmd_tx.send(crate::Cmd::HotkeyFailed(hotkeys_config.icons.clone()));
    }
    if let Err(e) = manager.register(taskbar_hk) {
        eprintln!("Failed to register taskbar hotkey: {e}");
        let _ = cmd_tx.send(crate::Cmd::HotkeyFailed(hotkeys_config.taskbar.clone()));
    }
    if let Err(e) = manager.register(windows_hk) {
        eprintln!("Failed to register windows hotkey: {e}");
        let _ = cmd_tx.send(crate::Cmd::HotkeyFailed(hotkeys_config.windows.clone()));
    }

    Ok(RegisteredHotkeys {
        manager,
        icons_id,
        taskbar_id,
        windows_id,
        icons_hk,
        taskbar_hk,
        windows_hk,
    })
}

// swap out hotkeys when the config changes
pub fn reregister_hotkeys(
    registered: &mut RegisteredHotkeys,
    hotkeys_config: &crate::config::HotkeysConfig,
    cmd_tx: &std::sync::mpsc::Sender<crate::Cmd>,
) {
    // unregister old ones (ignore errors, they might already be gone)
    let _ = registered.manager.unregister(registered.icons_hk);
    let _ = registered.manager.unregister(registered.taskbar_hk);
    let _ = registered.manager.unregister(registered.windows_hk);

    let icons_hk = parse_or_default(&hotkeys_config.icons, "ctrl+alt+h", cmd_tx);
    let taskbar_hk = parse_or_default(&hotkeys_config.taskbar, "ctrl+alt+t", cmd_tx);
    let windows_hk = parse_or_default(&hotkeys_config.windows, "ctrl+alt+w", cmd_tx);

    registered.icons_id = icons_hk.id();
    registered.taskbar_id = taskbar_hk.id();
    registered.windows_id = windows_hk.id();

    if let Err(e) = registered.manager.register(icons_hk) {
        eprintln!("Re-register icons hotkey failed: {e}");
        let _ = cmd_tx.send(crate::Cmd::HotkeyFailed(hotkeys_config.icons.clone()));
    }
    if let Err(e) = registered.manager.register(taskbar_hk) {
        eprintln!("Re-register taskbar hotkey failed: {e}");
        let _ = cmd_tx.send(crate::Cmd::HotkeyFailed(hotkeys_config.taskbar.clone()));
    }
    if let Err(e) = registered.manager.register(windows_hk) {
        eprintln!("Re-register windows hotkey failed: {e}");
        let _ = cmd_tx.send(crate::Cmd::HotkeyFailed(hotkeys_config.windows.clone()));
    }

    registered.icons_hk = icons_hk;
    registered.taskbar_hk = taskbar_hk;
    registered.windows_hk = windows_hk;
}

// check if a hotkey was pressed (non-blocking)
pub fn poll_hotkey_event() -> Option<GlobalHotKeyEvent> {
    GlobalHotKeyEvent::receiver().try_recv().ok()
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_basic_hotkey() {
        assert!(parse_hotkey("ctrl+alt+h").is_ok());
        assert!(parse_hotkey("win+shift+f1").is_ok());
    }

    #[test]
    fn rejects_missing_modifier() {
        assert!(parse_hotkey("h").is_err());
    }

    #[test]
    fn rejects_unknown_key() {
        assert!(parse_hotkey("ctrl+alt+nope").is_err());
    }

    #[test]
    fn rejects_unknown_modifier() {
        assert!(parse_hotkey("foo+h").is_err());
    }

    #[test]
    fn modifier_aliases_and_case_are_equivalent() {
        let a = parse_hotkey("ctrl+alt+h").unwrap();
        let b = parse_hotkey("CONTROL+Alt+H").unwrap();
        assert_eq!(a.id(), b.id());
    }

    #[test]
    fn modifier_only_is_rejected() {
        // "ctrl+alt" -> key part "alt" is not a valid key code
        assert!(parse_hotkey("ctrl+alt").is_err());
    }
}
