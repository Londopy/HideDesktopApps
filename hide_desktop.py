"""
hide_desktop.py — Lightweight system tray app: hide/show desktop icons & windows.

Controls
--------
  Configurable hotkey (default Ctrl+Alt+H)  -> toggle desktop icons
  Tray double-click                         -> toggle desktop icons
  Right-click tray icon                     -> full menu

Menu
----
  Toggle Desktop Icons  (default / double-click)
  Toggle All Windows
  Settings...
  Restart
  ------------------
  Exit (restore everything)

Startup
-------
  Creates HideDesktopApps.lnk in the Windows Startup folder so Task Manager
  shows the app name and icon instead of "Python".

Dependencies
------------
  pip install pystray pillow pywin32
  (tkinter ships with standard Python on Windows)
"""

import configparser
import ctypes
import ctypes.wintypes
import os
import subprocess
import sys
import threading
import tkinter as tk
import winreg
from tkinter import messagebox, ttk

import pystray
import win32con
import win32gui
import win32process
from PIL import Image, ImageDraw

# ---------------------------------------------------------------------------
# Paths
# ---------------------------------------------------------------------------

_SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
CONFIG_FILE  = os.path.join(_SCRIPT_DIR, "config.ini")
ICON_FILE    = os.path.join(_SCRIPT_DIR, "hide_desktop.ico")

# ---------------------------------------------------------------------------
# Config
# ---------------------------------------------------------------------------

_DEFAULT_CONFIG = {"hotkey": {"modifiers": "ctrl+alt", "key": "h"}}


def _load_config() -> configparser.ConfigParser:
    cfg = configparser.ConfigParser()
    cfg.read_dict(_DEFAULT_CONFIG)
    if os.path.exists(CONFIG_FILE):
        cfg.read(CONFIG_FILE, encoding="utf-8")
    return cfg


def _save_config(cfg: configparser.ConfigParser) -> None:
    with open(CONFIG_FILE, "w", encoding="utf-8") as fh:
        fh.write("# HideDesktopApps configuration\n")
        fh.write("# Modifiers: ctrl, alt, shift, win  (combine with +)\n")
        fh.write("# Key: any single letter A-Z\n\n")
        cfg.write(fh)


def _ensure_config() -> None:
    if not os.path.exists(CONFIG_FILE):
        _save_config(_load_config())


# ---------------------------------------------------------------------------
# Global state
# ---------------------------------------------------------------------------

APP_NAME = "HideDesktopApps"

_lock              = threading.Lock()
_icons_hidden      : bool = False
_windows_hidden    : bool = False
_hidden_windows    : list[tuple[int, tuple]] = []
_icon              : pystray.Icon | None = None
_restart_requested : bool = False        # set by _request_restart()

_SHELL_CLASSES = {
    "Shell_TrayWnd", "Progman", "WorkerW", "Button",
    "DV2ControlHost", "Windows.UI.Core.CoreWindow",
    "XamlExplorerHostIslandWindow",
}


# ---------------------------------------------------------------------------
# Desktop-icon toggle
# ---------------------------------------------------------------------------

def _find_desktop_listview() -> int | None:
    progman = win32gui.FindWindow("Progman", None)
    defview = win32gui.FindWindowEx(progman, None, "SHELLDLL_DefView", None)
    if defview:
        lv = win32gui.FindWindowEx(defview, None, "SysListView32", None)
        if lv:
            return lv
    result: list[int] = []

    def _cb(hwnd: int, _) -> None:
        if result:
            return
        dv = win32gui.FindWindowEx(hwnd, None, "SHELLDLL_DefView", None)
        if dv:
            lv = win32gui.FindWindowEx(dv, None, "SysListView32", None)
            if lv:
                result.append(lv)

    win32gui.EnumWindows(_cb, None)
    return result[0] if result else None


def _hide_desktop_icons() -> None:
    global _icons_hidden
    lv = _find_desktop_listview()
    if lv:
        win32gui.ShowWindow(lv, win32con.SW_HIDE)
    _icons_hidden = True
    _refresh_icon()


def _show_desktop_icons() -> None:
    global _icons_hidden
    lv = _find_desktop_listview()
    if lv:
        win32gui.ShowWindow(lv, win32con.SW_SHOW)
    _icons_hidden = False
    _refresh_icon()


def _toggle_icons(_icon_arg=None, _item=None) -> None:
    with _lock:
        if _icons_hidden:
            _show_desktop_icons()
        else:
            _hide_desktop_icons()


