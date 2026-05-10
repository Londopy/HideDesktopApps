"""
hide_desktop.py — v0.2
Lightweight system-tray app: hide/show desktop icons, taskbar, and windows.

Controls
--------
  Ctrl+Alt+H  (configurable) → toggle desktop icons
  Ctrl+Alt+W  (configurable) → toggle all windows
  Tray double-click          → toggle desktop icons
  Right-click tray icon      → full menu

Menu
----
  Toggle Desktop Icons   (default / double-click)
  Toggle Taskbar
  Toggle All Windows
  Settings...
  Restart
  ------------------
  Exit (restore everything)

Config  (config.ini — auto-created on first run)
------
  [hotkey]
  modifiers = ctrl+alt      ; Ctrl, Alt, Shift, Win — combine with +
  key = h

  [hotkey_windows]
  modifiers = ctrl+alt
  key = w

  [startup]
  run_at_startup = true
  delay = 30                ; seconds to wait after login before launching

  [defaults]
  icons_hidden = false      ; start with desktop icons already hidden

Dependencies
------------
  pip install pystray pillow pywin32
  (tkinter ships with standard Python on Windows)
"""
from __future__ import annotations

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

_DEFAULT_CONFIG: dict[str, dict[str, str]] = {
    "hotkey":         {"modifiers": "ctrl+alt", "key": "h"},
    "hotkey_windows": {"modifiers": "ctrl+alt", "key": "w"},
    "hotkey_taskbar": {"modifiers": "ctrl+alt", "key": "t"},
    "startup":        {"run_at_startup": "true", "delay": "30"},
    "defaults":       {"icons_hidden": "false"},
}


def _load_config() -> configparser.ConfigParser:
    cfg = configparser.ConfigParser()
    cfg.read_dict(_DEFAULT_CONFIG)
    if os.path.exists(CONFIG_FILE):
        cfg.read(CONFIG_FILE, encoding="utf-8")
    return cfg


def _save_config(cfg: configparser.ConfigParser) -> None:
    with open(CONFIG_FILE, "w", encoding="utf-8") as fh:
        fh.write("# HideDesktopApps configuration\n")
        fh.write("# Hotkey modifiers: ctrl, alt, shift, win  (combine with +)\n")
        fh.write("# Hotkey key: any single letter A-Z\n\n")
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
_taskbar_hidden    : bool = False
_hidden_windows    : list[tuple[int, tuple]] = []
_icon              : pystray.Icon | None = None
_restart_requested : bool = False

