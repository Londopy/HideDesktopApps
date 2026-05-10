# Changelog

All notable changes to HideDesktopApps are documented here.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and [patchnotes](https://pypi.org/project/patchnotes/).

---

## [0.3.0] — 2026-05-09

### Added
- **`atexit` cleanup handler** — all hidden state (taskbar, windows, desktop icons) is now automatically restored if the app exits via an unhandled exception or `Ctrl+C`. Hard-kills (Task Manager → End Process) are still unrecoverable; documented in README.
- **Caveats & Known Issues section** in README — covers ghost-window risk and antivirus false positives when packaging with PyInstaller.
- **Who is this for? section** in README — use cases for streamers, kiosk setups, focus tools, and desktop customisation.
- **`_restore_all()` helper** — single function that handles all three restore operations; `_exit()` and `_request_restart()` now delegate to it instead of duplicating the logic.

---

## [0.2.0] — 2026-05-09

### Added
- **Taskbar toggle** — hide and restore the taskbar from the tray menu or via the new `Ctrl+Alt+T` hotkey. Supports multi-monitor setups (primary + secondary taskbars).
- **Third configurable hotkey** — `[hotkey_taskbar]` section added to `config.ini`. Default: `Ctrl+Alt+T`.
- **Toggle All Windows hotkey** — `Ctrl+Alt+W` now has a dedicated, fully configurable hotkey (`[hotkey_windows]` config section).
- **Tabbed Settings window** — Settings are now organised across three tabs: Hotkeys, Startup, and Defaults.
- **Hotkeys tab** — configure modifier keys and letter for all three hotkeys with live conflict detection. All three hotkeys must be unique; the app restarts automatically when changes are applied.
- **Startup tab** — toggle run-at-startup and configure a startup delay (0–120 seconds) without editing files.
- **Defaults tab** — choose whether desktop icons start hidden on each launch.
- **Configurable startup delay** — `delay` key in `[startup]` section (seconds). The VBS launcher uses `WScript.Sleep` so the delay happens before the app loads, not after.
- **Run-at-startup toggle** — `run_at_startup` key in `[startup]` section. The app adds or removes its VBS launcher automatically when this changes.
- **Tray icon orange dot** — a small orange dot appears on the tray icon when the taskbar is hidden, so you always know the current state at a glance.
- **Tray tooltip updated** — tooltip now shows all three active hotkeys.
- **Debug run option** — `install_and_run.bat` option 8 launches the app using `python.exe` instead of `pythonw.exe` so errors print to the console rather than disappearing silently.
- **`from __future__ import annotations`** — defers annotation evaluation so the app runs correctly on Python 3.7+, not just 3.10+.

### Changed
- **Minimum Python version lowered** to 3.7 (was effectively 3.10 due to `X | Y` union annotation syntax).
- **`install_and_run.bat` header** updated to show both active hotkeys and remove the outdated "wallpaper double-click" trigger description.
- **"All done" message** in the batch installer updated to mention all hotkeys.
- **`config.ini`** gains two new sections: `[hotkey_taskbar]` and `[hotkey_windows]`. Existing installs without these sections fall back to the built-in defaults automatically.

### Fixed
- App failed to appear in the system tray on Python < 3.10 due to a `TypeError` at import time caused by `pystray.Icon | None` annotation syntax. Fixed with `from __future__ import annotations`.
- Entry-point block in `hide_desktop.py` was corrupted with duplicate code appended from a previous write operation, causing a `SyntaxError` on line 863. File has been cleaned and verified.

---

## [0.1.0] — 2025-04-01

### Added
- Initial release.
- System tray icon with four coloured tiles; icon reflects current state (hidden icons → grey tiles + red X, hidden windows → single grey tile + red X).
- **Toggle Desktop Icons** via `Ctrl+Alt+H` hotkey (configurable) and tray double-click.
- **Toggle All Windows** via tray menu.
- Global hotkey listener using `RegisterHotKey` / `UnregisterHotKey` via ctypes.
- Settings window (single tab) — configure hotkey modifier and key via GUI.
- `config.ini` auto-created on first run with sensible defaults.
- VBS launcher written to Windows Startup folder on first launch; appears as **HideDesktopApps** in Task Manager → Startup tab.
- `install_and_run.bat` TUI — full install, package install, run, add/remove/check startup, open folder.
- In-process restart loop — `Restart` menu item re-initialises without spawning a new process.
- Exit restores all hidden state before quitting.
- `.ico` file auto-generated from tray icon on first run.
