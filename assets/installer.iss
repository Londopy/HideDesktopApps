; HideDesktopApps — Inno Setup Script
; Build: iscc /DAppVersion=1.0.0 installer.iss
; Or from CI: iscc /DAppVersion=${{ github.ref_name }} installer.iss

#ifndef AppVersion
  #define AppVersion "1.0.0"
#endif

[Setup]
AppId={{A1B2C3D4-E5F6-7890-ABCD-EF1234567890}
AppName=HideDesktopApps
AppVersion={#AppVersion}
AppVerName=HideDesktopApps {#AppVersion}
AppPublisher=HideDesktopApps
AppPublisherURL=https://github.com/Londopy/HideDesktopApps
AppSupportURL=https://github.com/Londopy/HideDesktopApps/issues
AppUpdatesURL=https://github.com/Londopy/HideDesktopApps/releases
DefaultDirName={autopf}\HideDesktopApps
DefaultGroupName=HideDesktopApps
AllowNoIcons=yes
; Output goes to installer-output/ (created by CI)
OutputDir=..\installer-output
OutputBaseFilename=HideDesktopApps-v{#AppVersion}-x64-setup
SetupIconFile=..\hide_desktop.ico
Compression=lzma2/ultra64
SolidCompression=yes
; x64 only for this script — separate scripts/runs for x86 and arm64
ArchitecturesAllowed=x64
ArchitecturesInstallIn64BitMode=x64
WizardStyle=modern
; Don't require admin — install per-user so startup task works without UAC
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
; Minimum Windows 10 1903
MinVersion=10.0.18362
UninstallDisplayIcon={app}\HideDesktopApps.exe
UninstallDisplayName=HideDesktopApps
VersionInfoVersion={#AppVersion}
VersionInfoDescription=HideDesktopApps
VersionInfoCopyright=HideDesktopApps contributors

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; \
  Description: "{cm:CreateDesktopIcon}"; \
  GroupDescription: "{cm:AdditionalIcons}"; \
  Flags: unchecked

[Files]
; Main executable — built by cargo before running iscc
Source: "..\target\x86_64-pc-windows-msvc\release\HideDesktopApps.exe"; \
  DestDir: "{app}"; \
  Flags: ignoreversion

[Icons]
Name: "{group}\HideDesktopApps"; \
  Filename: "{app}\HideDesktopApps.exe"; \
  Comment: "Hide and show desktop icons, taskbar, and windows"
Name: "{group}\{cm:UninstallProgram,HideDesktopApps}"; \
  Filename: "{uninstallexe}"
Name: "{commondesktop}\HideDesktopApps"; \
  Filename: "{app}\HideDesktopApps.exe"; \
  Tasks: desktopicon

[Run]
; Register AUMID in the registry so Windows toast notifications work.
; The AUMID must match what the app uses when sending toasts.
Filename: "powershell.exe"; \
  Parameters: "-WindowStyle Hidden -NonInteractive -Command ""New-Item -Force -Path 'HKCU:\Software\Classes\AppUserModelId\Londopy.HideDesktopApps' | New-ItemProperty -Name 'DisplayName' -Value 'HideDesktopApps' -PropertyType String | Out-Null"""; \
  Flags: runhidden waituntilterminated; \
  StatusMsg: "Registering app identity..."

; Register the startup scheduled task (30-second delay, runs at logon).
Filename: "powershell.exe"; \
  Parameters: "-WindowStyle Hidden -NonInteractive -Command ""$trigger = New-ScheduledTaskTrigger -AtLogOn; $trigger.Delay = 'PT30S'; $action = New-ScheduledTaskAction -Execute '{app}\HideDesktopApps.exe'; $settings = New-ScheduledTaskSettingsSet -ExecutionTimeLimit 0; Register-ScheduledTask -TaskName 'HideDesktopApps' -Trigger $trigger -Action $action -Settings $settings -Force | Out-Null"""; \
  Flags: runhidden waituntilterminated; \
  StatusMsg: "Registering startup task..."

; Launch after install (user can uncheck)
Filename: "{app}\HideDesktopApps.exe"; \
  Description: "{cm:LaunchProgram,HideDesktopApps}"; \
  Flags: nowait postinstall skipifsilent

[UninstallRun]
; Remove startup task on uninstall (silently, ignore errors)
Filename: "powershell.exe"; \
  Parameters: "-WindowStyle Hidden -NonInteractive -Command ""Unregister-ScheduledTask -TaskName 'HideDesktopApps' -Confirm:$false 2>$null"""; \
  Flags: runhidden skipifdoesntexist waituntilterminated

; Remove AUMID registry entry
Filename: "powershell.exe"; \
  Parameters: "-WindowStyle Hidden -NonInteractive -Command ""Remove-Item -Path 'HKCU:\Software\Classes\AppUserModelId\Londopy.HideDesktopApps' -Recurse -Force -ErrorAction SilentlyContinue"""; \
  Flags: runhidden waituntilterminated

[UninstallDelete]
; Clean up config and generated files left in AppData
Type: filesandordirs; Name: "{userappdata}\HideDesktopApps"

[Code]
// Kill the running instance before install/uninstall so the exe isn't locked.
procedure TerminateApp();
var
  ResultCode: Integer;
begin
  Exec('taskkill.exe', '/F /IM HideDesktopApps.exe', '', SW_HIDE,
       ewWaitUntilTerminated, ResultCode);
  Sleep(500);
end;

procedure CurStepChanged(CurStep: TSetupStep);
begin
  if CurStep = ssInstall then
    TerminateApp();
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep = usUninstall then
    TerminateA