# ---------------------------------------------------------------------------
# All-windows toggle
# ---------------------------------------------------------------------------

def _should_include_window(hwnd: int) -> bool:
    if not win32gui.IsWindowVisible(hwnd):
        return False
    if not win32gui.GetWindowText(hwnd):
        return False
    if win32gui.GetClassName(hwnd) in _SHELL_CLASSES:
        return False
    ex_style = win32gui.GetWindowLong(hwnd, win32con.GWL_EXSTYLE)
    if ex_style & win32con.WS_EX_TOOLWINDOW:
        return False
    try:
        _, pid = win32process.GetWindowThreadProcessId(hwnd)
        if pid == os.getpid():
            return False
    except Exception:
        return False
    return True


def _hide_all_windows() -> None:
    global _windows_hidden, _hidden_windows
    candidates: list[int] = []
    win32gui.EnumWindows(
        lambda h, _: candidates.append(h) if _should_include_window(h) else None,
        None,
    )
    _hidden_windows = []
    for hwnd in candidates:
        try:
            placement = win32gui.GetWindowPlacement(hwnd)
            win32gui.ShowWindow(hwnd, win32con.SW_HIDE)
            _hidden_windows.append((hwnd, placement))
        except Exception:
            pass
    _windows_hidden = True
    _refresh_icon()


def _show_all_windows() -> None:
    global _windows_hidden, _hidden_windows
    for hwnd, placement in _hidden_windows:
        try:
            if not win32gui.IsWindow(hwnd):
                continue
            win32gui.ShowWindow(hwnd, win32con.SW_SHOW)
            show_cmd = placement[1]
            if show_cmd == win32con.SW_SHOWMAXIMIZED:
                win32gui.ShowWindow(hwnd, win32con.SW_SHOWMAXIMIZED)
            elif show_cmd == win32con.SW_SHOWMINIMIZED:
                win32gui.ShowWindow(hwnd, win32con.SW_SHOWMINIMIZED)
        except Exception:
            pass
    _hidden_windows = []
    _windows_hidden = False
    _refresh_icon()


def _toggle_windows(_icon_arg=None, _item=None) -> None:
    with _lock:
        if _windows_hidden:
            _show_all_windows()
        else:
            _hide_all_windows()


# ---------------------------------------------------------------------------
# Tray icon drawing
# ---------------------------------------------------------------------------

def _draw_icon(icons_hidden: bool, windows_hidden: bool) -> Image.Image:
    size = 64
    img  = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    d    = ImageDraw.Draw(img)

    if windows_hidden:
        d.rectangle((4, 4, 60, 60), fill="#7f8c8d", outline="#bdc3c7", width=2)
        d.line([(8, 8), (56, 56)], fill="#e74c3c", width=7)
        d.line([(56, 8), (8, 56)], fill="#e74c3c", width=7)
    else:
        colours = (
            ((4,  4,  30, 30), "#3498db" if not icons_hidden else "#7f8c8d"),
            ((34, 4,  60, 30), "#2ecc71" if not icons_hidden else "#7f8c8d"),
            ((4,  34, 30, 60), "#e74c3c" if not icons_hidden else "#7f8c8d"),
            ((34, 34, 60, 60), "#f39c12" if not icons_hidden else "#7f8c8d"),
        )
        outline = "white" if not icons_hidden else "#bdc3c7"
        for box, colour in colours:
            d.rectangle(box, fill=colour, outline=outline, width=2)
        if icons_hidden:
            d.line([(8, 8), (56, 56)], fill="#e74c3c", width=5)
            d.line([(56, 8), (8, 56)], fill="#e74c3c", width=5)

    return img


def _refresh_icon() -> None:
    if _icon is None:
        return
    _icon.icon = _draw_icon(_icons_hidden, _windows_hidden)
    parts: list[str] = []
    if _icons_hidden:
        parts.append("icons hidden")
    if _windows_hidden:
        parts.append("windows hidden")
    _icon.title = f"{APP_NAME} — {' · '.join(parts) or 'everything visible'}"


# ---------------------------------------------------------------------------
# .ico file  (embedded in the startup shortcut)
# ---------------------------------------------------------------------------

def _create_icon_file() -> str:
    if os.path.exists(ICON_FILE):
        return ICON_FILE
    base = _draw_icon(False, False)
    try:
        resample = Image.Resampling.LANCZOS
    except AttributeError:
        resample = Image.LANCZOS
    sizes  = [16, 32, 48]
    frames = [base.resize((s, s), resample) for s in sizes]
    frames[0].save(ICON_FILE, format="ICO",
                   append_images=frames[1:], sizes=[(s, s) for s in sizes])
    return ICON_FILE


