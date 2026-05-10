# HideDesktopApps
A lightweight Windows system-tray app that hides and shows desktop icons, the taskbar, and all windows — perfect for revealing your Wallpaper Engine wallpaper on demand.

[![PyPI](https://img.shields.io/pypi/v/hide-desktop-apps)](https://pypi.org/project/hide-desktop-apps/)
[![Python](https://img.shields.io/pypi/pyversions/hide-desktop-apps)](https://pypi.org/project/hide-desktop-apps/)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://github.com/Londopy/HideDesktopApps/blob/main/LICENSE)

---

## Install from PyPI

```bash
pip install hide-desktop-apps
hide-desktop-apps
```

Config and the tray icon are stored in `%APPDATA%\HideDesktopApps\` and created automatically on first run.

---

## Features
- **Three configurable hotkeys** — `Ctrl+Alt+H` toggles icons, `Ctrl+Alt+T` toggles the taskbar, `Ctrl+Alt+W` toggles all windows
- **Taskbar toggle** — hide/show the taskbar from the tray or via hotkey (supports multi-monitor)
- **Tray double-click** — double-click the tray icon to toggle icons
- **Tray right-click menu** — toggle icons, taskbar, or all windows; open settings; restart; exit
- **Auto-start** — places a launcher in your Windows Startup folder, appears as **HideDesktopApps** in Task Manager → Startup tab
- **Settings window** — change all three hotkeys, startup delay, run-at-startup toggle, and default state — all without editing files
- Tiny memory footprint — pure Python, no background scanning

---

## Requirements
- Windows 10 or 11
- Python 3.7 or newer
- Packages: `pystray`, `Pillow`, `pywin32`

---

## Installation

### Via pip (recommended)
```bash
pip install hide-desktop-apps
hide-desktop-apps
```

### From source (clone this repo)
Double-click `install_and_run.bat` — a menu will appear:
```
1  FULL INSTALL  ← does everything: installs packages, adds to startup, launches app
2  Install / update Python packages only
3  Run app now
4  Add to Windows startup
5  Remove from Windows startup
6  Check startup status
7  Open this folder in Explorer
8  Run in debug mode  (shows errors in console)
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
| `Ctrl+Alt+T` | Toggle taskbar |
| `Ctrl+Alt+W` | Toggle all open windows |
| Double-click tray icon | Toggle desktop icons |
| Right-click tray → **Toggle Desktop Icons** | Toggle desktop icons |
| Right-click tray → **Toggle Taskbar** | Hide / restore the taskbar |
| Right-click tray → **Toggle All Windows** | Hide / restore all open app windows |
| Right-click tray → **Settings…** | Open the settings window |
| Right-click tray → **Restart** | Restart the app (picks up config changes) |
| Right-click tray → **Exit** | Restore everything and quit |

---

## Tray icon guide
| Icon | Meaning |
|---|---|
| Four coloured tiles | Everything visible |
| Four coloured tiles + orange dot | Taskbar hidden |
| Four grey tiles + red X | Desktop icons hidden |
| Single large grey tile + red X | All windows hidden |

---

## Settings window
Right-click the tray icon → **Settings…** to open a tabbed settings window:

**Hotkeys tab** — set the modifier keys and letter for all three hotkeys (icons, taskbar, windows). Conflicts are detected automatically. The app restarts to apply hotkey changes.

**Startup tab** — toggle whether the app runs at Windows startup, and configure the delay (0–120 seconds) before it launches after login.

**Defaults tab** — choose whether desktop icons start hidden when the app launches.

You can also edit `config.ini` directly:
```ini
[hotkey]
modifiers = ctrl+alt
key = h

[hotkey_taskbar]
modifiers = ctrl+alt
key = t

[hotkey_windows]
modifiers = ctrl+alt
key = w

[startup]
run_at_startup = true
delay = 30

[defaults]
icons_hidden = false
```

---

## Startup management
On first launch the app drops a small VBS launcher into your Windows Startup folder. This shows up in **Task Manager → Startup tab** as **HideDesktopApps** and can be enabled/disabled there like any other startup entry.

To remove auto-start:
- Settings window → Startup tab → uncheck "Run HideDesktopApps when Windows starts"
- Installer TUI → option **5 — Remove from Windows startup**
- **Task Manager → Startup tab** → right-click `HideDesktopApps` → Disable / Delete

---

## Recovery tips

### Taskbar is hidden and you can't find the tray icon
If you hid the taskbar and the tray icon is out of reach, the quickest way to get everything back is to open a **Run dialog** (`Win+R`) or any command prompt and paste:

```python
python -c "import win32gui, win32con; win32gui.ShowWindow(win32gui.FindWindow('Shell_TrayWnd', None), win32con.SW_SHOW)"
```

This is a totally normal thing to need — `Ctrl+Alt+T` is easy to hit by accident, and the taskbar disappearing without an obvious way back is just part of learning the app. Once you're back up, consider binding the taskbar hotkey to something you're less likely to hit unintentionally, or leaving the taskbar toggle to the tray menu only.

### All windows are hidden
Press `Ctrl+Alt+W` again to restore them, or right-click the tray icon → **Toggle All Windows**.

### App isn't appearing in the tray
Run option **8 — Debug mode** in `install_and_run.bat`. This runs the app in a visible console window so any errors are shown instead of swallowed silently.

---

## File layout
```
HideDesktopApps/
├── hide_desktop.py      — main app
├── install_and_run.bat  — setup TUI
├── config.ini           — configuration (auto-created on first run)
├── requirements.txt     — pip dependencies
├── CHANGELOG.md         — version history
└── README.md            — this file
```
