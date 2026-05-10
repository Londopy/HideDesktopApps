@echo off
setlocal EnableDelayedExpansion
title HideDesktopApps - Setup

:: NOTE: If this window flashes and closes, right-click it and choose
::       "Run as administrator", or open a Command Prompt here and run it.

:MENU
cls
color 0B
echo.
echo  ====================================================
echo       HideDesktopApps  ^|  Setup and Manager
echo  ====================================================
color 07
echo.
echo   Hotkeys  : Ctrl+Alt+H = toggle icons   Ctrl+Alt+W = toggle windows
echo   Trigger  : hotkey or tray double-click  (change via right-click Settings)
echo.
echo  ----------------------------------------------------
echo.
echo   1  FULL INSTALL  (Recommended)
echo      Install packages + add to startup + run app
echo.
echo   2  Install / update Python packages only
echo   3  Run app now  (adds icon to system tray)
echo   4  Add to Windows startup
echo   5  Remove from Windows startup
echo   6  Check startup status
echo   7  Open this folder in Explorer
echo   8  Run in debug mode  (shows errors in console)
echo   0  Exit
echo.
echo  ----------------------------------------------------
echo.

choice /c 123456780 /n /m "  Choose [0-8]: "
set CHOICE=%errorlevel%

if %CHOICE%==1 goto FULL_INSTALL
if %CHOICE%==2 goto INSTALL
if %CHOICE%==3 goto RUN
if %CHOICE%==4 goto ADD_STARTUP
if %CHOICE%==5 goto REMOVE_STARTUP
if %CHOICE%==6 goto CHECK_STARTUP
if %CHOICE%==7 goto OPEN_FOLDER
if %CHOICE%==8 goto DEBUG_RUN
if %CHOICE%==9 goto EXIT_MENU

goto MENU


:: ============================================================
:FULL_INSTALL
cls
color 0B
echo.
echo  ====================================================
echo   Full Install - Setting everything up...
echo  ====================================================
echo.
color 07

echo  [1/3] Installing Python packages...
echo.
pip install --upgrade pystray pillow pywin32
if %errorlevel% neq 0 (
    color 0C
    echo.
    echo  [!!] Package install failed.
    echo       Make sure Python is installed and on your PATH.
    echo       Then try option 2 to install packages manually.
    color 07
    echo.
    pause
    goto MENU
)
color 0A
echo.
echo  [OK] Packages installed.
color 07

echo.
echo  [2/3] Adding to Windows startup...
echo.
python "%~dp0hide_desktop.py" --add-startup
color 0A
echo  [OK] Startup shortcut created.
echo       App will now appear as "HideDesktopApps" in Task Manager startup.
color 07

echo.
echo  [3/3] Launching app...
echo.
where pythonw >nul 2>&1
if %errorlevel%==0 (
    start "" pythonw "%~dp0hide_desktop.py"
) else (
    start "" python "%~dp0hide_desktop.py"
)

echo.
color 0B
echo  ====================================================
color 0A
echo   All done!
color 07
echo.
echo   - Look for the coloured squares icon in your system tray
echo     (bottom-right corner, near the clock).
echo   - If you don't see it, click the  ^  arrow to expand the tray.
echo   - Right-click the icon for the full menu.
echo   - Ctrl+Alt+H toggles desktop icons.
echo   - Ctrl+Alt+W toggles all open windows.
echo   - Hotkeys can be changed via right-click Settings.
echo.
color 0B
echo  ====================================================
color 07
echo.
pause
goto MENU


:: ============================================================
:INSTALL
cls
color 0B
echo.
echo  ---- Installing / updating Python packages ---------
echo.
color 07
pip install --upgrade pystray pillow pywin32
if %errorlevel%==0 (
    color 0A
    echo.
    echo  [OK] All packages installed successfully.
) else (
    color 0C
    echo.
    echo  [!!] pip returned an error.
    echo       Make sure Python is installed and on your PATH.
)
color 07
echo.
pause
goto MENU


:: ============================================================
:RUN
cls
color 0B
echo.
echo  ---- Launching HideDesktopApps ---------------------
echo.
color 07

where pythonw >nul 2>&1
if %errorlevel%==0 (
    start "" pythonw "%~dp0hide_desktop.py"
) else (
    start "" python "%~dp0hide_desktop.py"
)

color 0A
echo  [OK] App launched.
echo       Look for the coloured squares icon in your system tray
echo       (bottom-right, near the clock). Click ^ to expand if hidden.
color 07
echo.
pause
goto MENU


:: ============================================================
:ADD_STARTUP
cls
color 0B
echo.
echo  ---- Adding to Windows startup ---------------------
echo.
color 07

python "%~dp0hide_desktop.py" --add-startup
if %errorlevel%==0 (
    color 0A
    echo.
    echo  [OK] Done! Check Task Manager -> Startup apps.
    echo       It will appear as "HideDesktopApps" with its own icon.
) else (
    color 0C
    echo.
    echo  [!!] Failed. Make sure Python is installed and on your PATH.
)
color 07
echo.
pause
goto MENU


:: ============================================================
:REMOVE_STARTUP
cls
color 0B
echo.
echo  ---- Removing from Windows startup ----------------
echo.
color 07

python "%~dp0hide_desktop.py" --remove-startup
color 0A
echo  [OK] Done. App will no longer start with Windows.
color 07
echo.
pause
goto MENU


:: ============================================================
:CHECK_STARTUP
cls
color 0B
echo.
echo  ---- Startup status --------------------------------
echo.
color 07

for /f %%S in ('python "%~dp0hide_desktop.py" --check-startup 2^>nul') do set STATUS=%%S

if "!STATUS!"=="ENABLED" (
    color 0A
    echo  [ENABLED]  HideDesktopApps is registered for startup.
    echo             Task Scheduler task: HideDesktopApps
    echo             Visible in: Task Manager ^> Startup tab
) else (
    color 0C
    echo  [DISABLED] HideDesktopApps is NOT registered for startup.
    echo             Run option 4 to add it, or option 1 for a full install.
)
color 07
echo.
pause
goto MENU


:: ============================================================
:OPEN_FOLDER
explorer "%~dp0"
goto MENU


:: ============================================================
:DEBUG_RUN
cls
color 0B
echo.
echo  ---- Debug run (errors visible) --------------------
echo.
color 07
echo  Running with python.exe so errors appear in this window.
echo  Press Ctrl+C to stop the app.
echo.
python "%~dp0hide_desktop.py"
echo.
color 0C
echo  App exited (see any errors above).
color 07
echo.
pause
goto MENU


:: ============================================================
:EXIT_MENU
color 07
cls
echo.
echo  Bye!
echo.
endlocal
exit /b 0
