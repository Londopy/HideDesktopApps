# HideDesktopApps

A lightweight Windows system-tray app that hides and shows desktop icons — perfect for revealing your Wallpaper Engine wallpaper on demand.

---

## Features

- **Global hotkey** `Ctrl+Alt+H` (configurable) — toggle desktop icons from anywhere
- **Wallpaper double-click** — double-click on your desktop/wallpaper to toggle icons (works with Wallpaper Engine)
- **Tray double-click** — double-click the tray icon for the same action
- **Tray right-click menu** — toggle icons, toggle all open windows, change settings, or exit
- **Auto-start** — places a launcher in your Windows Startup folder on first run, appears as **HideDesktopApps** in Task Manager → Startup tab
- **Settings window** — change the hotkey from the tray menu without editing any files
- Tiny memory footprint — pure Python, no background scanning

---

## Requirements

- Windows 10 or 11
- Python 3.10 or newer
- Packages: `pystray`, `Pillow`, `pywin32`

---

## Installation

### Quick install (recommended)

Double-click `install_and_run.bat` — a colour menu will appear:

```
1  FULL INSTALL  ← does everything: installs packages, adds to startup, launches app
2  Install / update Python packages only
3  Run app now
4  Add to Windows startup
5  Remove from Windows startup
6  Check startup status
7  Open this folder in Explorer
0  Exit
```

Choose **1** for a first-time install.

### Manual install

```bash
pip install pystray pillow pywin32
pythonw hide_desktop.py
```

The app writes its startup entry automatically on first launch.

---

## Controls

| Action | Effect |
|---|---|
| `Ctrl+Alt+H` | Toggle desktop icons |
| Double-click wallpaper / desktop | Toggle desktop icons |
| Double-click tray icon | Toggle desktop icons |
| Right-click tray → **Toggle Desktop Icons** | Toggle desktop icons |
| Right-click tray → **Toggle All Windows** | Hide / restore all open app windows |
| Right-click tray → **Settings…** | Change the hotkey (app restarts automatically) |
| Right-click tray → **Exit** | Restore everything and quit |

---

## Tray icon guide

| Icon | Meaning |
|---|---|
| Four coloured tiles | Everything visible |
| Four grey tiles + red X | Desktop icons hidden |
| Single large grey tile + red X | All windows hidden |

---

## Changing the hotkey

Right-click the tray icon → **Settings…** — a small window lets you pick any modifier combination (Ctrl, Alt, Shift, Win) and letter key. Click **Apply & Save** and the app restarts automatically with the new hotkey.

You can also edit `config.ini` directly:

```ini
[hotkey]
modifiers = ctrl+alt
key = h
```

---

## Notes on Wallpaper Engine

The wallpaper double-click works by listening for two left-clicks within Windows' system double-click interval on a window that is a descendant of `Progman` or `WorkerW` — the same root windows that Wallpaper Engine uses for its render surface. No special Wallpaper Engine configuration is needed.

### Safety design

The mouse listener uses a `WH_MOUSE_LL` low-level hook. The callback does **only** pure arithmetic and a single non-blocking queue post before returning — no Win32 window calls happen inside it. A separate worker thread handles everything else. This means the hook will never delay or freeze mouse input regardless of system load.

---

## Startup management

On first launch the app drops a small VBS launcher into your Windows Startup folder. This shows up in **Task Manager → Startup tab** as **HideDesktopApps** and can be enabled/disabled there like any other startup entry.

To remove auto-start:

- Installer TUI → option **5 — Remove from Windows startup**
- **Task Manager → Startup tab** → right-click `HideDesktopApps` → Disable / Delete

---

## File layout

```
HideDesktopApps/
├── hide_desktop.py      — main app
├── install_and_run.bat  — setup TUI
├── config.ini           — hotkey config (auto-created on first run)
├── requirements.txt     — pip dependencies
└── README.md            — this file
```
