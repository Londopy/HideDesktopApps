; HideDesktopApps — Inno Setup Script
; Build: iscc /DAppVersion=1.0.0 installer.iss
; Or from CI: iscc /DAppVersion=${{ github.ref_name }} installer.iss

#ifndef AppVersion
  #define AppVersion "1.0.0"
#endif

; VersionInfoVersion must be plain numeric (x.x.x[.x]); strip any prerelease
; suffix like "-rc1" so prerelease tags (e.g. v1.1.4-rc1) still compile.
#if Pos("-", AppVersion) > 0
  #define NumericVersion Copy(AppVersion, 1, Pos("-", AppVersion) - 1)
#else
  #define NumericVersion AppVersion
#endif

; Architecture to build for: x64 (default), x86, or arm64.
; CI passes /DArch=x64|x86|arm64; each run produces one setup.
#ifndef Arch
  #define Arch "x64"
#endif

#if Arch == "x64"
  #define TargetDir "x86_64-pc-windows-msvc"
  #define ArchAllowed "x64compatible"
  #define Arch64Mode "x64compatible"
#elif Arch == "x86"
  #define TargetDir "i686-pc-windows-msvc"
  #define ArchAllowed ""
  #define Arch64Mode ""
#elif Arch == "arm64"
  #define TargetDir "aarch64-pc-windows-msvc"
  #define ArchAllowed "arm64"
  #define Arch64Mode "arm64"
#else
  #error Unknown Arch (expected x64, x86, or arm64)
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
DefaultDirName={localappdata}\HideDesktopApps
DefaultGroupName=HideDesktopApps
AllowNoIcons=yes
; Output goes to installer-output/ (created by CI)
OutputDir=..\installer-output
OutputBaseFilename=HideDesktopApps-v{#AppVersion}-{#Arch}-setup
SetupIconFile=..\hide_desktop.ico
Compression=lzma2/ultra64
SolidCompression=yes
; Architecture controlled by the Arch define above (one setup per arch).
; For x86 these are left empty so the 32-bit build installs on any Windows.
ArchitecturesAllowed={#ArchAllowed}
ArchitecturesInstallIn64BitMode={#Arch64Mode}
WizardStyle=modern
; Don't require admin — install per-user so startup task works without UAC
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
; Minimum Windows 10 1903
MinVersion=10.0.18362
UninstallDisplayIcon={app}\HideDesktopApps.exe
UninstallDisplayName=HideDesktopApps
VersionInfoVersion={#NumericVersion}
VersionInfoDescription=HideDesktopApps
VersionInfoCopyright=HideDesktopApps contributors

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; \
  Description: "{cm:CreateDesktopIcon}"; \
  GroupDescription: "{cm:AdditionalIcons}"

[Files]
; Main executable — built by cargo before running iscc (target dir per arch)
Source: "..\target\{#TargetDir}\release\HideDesktopApps.exe"; \
  DestDir: "{app}"; \
  Flags: ignoreversion

[Icons]
Name: "{group}\HideDesktopApps"; \
  Filename: "{app}\HideDesktopApps.exe"; \
  Comment: "Hide and show desktop icons, taskbar, and windows"
Name: "{group}\{cm:UninstallProgram,HideDesktopApps}"; \
  Filename: "{uninstallexe}"
Name: "{userdesktop}\HideDesktopApps"; \
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
    TerminateApp();
end;