# ---------------------------------------------------------------------------
# Restart
# ---------------------------------------------------------------------------

def _request_restart(_icon_arg=None, _item=None) -> None:
    """
    Set the restart flag and stop the tray icon.
    The main() loop detects the flag and re-calls main(), picking up any
    config changes (new hotkey, etc.) without spawning a second process.
    """
    global _restart_requested
    with _lock:
        if _windows_hidden:
            _show_all_windows()
        if _icons_hidden:
            _show_desktop_icons()
    _restart_requested = True
    if _icon is not None:
        _icon.stop()


# ---------------------------------------------------------------------------
# Settings window
# ---------------------------------------------------------------------------

def _settings_window() -> None:
    """Hotkey settings dialog. Runs in its own daemon thread."""
    cfg      = _load_config()
    mods_raw = cfg["hotkey"].get("modifiers", "ctrl+alt").lower()
    key_raw  = cfg["hotkey"].get("key", "h").upper()

    root = tk.Tk()
    root.title("HideDesktopApps — Settings")
    root.resizable(False, False)
    root.attributes("-topmost", True)

    ctrl_var  = tk.BooleanVar(value="ctrl"  in mods_raw)
    alt_var   = tk.BooleanVar(value="alt"   in mods_raw)
    shift_var = tk.BooleanVar(value="shift" in mods_raw)
    win_var   = tk.BooleanVar(value="win"   in mods_raw)
    key_var   = tk.StringVar(value=key_raw)
    preview   = tk.StringVar()

    def _refresh(*_):
        parts = (
            (["Ctrl"]  if ctrl_var.get()  else []) +
            (["Alt"]   if alt_var.get()   else []) +
            (["Shift"] if shift_var.get() else []) +
            (["Win"]   if win_var.get()   else [])
        )
        k = key_var.get().strip().upper()[:1] or "?"
        preview.set("+".join(parts + [k]))

    for v in (ctrl_var, alt_var, shift_var, win_var, key_var):
        v.trace_add("write", _refresh)

    pad = dict(padx=16)
    ttk.Label(root, text="Hotkey  —  toggle desktop icons",
              font=("Segoe UI", 10, "bold")).pack(**pad, pady=(14, 6), anchor="w")
    ttk.Separator(root, orient="horizontal").pack(fill="x", padx=16)

    mf = ttk.Frame(root)
    mf.pack(**pad, pady=10, anchor="w")
    ttk.Label(mf, text="Modifiers:").grid(row=0, column=0, sticky="w")
    for col, (label, var) in enumerate(
        [("Ctrl", ctrl_var), ("Alt", alt_var), ("Shift", shift_var), ("Win", win_var)], 1
    ):
        ttk.Checkbutton(mf, text=label, variable=var).grid(
            row=0, column=col, padx=7, sticky="w")

    kf = ttk.Frame(root)
    kf.pack(**pad, anchor="w")
    ttk.Label(kf, text="Key (A-Z):").grid(row=0, column=0, sticky="w")
    ttk.Entry(kf, textvariable=key_var, width=4,
              justify="center", font=("Consolas", 12, "bold")).grid(
        row=0, column=1, padx=10)

    ttk.Label(root, text="Preview:").pack(**pad, pady=(10, 0), anchor="w")
    ttk.Label(root, textvariable=preview,
              font=("Consolas", 14, "bold"), foreground="#0078d4").pack(
        **pad, pady=(0, 2), anchor="w")
    ttk.Label(root, text="App will restart automatically to apply the new hotkey.",
              foreground="gray").pack(**pad, pady=(2, 8), anchor="w")
    ttk.Separator(root, orient="horizontal").pack(fill="x", padx=16, pady=(0, 10))

    bf = ttk.Frame(root)
    bf.pack(**pad, pady=(0, 14))

    def _apply() -> None:
        mods = (
            (["ctrl"]  if ctrl_var.get()  else []) +
            (["alt"]   if alt_var.get()   else []) +
            (["shift"] if shift_var.get() else []) +
            (["win"]   if win_var.get()   else [])
        )
        k = key_var.get().strip().upper()[:1]
        if not mods:
            messagebox.showerror("No modifier", "Select at least one modifier.", parent=root)
            return
        if not k or not k.isalpha():
            messagebox.showerror("Invalid key", "Enter a single letter A-Z.", parent=root)
            return
        cfg["hotkey"]["modifiers"] = "+".join(mods)
        cfg["hotkey"]["key"]       = k.lower()
        _save_config(cfg)
        root.destroy()
        _request_restart()   # ← restarts automatically, no manual step needed

    ttk.Button(bf, text="Apply & Save", command=_apply).pack(side="left")
    ttk.Button(bf, text="Cancel", command=root.destroy).pack(side="left", padx=10)

    _refresh()
    root.mainloop()


