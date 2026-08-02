//! Discord Rich Presence over the local IPC named pipe.
//!
//! Discord ties the presence to the *lifetime of the IPC connection*: the moment
//! the pipe closes, the activity disappears from your profile. So we keep one
//! long-lived worker thread that owns the pipe, answers Discord's PINGs, and
//! reconnects on its own if Discord restarts (or wasn't running at launch).

use crate::dlog;
use std::fs::File;
use std::io::{Read, Write};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::OnceLock;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const DISCORD_APP_ID: &str = "1506489357534761111";
const REPO_URL: &str = "https://github.com/Londopy/HideDesktopApps";

// art asset key — upload an image with this name under
// Discord Developer Portal > your app > Rich Presence > Art Assets
const LARGE_IMAGE_KEY: &str = "logo";
const LARGE_IMAGE_TEXT: &str = "HideDesktopApps — clear your desktop with one hotkey";
const BUTTON_LABEL: &str = "Get HideDesktopApps";

// discord opcodes
const OP_HANDSHAKE: u32 = 0;
const OP_FRAME: u32 = 1;
const OP_CLOSE: u32 = 2;
const OP_PING: u32 = 3;
const OP_PONG: u32 = 4;

// windows: ReadFile on a PIPE_NOWAIT handle with nothing buffered
const ERROR_NO_DATA: i32 = 232;
// sanity cap so a garbage length can't make us allocate a gigabyte
const MAX_FRAME_BYTES: usize = 64 * 1024;

/// What the presence should currently show.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Presence {
    pub icons_hidden: bool,
    pub taskbar_hidden: bool,
    pub windows_hidden: bool,
    pub active_profile: Option<String>,
}

enum Msg {
    /// `None` clears the presence but keeps the connection alive.
    Set(Option<Presence>),
}

static TX: OnceLock<Sender<Msg>> = OnceLock::new();

/// Starts the presence worker. Safe to call once, early in `main`.
pub fn init() {
    if TX.get().is_some() {
        return;
    }
    let (tx, rx) = mpsc::channel();
    if TX.set(tx).is_ok() {
        std::thread::Builder::new()
            .name("discord-rpc".into())
            .spawn(move || worker(rx))
            .ok();
    }
}

/// Updates the presence. Clears it when nothing is hidden.
pub fn set_rich_presence(
    icons_hidden: bool,
    taskbar_hidden: bool,
    windows_hidden: bool,
    active_profile: Option<String>,
) {
    if !icons_hidden && !taskbar_hidden && !windows_hidden {
        clear_rich_presence();
        return;
    }
    send(Msg::Set(Some(Presence {
        icons_hidden,
        taskbar_hidden,
        windows_hidden,
        active_profile,
    })));
}

/// Clears the presence (nothing hidden, or the feature was switched off).
pub fn clear_rich_presence() {
    send(Msg::Set(None));
}

fn send(msg: Msg) {
    if let Some(tx) = TX.get() {
        let _ = tx.send(msg);
    }
}

// ---------------------------------------------------------------- presence text

// "desktop icons, the taskbar & app windows"
fn hidden_list(icons: bool, taskbar: bool, windows: bool) -> String {
    let mut parts = Vec::with_capacity(3);
    if icons {
        parts.push("desktop icons");
    }
    if taskbar {
        parts.push("the taskbar");
    }
    if windows {
        parts.push("app windows");
    }
    match parts.len() {
        0 => String::new(),
        1 => parts[0].to_string(),
        2 => format!("{} & {}", parts[0], parts[1]),
        _ => format!("{}, {} & {}", parts[0], parts[1], parts[2]),
    }
}

// discord rejects fields over 128 chars
fn clamp(s: String) -> String {
    if s.chars().count() <= 128 {
        return s;
    }
    let mut out: String = s.chars().take(127).collect();
    out.push('…');
    out
}

