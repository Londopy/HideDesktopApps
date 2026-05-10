"""
hide-desktop-apps
=================
Windows system-tray utility to hide and show desktop icons,
the taskbar, and all open windows — via hotkeys or the tray menu.

Usage
-----
After installing with pip::

    pip install hide-desktop-apps

Run it::

    hide-desktop-apps

Or as a module::

    python -m hide_desktop_apps

Public API
----------
Only ``main()`` is part of the public API.  Everything else is internal.
"""

from hide_desktop_apps.app import main  # noqa: F401

__version__ = "0.4.0"
__all__ = ["main"]