def _open_settings(_icon_arg=None, _item=None) -> None:
    threading.Thread(target=_settings_window, daemon=True).start()


# ---------------------------------------------------------------------------
# Global hotkey
# ---------------------------------------------------------------------------

_MOD_MAP = {"ctrl": 0x0002, "alt": 0x0001, "shift": 0x0004, "win": 0x0008}
_MOD_NOREPEAT = 0x4000
_HOTKEY_ID    = 1
_WM_HOTKEY    = 0x0312


def _parse_hotkey(cfg: configparser.ConfigParser) -> tuple[int, int]:
    mods_raw = cfg["hotkey"].get("modifiers", "ctrl+alt").lower()
    key_raw  = cfg["hotkey"].get("key", "h").upper()[:1]
    mods = _MOD_NOREPEAT
    for name, flag in _MOD_MAP.items():
        if name in mods_raw:
            mods |= flag
    return mods, (ord(key_raw) if key_raw.isalpha() else ord("H"))


def _hotkey_label(cfg: configparser.ConfigParser) -> str:
    mods_raw = cfg["hotkey"].get("modifiers", "ctrl+alt").lower()
    key_raw  = cfg["hotkey"].get("key", "h").upper()[:1]
    return "+".join([n.capitalize() for n in _MOD_MAP if n in mods_raw] + [key_raw])


def _run_hotkey_listener() -> None:
    cfg      = _load_config()
    mods, vk = _parse_hotkey(cfg)
    label    = _hotkey_label(cfg)

    if not ctypes.windll.user32.RegisterHotKey(None, _HOTKEY_ID, mods, vk):
        print(f"[warn] Could not register {label} — open Settings to choose a different key.")
        return

    msg = ctypes.wintypes.MSG()
    while ctypes.windll.user32.GetMessageW(ctypes.byref(msg), None, 0, 0) > 0:
        if msg.message == _WM_HOTKEY and msg.wParam == _HOTKEY_ID:
            _toggle_icons()
        ctypes.windll.user32.TranslateMessage(ctypes.byref(msg))
        ctypes.windll.user32.DispatchMessageW(ctypes.byref(msg))

    ctypes.windll.user32.UnregisterHotKey(None, _HOTKEY_ID)


# ---------------------------------------------------------------------------
# Startup — VBS launcher in the Windows Startup folder
# ---------------------------------------------------------------------------
#
# A tiny .vbs file placed in the user's Startup folder is the most reliable
# way to appear as a named entry in Task Manager → Startup tab.
# Windows lists startup-folder items by their filename, so
# "HideDesktopApps.vbs" → shows as "HideDesktopApps" in Task Manager.
#
# The VBS uses WScript.Shell.Run with WindowStyle=0 so there is no console
# flash, and bWaitOnReturn=False so the login sequence is not held up.
# ---------------------------------------------------------------------------

_REG_RUN    = r"Software\Microsoft\Windows\CurrentVersion\Run"
_STARTUP_DIR = os.path.join(
    os.environ.get("APPDATA", ""),
    "Microsoft", "Windows", "Start Menu", "Programs", "Startup",
)


def _startup_target() -> tuple[str, str]:
    pythonw = os.path.join(os.path.dirname(sys.executable), "pythonw.exe")
    if not os.path.isfile(pythonw):
        pythonw = sys.executable
    return pythonw, os.path.abspath(__file__)


def _get_vbs_path() -> str:
    return os.path.join(_STARTUP_DIR, f"{APP_NAME}.vbs")


def _write_vbs(vbs_path: str, pythonw: str, script: str) -> None:
    """
    Write a one-liner VBS launcher.
    Chr(34) is a double-quote — avoids any escaping headaches inside VBS strings.
    WindowStyle 0 = hidden window; False = don't wait for it to finish.
    """
    q = chr(34)   # double-quote character
    content = (
        f"' {APP_NAME} — auto-generated launcher, do not edit manually\r\n"
        f'CreateObject("WScript.Shell").Run '
        f'{q}{pythonw}{q} & " " & {q}{script}{q}, 0, False\r\n'
    )
    with open(vbs_path, "w", encoding="utf-8") as fh:
        fh.write(content)


