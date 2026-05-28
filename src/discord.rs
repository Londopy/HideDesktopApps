use std::fs::File;
use std::io::{Read, Write};

const DISCORD_APP_ID: &str = "1506489357534761111";

// discord can use any pipe 0-9, just try them all
fn open_pipe() -> Option<File> {
    for i in 0..10 {
        let path = format!(r"\\.\pipe\discord-ipc-{}", i);
        if let Ok(f) = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
        {
            return Some(f);
        }
    }
    None
}

// discord ipc format: opcode (4 bytes) + length (4 bytes) + json payload
fn send_message(pipe: &mut File, opcode: u32, payload: &str) -> std::io::Result<()> {
    let data = payload.as_bytes();
    let mut msg = Vec::with_capacity(8 + data.len());
    msg.extend_from_slice(&opcode.to_le_bytes());
    msg.extend_from_slice(&(data.len() as u32).to_le_bytes());
    msg.extend_from_slice(data);
    pipe.write_all(&msg)
}

fn read_message(pipe: &mut File) -> std::io::Result<(u32, String)> {
    let mut header = [0u8; 8];
    pipe.read_exact(&mut header)?;
    let opcode = u32::from_le_bytes(header[0..4].try_into().unwrap());
    let len = u32::from_le_bytes(header[4..8].try_into().unwrap()) as usize;
    let mut payload = vec![0u8; len];
    pipe.read_exact(&mut payload)?;
    Ok((opcode, String::from_utf8_lossy(&payload).to_string()))
}

// updates discord rich presence based on what's currently hidden
pub fn set_rich_presence(
    icons_hidden: bool,
    taskbar_hidden: bool,
    windows_hidden: bool,
    active_profile: Option<String>,
) {
    // if nothing is hidden, clear the presence instead of showing stale info
    if !icons_hidden && !taskbar_hidden && !windows_hidden {
        clear_rich_presence();
        return;
    }

    std::thread::spawn(move || {
        if let Err(e) =
            set_presence_inner(icons_hidden, taskbar_hidden, windows_hidden, active_profile)
        {
            eprintln!("Discord rich presence error: {e}");
        }
    });
}

fn set_presence_inner(
    icons_hidden: bool,
    taskbar_hidden: bool,
    windows_hidden: bool,
    active_profile: Option<String>,
) -> anyhow::Result<()> {
    let mut pipe = open_pipe().ok_or_else(|| anyhow::anyhow!("Discord IPC pipe not found"))?;

    // 0 = handshake
    let handshake = serde_json::json!({ "v": 1, "client_id": DISCORD_APP_ID });
    send_message(&mut pipe, 0, &handshake.to_string())?;
    let _ = read_message(&mut pipe)?;

    // show profile name if one is active, otherwise "Custom"
    let state_str = match &active_profile {
        Some(name) => name.clone(),
        None => "Custom".to_string(),
    };

    // show which things are hidden
    let mut parts = Vec::new();
    if icons_hidden {
        parts.push("Icons");
    }
    if taskbar_hidden {
        parts.push("Taskbar");
    }
    if windows_hidden {
        parts.push("Windows");
    }
    let details = format!("{} hidden", parts.join(" · "));

    let pid = unsafe { windows::Win32::System::Threading::GetCurrentProcessId() };

    // 1 = SET_ACTIVITY
    let activity = serde_json::json!({
        "cmd": "SET_ACTIVITY",
        "args": {
            "pid": pid,
            "activity": {
                "state": state_str,
                "details": details
            }
        },
        "nonce": "1"
    });
    send_message(&mut pipe, 1, &activity.to_string())?;
    let _ = read_message(&mut pipe)?;

    Ok(())
}

// clears discord presence (called when nothing is hidden)
pub fn clear_rich_presence() {
    std::thread::spawn(|| {
        if let Err(e) = clear_presence_inner() {
            eprintln!("Discord clear presence error: {e}");
        }
    });
}

fn clear_presence_inner() -> anyhow::Result<()> {
    let mut pipe = open_pipe().ok_or_else(|| anyhow::anyhow!("Discord IPC pipe not found"))?;

    let handshake = serde_json::json!({ "v": 1, "client_id": DISCORD_APP_ID });
    send_message(&mut pipe, 0, &handshake.to_string())?;
    let _ = read_message(&mut pipe)?;

    let pid = unsafe { windows::Win32::System::Threading::GetCurrentProcessId() };

    let clear = serde_json::json!({
        "cmd": "SET_ACTIVITY",
        "args": { "pid": pid, "activity": null },
        "nonce": "2"
    });
    send_message(&mut pipe, 1, &clear.to_string())?;
    let _ = read_message(&mut pipe)?;

    Ok(())
}
