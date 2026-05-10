"""
Entry point for ``python -m hide_desktop_apps`` and the
``hide-desktop-apps`` console script installed by pip.

Supports optional CLI flags:
  --add-startup      Write the VBS startup launcher and exit.
  --remove-startup   Remove the VBS startup launcher and exit.
  --check-startup    Print ENABLED or DISABLED and exit.
"""
from __future__ import annotations

import sys

import hide_desktop_apps.app as _app


def main() -> None:
    if "--add-startup" in sys.argv:
        _app._create_icon_file()
        _app._add_to_startup()
    elif "--remove-startup" in sys.argv:
        _app._remove_from_startup()
    elif "--check-startup" in sys.argv:
        print("ENABLED" if _app._check_startup() else "DISABLED")
    else:
        # Inner restart loop — _request_restart() stops the icon; we re-call
        # _app.main() in-process rather than spawning a new process.
        while True:
            _app._restart_requested = False
            _app.main()
            if not _app._restart_requested:
                break
            print("[restart] Restarting...")


if __name__ == "__main__":
    main()