def _scrub_old_startup_files() -> None:
    """Remove any leftovers from previous methods (.lnk shortcut, registry entry)."""
    # Old .lnk shortcut
    lnk = os.path.join(_STARTUP_DIR, f"{APP_NAME}.lnk")
    if os.path.exists(lnk):
        try:
            os.remove(lnk)
            print(f"[startup] Removed old .lnk: {lnk}")
        except OSError:
            pass

    # Old registry Run entries whose value contains hide_desktop.py
    script_lower = os.path.abspath(__file__).lower()
    try:
        key = winreg.OpenKey(
            winreg.HKEY_CURRENT_USER, _REG_RUN, 0,
            winreg.KEY_READ | winreg.KEY_SET_VALUE,
        )
        to_delete: list[str] = []
        i = 0
        while True:
            try:
                name, value, _ = winreg.EnumValue(key, i)
                if script_lower in value.lower():
                    to_delete.append(name)
                i += 1
            except OSError:
                break
        for name in to_delete:
            try:
                winreg.DeleteValue(key, name)
                print(f"[startup] Removed old registry entry: '{name}'")
            except OSError:
                pass
        winreg.CloseKey(key)
    except OSError:
        pass


def _add_to_startup() -> None:
    """Drop HideDesktopApps.vbs into the Startup folder."""
    _scrub_old_startup_files()

    pythonw, script = _startup_target()
    vbs = _get_vbs_path()

    try:
        os.makedirs(_STARTUP_DIR, exist_ok=True)
        _write_vbs(vbs, pythonw, script)
        size = os.path.getsize(vbs)
        print(f"[startup] VBS launcher written: {vbs}  ({size} bytes)")
        print(f"[startup] Shows as '{APP_NAME}' in Task Manager → Startup tab.")
        print(f"[startup] (Close and reopen Task Manager if it is already open.)")
    except OSError as exc:
        print(f"[startup] Failed to write VBS: {exc}")


def _remove_from_startup() -> None:
    vbs = _get_vbs_path()
    if os.path.exists(vbs):
        try:
            os.remove(vbs)
            print(f"[startup] Removed: {vbs}")
        except OSError as exc:
            print(f"[startup] Could not remove VBS: {exc}")
    else:
        print("[startup] VBS not found (already removed?).")
    _scrub_old_startup_files()


def _check_startup() -> bool:
    return os.path.exists(_get_vbs_path())


# ---------------------------------------------------------------------------
# Exit
# ---------------------------------------------------------------------------

def _exit(icon: pystray.Icon, _item) -> None:
    with _lock:
        if _windows_hidden:
            _show_all_windows()
        if _icons_hidden:
            _show_desktop_icons()
    icon.stop()


# ---------------------------------------------------------------------------
# main()  —  called in a loop so restart works without spawning a new process
# ---------------------------------------------------------------------------

def main() -> None:
    global _icon, _restart_requested

    _ensure_config()
    _add_to_startup()

    cfg   = _load_config()
    label = _hotkey_label(cfg)

    threading.Thread(target=_run_hotkey_listener, daemon=True).start()

    menu = pystray.Menu(
        pystray.MenuItem("Toggle Desktop Icons",      _toggle_icons,   default=True),
        pystray.MenuItem("Toggle All Windows",        _toggle_windows),
        pystray.MenuItem("Settings...",               _open_settings),
        pystray.MenuItem("Restart",                   _request_restart),
        pystray.Menu.SEPARATOR,
        pystray.MenuItem("Exit (restore everything)", _exit),
    )

    _icon = pystray.Icon(
        APP_NAME,
        _draw_icon(False, False),
        f"{APP_NAME} — {label} or double-click to toggle icons",
        menu,
    )

    _icon.run()   # blocks until _exit() or _request_restart() calls icon.stop()


# ---------------------------------------------------------------------------
# Entry point
# ---------------------------------------------------------------------------

if __name__ == "__main__":
    if "--add-startup" in sys.argv:
        _create_icon_file()
        _add_to_startup()
    elif "--remove-startup" in sys.argv:
        _remove_from_startup()
    elif "--check-startup" in sys.argv:
        print("ENABLED" if _check_startup() else "DISABLED")
    else:
        # Loop so that _request_restart() can re-initialise without spawning
        # a new process (picks up new config, re-registers hotkey, etc.)
        while True:
            _restart_requested = False
            main()
            if not _restart_requested:
                break
            print("[restart] Restarting...")