# Classes excluded from "Toggle All Windows" (but taskbar is toggled separately)
_SHELL_CLASSES = {
    "Shell_TrayWnd", "Shell_SecondaryTrayWnd", "Progman", "WorkerW", "Button",
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
# Taskbar toggle
# ---------------------------------------------------------------------------

def _find_taskbars() -> list[int]:
    """Return handles for the primary taskbar and any secondary-monitor taskbars."""
    hwnds: list[int] = []
    main = win32gui.FindWindow("Shell_TrayWnd", None)
    if main:
        hwnds.append(main)

    def _cb(hwnd: int, _) -> None:
        if win32gui.GetClassName(hwnd) == "Shell_SecondaryTrayWnd":
            hwnds.append(hwnd)

    win32gui.EnumWindows(_cb, None)
    return hwnds


def _hide_taskbar() -> None:
    global _taskbar_hidden
    for hwnd in _find_taskbars():
        win32gui.ShowWindow(hwnd, win32con.SW_HIDE)
    _taskbar_hidden = True
    _refresh_icon()


def _show_taskbar() -> None:
    global _taskbar_hidden
    for hwnd in _find_taskbars():
        win32gui.ShowWindow(hwnd, win32con.SW_SHOW)
    _taskbar_hidden = False
    _refresh_icon()


def _toggle_taskbar(_icon_arg=None, _item=None) -> None:
    with _lock:
        if _taskbar_hidden:
            _show_taskbar()
        else:
            _hide_taskbar()


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

def _draw_icon(icons_hidden: bool, windows_hidden: bool,
               taskbar_hidden: bool = False) -> Image.Image:
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

    # Orange dot in the bottom-right corner when taskbar is hidden
    # r=14 so it scales to ~3-4px at 16px tray size — visible but not overwhelming
    if taskbar_hidden:
        r = 14
        d.ellipse((size - r*2 - 1, size - r*2 - 1, size - 1, size - 1),
                  fill="#e67e22", outline="white", width=2)

    return img


def _refresh_icon() -> None:
    if _icon is None:
        return
    _icon.icon = _draw_icon(_icons_hidden, _windows_hidden, _taskbar_hidden)
    parts: list[str] = []
    if _icons_hidden:
        parts.append("icons hidden")
    if _windows_hidden:
        parts.append("windows hidden")
    if _taskbar_hidden:
        parts.append("taskbar hidden")
    _icon.title = f"{APP_NAME} — {' · '.join(parts) or 'everything visible'}"


# ---------------------------------------------------------------------------
# .ico file
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
# Defaults
# ---------------------------------------------------------------------------

def _apply_defaults(cfg: configparser.ConfigParser) -> None:
    """Apply [defaults] immediately after the tray icon is created."""
    if cfg["defaults"].getboolean("icons_hidden", False):
        _hide_desktop_icons()


# ---------------------------------------------------------------------------
# Restart
# ---------------------------------------------------------------------------

def _request_restart(_icon_arg=None, _item=None) -> None:
    """Restore everything, then stop the icon. main() loop re-calls main()."""
    global _restart_requested
    with _lock:
        if _taskbar_hidden:
            _show_taskbar()
        if _windows_hidden:
            _show_all_windows()
        if _icons_hidden:
            _show_desktop_icons()
    _restart_requested = True
    if _icon is not None:
        _icon.stop()


# ---------------------------------------------------------------------------
# Settings window  (tabbed: Hotkeys / Startup / Defaults)
# ---------------------------------------------------------------------------

def _settings_window() -> None:
    """Full settings dialog. Runs in its own daemon thread."""
    cfg = _load_config()

    root = tk.Tk()
    root.title("HideDesktopApps — Settings")
    root.resizable(False, False)
    root.attributes("-topmost", True)

    notebook = ttk.Notebook(root)
    notebook.pack(fill="both", expand=True, padx=12, pady=(12, 4))

    pad = dict(padx=14)

    # ── shared helper: build one hotkey block ────────────────────────────────

    def _make_hotkey_block(parent, section: str, def_mods: str, def_key: str):
        """
        Add modifier checkboxes + key entry + preview label to `parent`.
        Returns a callable that returns (mods_str, key_str) or None if invalid.
        """
        mods_raw = cfg[section].get("modifiers", def_mods).lower()
        key_raw  = cfg[section].get("key", def_key).upper()

        ctrl_v  = tk.BooleanVar(value="ctrl"  in mods_raw)
        alt_v   = tk.BooleanVar(value="alt"   in mods_raw)
        shift_v = tk.BooleanVar(value="shift" in mods_raw)
        win_v   = tk.BooleanVar(value="win"   in mods_raw)
        key_v   = tk.StringVar(value=key_raw)
        prev_v  = tk.StringVar()

        def _upd(*_):
            parts = (
                (["Ctrl"]  if ctrl_v.get()  else []) +
                (["Alt"]   if alt_v.get()   else []) +
                (["Shift"] if shift_v.get() else []) +
                (["Win"]   if win_v.get()   else [])
            )
            k = key_v.get().strip().upper()[:1] or "?"
            prev_v.set("+".join(parts + [k]))

        for v in (ctrl_v, alt_v, shift_v, win_v, key_v):
            v.trace_add("write", _upd)

        mf = ttk.Frame(parent)
        mf.pack(**pad, pady=(4, 2), anchor="w")
        ttk.Label(mf, text="Modifiers:").grid(row=0, column=0, sticky="w")
        for col, (lbl, var) in enumerate(
            [("Ctrl", ctrl_v), ("Alt", alt_v), ("Shift", shift_v), ("Win", win_v)], 1
        ):
            ttk.Checkbutton(mf, text=lbl, variable=var).grid(
                row=0, column=col, padx=6, sticky="w")

        kf = ttk.Frame(parent)
        kf.pack(**pad, anchor="w")
        ttk.Label(kf, text="Key (A-Z):").grid(row=0, column=0, sticky="w")
        ttk.Entry(kf, textvariable=key_v, width=4,
                  justify="center", font=("Consolas", 12, "bold")).grid(
            row=0, column=1, padx=10)

        ttk.Label(parent, text="Preview:").pack(**pad, pady=(8, 0), anchor="w")
        ttk.Label(parent, textvariable=prev_v,
                  font=("Consolas", 13, "bold"), foreground="#0078d4").pack(
            **pad, pady=(0, 8), anchor="w")

        _upd()

        def get_values():
            mods = (
                (["ctrl"]  if ctrl_v.get()  else []) +
                (["alt"]   if alt_v.get()   else []) +
                (["shift"] if shift_v.get() else []) +
                (["win"]   if win_v.get()   else [])
            )
            k = key_v.get().strip().upper()[:1]
            if not mods or not k or not k.isalpha():
                return None
            return ("+".join(mods), k.lower())

        return get_values

    # ── Tab 1: Hotkeys ────────────────────────────────────────────────────────

    tab_hk = ttk.Frame(notebook)
    notebook.add(tab_hk, text="  Hotkeys  ")

    ttk.Label(tab_hk, text="Toggle Desktop Icons",
              font=("Segoe UI", 9, "bold")).pack(**pad, pady=(12, 4), anchor="w")
    ttk.Separator(tab_hk, orient="horizontal").pack(fill="x", padx=14)
    get_icons_hk = _make_hotkey_block(tab_hk, "hotkey", "ctrl+alt", "h")

    ttk.Label(tab_hk, text="Toggle Taskbar",
              font=("Segoe UI", 9, "bold")).pack(**pad, pady=(10, 4), anchor="w")
    ttk.Separator(tab_hk, orient="horizontal").pack(fill="x", padx=14)
    get_taskbar_hk = _make_hotkey_block(tab_hk, "hotkey_taskbar", "ctrl+alt", "t")

    ttk.Label(tab_hk, text="Toggle All Windows",
              font=("Segoe UI", 9, "bold")).pack(**pad, pady=(10, 4), anchor="w")
    ttk.Separator(tab_hk, orient="horizontal").pack(fill="x", padx=14)
    get_windows_hk = _make_hotkey_block(tab_hk, "hotkey_windows", "ctrl+alt", "w")

    ttk.Label(tab_hk, text="App restarts automatically when hotkeys are changed.",
              foreground="gray").pack(**pad, pady=(4, 10), anchor="w")

    # ── Tab 2: Startup ────────────────────────────────────────────────────────

    tab_su = ttk.Frame(notebook)
    notebook.add(tab_su, text="  Startup  ")

    run_var   = tk.BooleanVar(value=cfg["startup"].getboolean("run_at_startup", True))
    delay_var = tk.IntVar(value=max(0, min(120, cfg["startup"].getint("delay", 30))))

    ttk.Label(tab_su, text="Auto-start",
              font=("Segoe UI", 9, "bold")).pack(**pad, pady=(12, 4), anchor="w")
    ttk.Separator(tab_su, orient="horizontal").pack(fill="x", padx=14)
    ttk.Checkbutton(tab_su,
                    text="Run HideDesktopApps when Windows starts",
                    variable=run_var).pack(**pad, pady=(8, 4), anchor="w")

    ttk.Label(tab_su, text="Startup delay",
              font=("Segoe UI", 9, "bold")).pack(**pad, pady=(10, 4), anchor="w")
    ttk.Separator(tab_su, orient="horizontal").pack(fill="x", padx=14)
    df = ttk.Frame(tab_su)
    df.pack(**pad, pady=(8, 4), anchor="w")
    ttk.Label(df, text="Wait").grid(row=0, column=0, sticky="w")
    ttk.Spinbox(df, from_=0, to=120, textvariable=delay_var,
                width=5, justify="center").grid(row=0, column=1, padx=8)
    ttk.Label(df, text="seconds after login before launching").grid(
        row=0, column=2, sticky="w")
    ttk.Label(tab_su,
              text="Startup changes take effect next time you log in.",
              foreground="gray").pack(**pad, pady=(4, 12), anchor="w")

    # ── Tab 3: Defaults ───────────────────────────────────────────────────────

    tab_df = ttk.Frame(notebook)
    notebook.add(tab_df, text="  Defaults  ")

    icons_def_var = tk.BooleanVar(
        value=cfg["defaults"].getboolean("icons_hidden", False))

    ttk.Label(tab_df, text="State on launch",
              font=("Segoe UI", 9, "bold")).pack(**pad, pady=(12, 4), anchor="w")
    ttk.Separator(tab_df, orient="horizontal").pack(fill="x", padx=14)
    ttk.Checkbutton(tab_df,
                    text="Start with desktop icons hidden",
                    variable=icons_def_var).pack(**pad, pady=(8, 4), anchor="w")
    ttk.Label(tab_df,
              text="Takes effect the next time the app starts or restarts.",
              foreground="gray").pack(**pad, pady=(4, 12), anchor="w")

    # ── Apply / Cancel (always visible below tabs) ────────────────────────────

    ttk.Separator(root, orient="horizontal").pack(fill="x", padx=12, pady=(4, 0))
    bf = ttk.Frame(root)
    bf.pack(padx=14, pady=10)

    def _apply() -> None:
        icons_hk   = get_icons_hk()
        taskbar_hk = get_taskbar_hk()
        windows_hk = get_windows_hk()

        if icons_hk is None:
            messagebox.showerror(
                "Icons hotkey",
                "Select at least one modifier and a letter A-Z.",
                parent=root)
            notebook.select(0)
            return
        if taskbar_hk is None:
            messagebox.showerror(
                "Taskbar hotkey",
                "Select at least one modifier and a letter A-Z.",
                parent=root)
            notebook.select(0)
            return
        if windows_hk is None:
            messagebox.showerror(
                "Windows hotkey",
                "Select at least one modifier and a letter A-Z.",
                parent=root)
            notebook.select(0)
            return
        all_hks = [icons_hk, taskbar_hk, windows_hk]
        if len(set(all_hks)) < len(all_hks):
            messagebox.showerror(
                "Hotkey conflict",
                "All three hotkeys must be different.",
                parent=root)
            notebook.select(0)
            return

        # Detect hotkey change so we know whether a restart is needed
        old_icons_hk   = (cfg["hotkey"].get("modifiers",          "ctrl+alt"),
                          cfg["hotkey"].get("key",                 "h"))
        old_taskbar_hk = (cfg["hotkey_taskbar"].get("modifiers",  "ctrl+alt"),
                          cfg["hotkey_taskbar"].get("key",         "t"))
        old_windows_hk = (cfg["hotkey_windows"].get("modifiers",  "ctrl+alt"),
                          cfg["hotkey_windows"].get("key",         "w"))
        hotkeys_changed = (icons_hk != old_icons_hk or
                           taskbar_hk != old_taskbar_hk or
                           windows_hk != old_windows_hk)

        # Clamp delay to valid range
        try:
            delay = max(0, min(120, int(delay_var.get())))
        except (ValueError, tk.TclError):
            delay = 30

        # Persist everything
        cfg["hotkey"]["modifiers"]          = icons_hk[0]
        cfg["hotkey"]["key"]                = icons_hk[1]
        cfg["hotkey_taskbar"]["modifiers"]  = taskbar_hk[0]
        cfg["hotkey_taskbar"]["key"]        = taskbar_hk[1]
        cfg["hotkey_windows"]["modifiers"]  = windows_hk[0]
        cfg["hotkey_windows"]["key"]        = windows_hk[1]
        cfg["startup"]["run_at_startup"]    = str(run_var.get()).lower()
        cfg["startup"]["delay"]             = str(delay)
        cfg["defaults"]["icons_hidden"]     = str(icons_def_var.get()).lower()
        _save_config(cfg)

        # Apply startup changes immediately (writes or removes VBS)
        if run_var.get():
            _add_to_startup(cfg)
        else:
            _remove_from_startup()

        root.destroy()

        if hotkeys_changed:
            _request_restart()

    ttk.Button(bf, text="Apply & Save", command=_apply).pack(side="left")
    ttk.Button(bf, text="Cancel", command=root.destroy).pack(side="left", padx=10)

    root.mainloop()


def _open_settings(_icon_arg=None, _item=None) -> None:
    threading.Thread(target=_settings_window, daemon=True).start()


# ---------------------------------------------------------------------------
# Global hotkeys
# ---------------------------------------------------------------------------

_MOD_MAP            = {"ctrl": 0x0002, "alt": 0x0001, "shift": 0x0004, "win": 0x0008}
_MOD_NOREPEAT       = 0x4000
_HOTKEY_ID_ICONS    = 1
_HOTKEY_ID_WINDOWS  = 2
_HOTKEY_ID_TASKBAR  = 3
_WM_HOTKEY          = 0x0312


def _parse_hotkey(cfg: configparser.ConfigParser,
                  section: str = "hotkey") -> tuple[int, int]:
    defs     = _DEFAULT_CONFIG.get(section, {"modifiers": "ctrl+alt", "key": "h"})
    mods_raw = cfg[section].get("modifiers", defs["modifiers"]).lower()
    key_raw  = cfg[section].get("key",       defs["key"]).upper()[:1]
    mods = _MOD_NOREPEAT
    for name, flag in _MOD_MAP.items():
        if name in mods_raw:
            mods |= flag
    return mods, (ord(key_raw) if key_raw.isalpha() else ord("H"))


def _hotkey_label(cfg: configparser.ConfigParser,
                  section: str = "hotkey") -> str:
    defs     = _DEFAULT_CONFIG.get(section, {"modifiers": "ctrl+alt", "key": "h"})
    mods_raw = cfg[section].get("modifiers", defs["modifiers"]).lower()
    key_raw  = cfg[section].get("key",       defs["key"]).upper()[:1]
    return "+".join([n.capitalize() for n in _MOD_MAP if n in mods_raw] + [key_raw])


def _run_hotkey_listener() -> None:
    cfg = _load_config()

    mods1, vk1 = _parse_hotkey(cfg, "hotkey")
    mods2, vk2 = _parse_hotkey(cfg, "hotkey_windows")
    mods3, vk3 = _parse_hotkey(cfg, "hotkey_taskbar")
    label1     = _hotkey_label(cfg, "hotkey")
    label2     = _hotkey_label(cfg, "hotkey_windows")
    label3     = _hotkey_label(cfg, "hotkey_taskbar")

    ok1 = bool(ctypes.windll.user32.RegisterHotKey(None, _HOTKEY_ID_ICONS,   mods1, vk1))
    ok2 = bool(ctypes.windll.user32.RegisterHotKey(None, _HOTKEY_ID_WINDOWS, mods2, vk2))
    ok3 = bool(ctypes.windll.user32.RegisterHotKey(None, _HOTKEY_ID_TASKBAR, mods3, vk3))

    if not ok1:
        print(f"[warn] Could not register icons hotkey {label1} — open Settings.")
    if not ok2:
        print(f"[warn] Could not register windows hotkey {label2} — open Settings.")
    if not ok3:
        print(f"[warn] Could not register taskbar hotkey {label3} — open Settings.")

    msg = ctypes.wintypes.MSG()
    while ctypes.windll.user32.GetMessageW(ctypes.byref(msg), None, 0, 0) > 0:
        if msg.message == _WM_HOTKEY:
            if msg.wParam == _HOTKEY_ID_ICONS:
                _toggle_icons()
            elif msg.wParam == _HOTKEY_ID_WINDOWS:
                _toggle_windows()
            elif msg.wParam == _HOTKEY_ID_TASKBAR:
                _toggle_taskbar()
        ctypes.windll.user32.TranslateMessage(ctypes.byref(msg))
        ctypes.windll.user32.DispatchMessageW(ctypes.byref(msg))

    if ok1:
        ctypes.windll.user32.UnregisterHotKey(None, _HOTKEY_ID_ICONS)
    if ok2:
        ctypes.windll.user32.UnregisterHotKey(None, _HOTKEY_ID_WINDOWS)
    if ok3:
        ctypes.windll.user32.UnregisterHotKey(None, _HOTKEY_ID_TASKBAR)


# ---------------------------------------------------------------------------
# Startup — VBS launcher in the Windows Startup folder
# ---------------------------------------------------------------------------

_REG_RUN     = r"Software\Microsoft\Windows\CurrentVersion\Run"
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


def _write_vbs(vbs_path: str, pythonw: str, script: str,
               delay_s: int = 30) -> None:
    """
    Write the VBS launcher.
    WScript.Sleep takes milliseconds; WindowStyle 0 = hidden; False = fire-and-forget.
    """
    q          = chr(34)
    sleep_line = f"WScript.Sleep {delay_s * 1000}\r\n" if delay_s > 0 else ""
    content    = (
        f"' {APP_NAME} — auto-generated launcher, do not edit manually\r\n"
        f"{sleep_line}"
        f'CreateObject("WScript.Shell").Run '
        f'{q}{pythonw}{q} & " " & {q}{script}{q}, 0, False\r\n'
    )
    with open(vbs_path, "w", encoding="utf-8") as fh:
        fh.write(content)


def _scrub_old_startup_files() -> None:
    """Remove leftover .lnk shortcuts and stale registry entries."""
    lnk = os.path.join(_STARTUP_DIR, f"{APP_NAME}.lnk")
    if os.path.exists(lnk):
        try:
            os.remove(lnk)
        except OSError:
            pass
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
            except OSError:
                pass
        winreg.CloseKey(key)
    except OSError:
        pass


def _add_to_startup(cfg: configparser.ConfigParser | None = None) -> None:
    """Write HideDesktopApps.vbs to the Startup folder using delay from config."""
    if cfg is None:
        cfg = _load_config()
    _scrub_old_startup_files()
    pythonw, script = _startup_target()
    delay_s = max(0, cfg["startup"].getint("delay", 30))
    vbs = _get_vbs_path()
    try:
        os.makedirs(_STARTUP_DIR, exist_ok=True)
        _write_vbs(vbs, pythonw, script, delay_s)
        print(f"[startup] VBS launcher written ({delay_s}s delay): {vbs}")
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
    _scrub_old_startup_files()


def _check_startup() -> bool:
    return os.path.exists(_get_vbs_path())


# ---------------------------------------------------------------------------
# Exit
# ---------------------------------------------------------------------------

def _exit(icon: pystray.Icon, _item) -> None:
    with _lock:
        if _taskbar_hidden:
            _show_taskbar()
        if _windows_hidden:
            _show_all_windows()
        if _icons_hidden:
            _show_desktop_icons()
    icon.stop()


# ---------------------------------------------------------------------------
# main()  —  called in a loop so restart works in-process
# ---------------------------------------------------------------------------

def main() -> None:
    global _icon, _restart_requested

    _ensure_config()
    cfg = _load_config()

    # Startup registration — add or remove VBS based on config
    if cfg["startup"].getboolean("run_at_startup", True):
        _add_to_startup(cfg)
    else:
        _remove_from_startup()

    label_icons   = _hotkey_label(cfg, "hotkey")
    label_windows = _hotkey_label(cfg, "hotkey_windows")
    label_taskbar = _hotkey_label(cfg, "hotkey_taskbar")

    threading.Thread(target=_run_hotkey_listener, daemon=True).start()

    menu = pystray.Menu(
        pystray.MenuItem("Toggle Desktop Icons",      _toggle_icons,   default=True),
        pystray.MenuItem("Toggle Taskbar",            _toggle_taskbar),
        pystray.MenuItem("Toggle All Windows",        _toggle_windows),
        pystray.MenuItem("Settings...",               _open_settings),
        pystray.MenuItem("Restart",                   _request_restart),
        pystray.Menu.SEPARATOR,
        pystray.MenuItem("Exit (restore everything)", _exit),
    )

    _icon = pystray.Icon(
        APP_NAME,
        _draw_icon(False, False),
        f"{APP_NAME} — {label_icons} · icons  |  {label_taskbar} · taskbar  |  {label_windows} · windows",
        menu,
    )

    # Apply default state after icon exists so _refresh_icon() can update it
    _apply_defaults(cfg)

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
        # Loop so _request_restart() can re-initialise without spawning a new process
        while True:
            _restart_requested = False
            main()
            if not _restart_requested:
                break
            print("[restart] Restarting...")
