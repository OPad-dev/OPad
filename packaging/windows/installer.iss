; OPad Inno Setup Installer Script (§W2-1, §W2-3)
; Produces: target/installer/OPad-Setup-<version>.exe

#define MyAppName "OPad"
#ifndef MyAppVersion
  #define CargoTomlPath "..\..\desktop\Cargo.toml"
  #define FileHandle FileOpen(CargoTomlPath)
  #if !FileHandle
    #error "Could not open " + CargoTomlPath + " to derive installer version!"
  #endif
  #define public FoundVersion 0
  #define public MyAppVersion ""
  #sub ProcessCargoLine
    #define FileLine FileRead(FileHandle)
    #if Pos("version = """, Trim(FileLine)) == 1
      #define LineTrimmed Trim(FileLine)
      #define Remainder Copy(LineTrimmed, 12, Len(LineTrimmed))
      #define EndQuote Pos("""", Remainder)
      #expr MyAppVersion = Copy(Remainder, 1, EndQuote - 1)
      #expr FoundVersion = 1
    #endif
  #endsub
  #define LoopIdx 0
  #for {LoopIdx = 0; !FileEof(FileHandle) && !FoundVersion; LoopIdx = LoopIdx + 1} ProcessCargoLine
  #expr FileClose(FileHandle)
  #if !FoundVersion || (MyAppVersion == "")
    #error "Could not read workspace version from " + CargoTomlPath + "!"
  #endif
#endif
#define MyAppPublisher "GFerreiroS"
#define MyAppURL "https://github.com/OPad-dev/OPad"
#define MyAppExeName "opad-gui.exe"
#define MyAppDaemonName "opad-daemon.exe"
#define MyAppCliName "opadctl.exe"

#ifndef SourceDir
  #define SourceDir "..\..\desktop\target\release"
#endif
#ifndef TosuDir
  #define TosuDir "..\..\build\tosu"
#endif
#ifndef LicensesDir
  #define LicensesDir "..\..\licenses"
#endif