fn activity_json(p: &Presence, started_at: u64) -> serde_json::Value {
    let details = clamp(format!(
        "Hiding {}",
        hidden_list(p.icons_hidden, p.taskbar_hidden, p.windows_hidden)
    ));
    let state = clamp(match &p.active_profile {
        Some(name) => format!("Profile: {name}"),
        None => "Custom setup".to_string(),
    });

    serde_json::json!({
        "details": details,
        "state": state,
        "timestamps": { "start": started_at },
        "assets": {
            "large_image": LARGE_IMAGE_KEY,
            "large_text": LARGE_IMAGE_TEXT
        },
        "buttons": [
            { "label": BUTTON_LABEL, "url": REPO_URL }
        ]
    })
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

// ---------------------------------------------------------------- ipc plumbing

struct Conn {
    pipe: File,
    nonce: u64,
}

// discord can use any pipe 0-9, just try them all
fn open_pipe() -> Option<File> {
    for i in 0..10 {
        let path = format!(r"\\.\pipe\discord-ipc-{i}");
        if let Ok(f) = std::fs::OpenOptions::new().read(true).write(true).open(&path) {
            return Some(f);
        }
    }
    None
}

// non-blocking reads, so one unresponsive discord can't wedge the worker forever
fn set_nonblocking(pipe: &File) -> std::io::Result<()> {
    use std::os::windows::io::AsRawHandle;
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::System::Pipes::{SetNamedPipeHandleState, PIPE_NOWAIT};

    let handle = HANDLE(pipe.as_raw_handle() as _);
    unsafe { SetNamedPipeHandleState(handle, Some(&PIPE_NOWAIT), None, None) }
        .map_err(|e| std::io::Error::other(e.to_string()))
}

// discord ipc frame: opcode (4 bytes LE) + length (4 bytes LE) + json payload
fn write_frame(pipe: &mut File, opcode: u32, payload: &str) -> std::io::Result<()> {
    let data = payload.as_bytes();
    let mut msg = Vec::with_capacity(8 + data.len());
    msg.extend_from_slice(&opcode.to_le_bytes());
    msg.extend_from_slice(&(data.len() as u32).to_le_bytes());
    msg.extend_from_slice(data);
    pipe.write_all(&msg)?;
    pipe.flush()
}

/// Fills `buf` before `deadline`.
/// Returns `Ok(false)` only when `allow_empty` and not a single byte arrived.
fn fill(
    pipe: &mut File,
    buf: &mut [u8],
    deadline: Instant,
    allow_empty: bool,
) -> std::io::Result<bool> {
    let mut filled = 0usize;
    while filled < buf.len() {
        let idle = match pipe.read(&mut buf[filled..]) {
            // PIPE_NOWAIT reports "nothing buffered" as either of these
            Ok(0) => true,
            Ok(n) => {
                filled += n;
                false
            }
            Err(e) if e.raw_os_error() == Some(ERROR_NO_DATA) => true,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => false,
            Err(e) => return Err(e),
        };
        if idle {
            if Instant::now() >= deadline {
                if allow_empty && filled == 0 {
                    return Ok(false);
                }
                return Err(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "discord did not reply in time",
                ));
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    Ok(true)
}

fn read_frame(pipe: &mut File, timeout: Duration) -> std::io::Result<Option<(u32, String)>> {
    let mut header = [0u8; 8];
    if !fill(pipe, &mut header, Instant::now() + timeout, true)? {
        return Ok(None);
    }
    let opcode = u32::from_le_bytes(header[0..4].try_into().unwrap());
    let len = u32::from_le_bytes(header[4..8].try_into().unwrap()) as usize;
    if len > MAX_FRAME_BYTES {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "discord frame too large",
        ));
    }
    let mut payload = vec![0u8; len];
    // header already landed, so the body is on its way — give it a fixed grace period
    fill(pipe, &mut payload, Instant::now() + Duration::from_secs(2), false)?;
    Ok(Some((opcode, String::from_utf8_lossy(&payload).into_owned())))
}

impl Conn {
    fn connect() -> std::io::Result<Conn> {
        let pipe = open_pipe().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::NotFound, "discord ipc pipe not found")
        })?;
        set_nonblocking(&pipe)?;
        let mut conn = Conn { pipe, nonce: 0 };

        let handshake = serde_json::json!({ "v": 1, "client_id": DISCORD_APP_ID });
        write_frame(&mut conn.pipe, OP_HANDSHAKE, &handshake.to_string())?;

        // wait for READY, answering anything else discord throws at us meanwhile
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            match read_frame(&mut conn.pipe, Duration::from_millis(200))? {
                Some((OP_FRAME, _)) => return Ok(conn),
                Some((OP_CLOSE, body)) => {
                    return Err(std::io::Error::other(format!(
                        "discord closed the connection: {body}"
                    )))
                }
                Some((OP_PING, body)) => write_frame(&mut conn.pipe, OP_PONG, &body)?,
                Some(_) => {}
                None => {}
            }
            if Instant::now() >= deadline {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "discord never sent READY",
                ));
            }
        }
    }

    /// Drains anything Discord sent us. Errors mean the connection is dead.
    fn pump(&mut self) -> std::io::Result<()> {
        while let Some((opcode, body)) = read_frame(&mut self.pipe, Duration::ZERO)? {
            match opcode {
                OP_PING => write_frame(&mut self.pipe, OP_PONG, &body)?,
                OP_CLOSE => {
                    return Err(std::io::Error::other(format!(
                        "discord closed the connection: {body}"
                    )))
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn set_activity(&mut self, activity: serde_json::Value) -> std::io::Result<()> {
        self.nonce += 1;
        let pid = unsafe { windows::Win32::System::Threading::GetCurrentProcessId() };
        let payload = serde_json::json!({
            "cmd": "SET_ACTIVITY",
            "args": { "pid": pid, "activity": activity },
            "nonce": self.nonce.to_string()
        });
        write_frame(&mut self.pipe, OP_FRAME, &payload.to_string())
    }
}

// ---------------------------------------------------------------- worker

fn worker(rx: Receiver<Msg>) {
    let mut conn: Option<Conn> = None;
    let mut desired: Option<Presence> = None;
    // what discord is actually showing; None = unknown, resend on next tick
    let mut shown: Option<Option<Presence>> = None;
    let mut started_at: u64 = now_secs();
    let mut next_retry = Instant::now();
    let mut backoff = Duration::from_secs(2);

    loop {
        match rx.recv_timeout(Duration::from_secs(1)) {
            Ok(Msg::Set(next)) => {
                // restart the elapsed timer when a fresh hide session begins
                if desired.is_none() && next.is_some() {
                    started_at = now_secs();
                }
                desired = next;
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }

        // keep the socket healthy; a dead one drops us into the retry path below
        if let Some(c) = conn.as_mut() {
            if let Err(e) = c.pump() {
                dlog!("discord: connection lost ({e})");
                conn = None;
                shown = None;
                next_retry = Instant::now() + backoff;
            }
        }

        // only hold a connection while there's something to show
        if conn.is_none() && desired.is_some() && Instant::now() >= next_retry {
            match Conn::connect() {
                Ok(c) => {
                    dlog!("discord: connected");
                    conn = Some(c);
                    shown = None;
                    backoff = Duration::from_secs(2);
                }
                Err(e) => {
                    dlog!("discord: connect failed ({e})");
                    next_retry = Instant::now() + backoff;
                    // back off up to a minute so a closed discord costs us nothing
                    backoff = (backoff * 2).min(Duration::from_secs(60));
                }
            }
        }

        // push the current state if discord isn't already showing it
        let needs_update = shown.as_ref() != Some(&desired);
        if needs_update {
            if let Some(c) = conn.as_mut() {
                let activity = match &desired {
                    Some(p) => activity_json(p, started_at),
                    None => serde_json::Value::Null,
                };
                match c.set_activity(activity) {
                    Ok(()) => shown = Some(desired.clone()),
                    Err(e) => {
                        dlog!("discord: set_activity failed ({e})");
                        conn = None;
                        shown = None;
                        next_retry = Instant::now() + backoff;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hidden_list_reads_like_english() {
        assert_eq!(hidden_list(true, false, false), "desktop icons");
        assert_eq!(hidden_list(true, true, false), "desktop icons & the taskbar");
        assert_eq!(
            hidden_list(true, true, true),
            "desktop icons, the taskbar & app windows"
        );
    }

    #[test]
    fn activity_has_details_state_and_button() {
        let p = Presence {
            icons_hidden: true,
            taskbar_hidden: true,
            windows_hidden: false,
            active_profile: Some("Focus".into()),
        };
        let a = activity_json(&p, 1_700_000_000);
        assert_eq!(a["details"], "Hiding desktop icons & the taskbar");
        assert_eq!(a["state"], "Profile: Focus");
        assert_eq!(a["buttons"][0]["url"], REPO_URL);
        assert_eq!(a["timestamps"]["start"], 1_700_000_000u64);
    }

    #[test]
    fn no_profile_falls_back_to_custom() {
        let p = Presence {
            icons_hidden: false,
            taskbar_hidden: false,
            windows_hidden: true,
            active_profile: None,
        };
        assert_eq!(activity_json(&p, 0)["state"], "Custom setup");
    }

    #[test]
    fn long_profile_names_are_clamped() {
        let p = Presence {
            icons_hidden: true,
            taskbar_hidden: false,
            windows_hidden: false,
            active_profile: Some("x".repeat(200)),
        };
        let state = activity_json(&p, 0)["state"].as_str().unwrap().to_string();
        assert!(state.chars().count() <= 128);
    }
}