[Setup]
; Fixed AppId across versions ensures upgrades replace cleanly rather than stacking
AppId={{E581335A-9877-4C7D-894C-97495B422B59}}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppVerName={#MyAppName} {#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppURL}
AppSupportURL={#MyAppURL}
AppUpdatesURL={#MyAppURL}
DefaultDirName={localappdata}\Programs\opad
DefaultGroupName=OPad
DisableProgramGroupPage=yes
; Per-user install: avoids requiring administrator privileges, matches per-user daemon model
PrivilegesRequired=lowest
OutputDir=..\..\build\installer
OutputBaseFilename=opad-setup
Compression=lzma2/ultra64
SolidCompression=yes
WizardStyle=modern
UninstallDisplayName={#MyAppName}
UninstallDisplayIcon={app}\{#MyAppExeName}
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible

; Upgrade behaviour: cleanly stop running instances before replacing binaries
CloseApplications=yes
CloseApplicationsFilter=opad-gui.exe,opad-daemon.exe,opadctl.exe,osupad-gui.exe,osupad-daemon.exe,osupadctl.exe

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
; Desktop icon is checked by default
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"
; Autostart at login is checked by default (§W2-1)
Name: "autostart"; Description: "Start OPad when I log in"; GroupDescription: "Windows Startup:"

[Files]
; Core application binaries
Source: "{#SourceDir}\{#MyAppExeName}"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\{#MyAppDaemonName}"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\{#MyAppCliName}"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\espflash.exe"; DestDir: "{app}"; Flags: ignoreversion

; Install-origin marker identifying this installation as "windows" (§U-2a)
Source: "install-origin"; DestDir: "{app}"; Flags: ignoreversion

; Bundled tosu and LGPL-3.0 compliance files (§T-2, §T-3)
Source: "{#TosuDir}\tosu.exe"; DestDir: "{app}\tosu"; Flags: ignoreversion
Source: "{#TosuDir}\tosu.env"; DestDir: "{app}\tosu"; Flags: ignoreversion skipifsourcedoesntexist
Source: "{#LicensesDir}\tosu\VERSION"; DestDir: "{app}\tosu"; Flags: ignoreversion
Source: "{#LicensesDir}\tosu\NOTICE"; DestDir: "{app}\tosu"; Flags: ignoreversion
Source: "{#LicensesDir}\tosu\LICENSE"; DestDir: "{app}\tosu"; Flags: ignoreversion

; Top-level application license
Source: "..\..\LICENSE"; DestDir: "{app}"; DestName: "LICENSE.txt"; Flags: ignoreversion
; Montserrat fonts embedded in opad-gui.exe (SIL OFL 1.1)
Source: "..\..\desktop\gui\assets\fonts\Montserrat-OFL.txt"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{userprograms}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"
Name: "{userdesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; Tasks: desktopicon

[Registry]
; Exact autostart values matching W1-1 (platform_windows.rs)
; opad-daemon -> "<dir>\opad-daemon.exe"
; opad-gui    -> "<dir>\opad-gui.exe" --tray
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: string; ValueName: "opad-daemon"; ValueData: """{app}\{#MyAppDaemonName}"""; Tasks: autostart; Flags: uninsdeletevalue
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: string; ValueName: "opad-gui"; ValueData: """{app}\{#MyAppExeName}"" --tray"; Tasks: autostart; Flags: uninsdeletevalue

[Run]
Filename: "{app}\{#MyAppDaemonName}"; Flags: nowait runhidden
Filename: "{app}\{#MyAppExeName}"; Description: "{cm:LaunchProgram,{#StringChange(MyAppName, '&', '&&')}}"; Flags: nowait postinstall skipifsilent

[UninstallDelete]
; Strong uninstall (§W2-3): remove entire install directory and disposable state
Type: filesandordirs; Name: "{app}"
Type: files; Name: "{userappdata}\opad\daemon.log"
Type: files; Name: "{userappdata}\opad\tosu.log"
Type: files; Name: "{userappdata}\osupad\daemon.log"
Type: files; Name: "{userappdata}\osupad\tosu.log"

[Code]
// Strong uninstall contract (§W2-3)
// 1. Unconditionally remove Run autostart keys even if toggled inside the GUI.
// 2. Prompt user before deleting lifetime counters and config in %APPDATA%\opad (default = keep).
// 3. Document why no cleanup is needed for:
//    - Named pipe: Windows kernel object, cleaned up when processes exit.
//    - COM port: managed dynamically by usbser.sys.
//    - Device NVS: hardware counters and calibration intentionally preserved (§W3-4).

// Stop running background processes so binaries are not locked during install or uninstall (§W2-3, §PKG-06)
procedure StopRunningProcesses;
var
  ResultCode: Integer;
  AppTosuPath: String;
  PowerShellCmd: String;
begin
  Exec('taskkill.exe', '/F /IM opad-daemon.exe /IM opad-gui.exe /IM opadctl.exe /IM osupad-daemon.exe /IM osupad-gui.exe /IM osupadctl.exe', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
  AppTosuPath := ExpandConstant('{app}\tosu\tosu.exe');
  StringChange(AppTosuPath, '''', '''''');
  PowerShellCmd := '-NoProfile -NonInteractive -Command "Get-CimInstance Win32_Process | Where-Object { $_.ExecutablePath -eq ''' + AppTosuPath + ''' } | Stop-Process -Force"';
  Exec('powershell.exe', PowerShellCmd, '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
end;

function InitializeUninstall(): Boolean;
begin
  StopRunningProcesses;
  Result := True;
end;

function PrepareToInstall(var NeedsRestart: Boolean): String;
begin
  StopRunningProcesses;
  Result := '';
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
var
  AppDataDir: String;
begin
  if CurUninstallStep = usUninstall then
  begin
    // Unconditionally remove Run registry entries (may have been toggled by the app at runtime)
    RegDeleteValue(HKEY_CURRENT_USER, 'Software\Microsoft\Windows\CurrentVersion\Run', 'opad-daemon');
    RegDeleteValue(HKEY_CURRENT_USER, 'Software\Microsoft\Windows\CurrentVersion\Run', 'opad-gui');
    RegDeleteValue(HKEY_CURRENT_USER, 'Software\Microsoft\Windows\CurrentVersion\Run', 'osupad-daemon');
    RegDeleteValue(HKEY_CURRENT_USER, 'Software\Microsoft\Windows\CurrentVersion\Run', 'osupad-gui');

    AppDataDir := ExpandConstant('{userappdata}\opad');
    if not DirExists(AppDataDir) then
      AppDataDir := ExpandConstant('{userappdata}\osupad');

    if DirExists(AppDataDir) then
    begin
      // Prompt user whether to delete database and lifetime counters. Default is NO (keep).
      // In silent mode (UninstallSilent), preserve user data as per default (§W2-3).
      if (not UninstallSilent) and (MsgBox('Also delete your OPad settings and lifetime key counters? This cannot be undone.',
                mbConfirmation, MB_YESNO or MB_DEFBUTTON2) = IDYES) then
      begin
        DelTree(AppDataDir, True, True, True);
      end
      else
      begin
        // Keep opad.db/opad.db, but purge disposable log files
        DeleteFile(AppDataDir + '\daemon.log');
        DeleteFile(AppDataDir + '\tosu.log');
      end;
    end;
  end;
end;
