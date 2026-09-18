; osu!pad Inno Setup Installer Script (§W2-1, §W2-3)
; Compliant with per-user installation, clean upgrade, and strong uninstall contracts.

#define MyAppName "osu!pad"
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
#define MyAppURL "https://github.com/GFerreiroS/osupad"
#define MyAppExeName "osupad-gui.exe"
#define MyAppDaemonName "osupad-daemon.exe"
#define MyAppCliName "osupadctl.exe"

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
DefaultDirName={localappdata}\Programs\osupad
DefaultGroupName=osu!pad
DisableProgramGroupPage=yes
; Per-user install: avoids requiring administrator privileges, matches per-user daemon model
PrivilegesRequired=lowest
OutputDir=..\..\build\installer
OutputBaseFilename=osupad-setup
Compression=lzma2/ultra64
SolidCompression=yes
WizardStyle=modern
UninstallDisplayName={#MyAppName}
UninstallDisplayIcon={app}\{#MyAppExeName}
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible

; Upgrade behaviour: cleanly stop running instances before replacing binaries
CloseApplications=yes
CloseApplicationsFilter=osupad-gui.exe,osupad-daemon.exe,osupadctl.exe,tosu.exe

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
; Desktop icon is unchecked by default (§W2-1)
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked
; Autostart at login is checked by default (§W2-1)
Name: "autostart"; Description: "Start osu!pad when I log in"; GroupDescription: "Windows Startup:"

[Files]
; Core application binaries
Source: "{#SourceDir}\{#MyAppExeName}"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\{#MyAppDaemonName}"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\{#MyAppCliName}"; DestDir: "{app}"; Flags: ignoreversion

; Install-origin marker identifying this installation as "windows" (§U-2a)
Source: "install-origin"; DestDir: "{app}"; Flags: ignoreversion

; Bundled tosu and LGPL-3.0 compliance files (§T-2, §T-3)
Source: "{#TosuDir}\tosu.exe"; DestDir: "{app}\tosu"; Flags: ignoreversion skipifsourcedoesntexist
Source: "{#LicensesDir}\tosu\VERSION"; DestDir: "{app}\tosu"; Flags: ignoreversion
Source: "{#LicensesDir}\tosu\NOTICE"; DestDir: "{app}\tosu"; Flags: ignoreversion
Source: "{#LicensesDir}\tosu\LICENSE"; DestDir: "{app}\tosu"; Flags: ignoreversion

; Top-level application license
Source: "..\..\LICENSE"; DestDir: "{app}"; DestName: "LICENSE.txt"; Flags: ignoreversion

[Icons]
Name: "{userprograms}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"
Name: "{userdesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; Tasks: desktopicon

[Registry]
; Exact autostart values matching W1-1 (platform_windows.rs)
; osupad-daemon -> "<dir>\osupad-daemon.exe"
; osupad-gui    -> "<dir>\osupad-gui.exe" --tray
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: string; ValueName: "osupad-daemon"; ValueData: """{app}\{#MyAppDaemonName}"""; Tasks: autostart; Flags: uninsdeletevalue
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: string; ValueName: "osupad-gui"; ValueData: """{app}\{#MyAppExeName}"" --tray"; Tasks: autostart; Flags: uninsdeletevalue

[Run]
Filename: "{app}\{#MyAppDaemonName}"; Flags: nowait runhidden
Filename: "{app}\{#MyAppExeName}"; Description: "{cm:LaunchProgram,{#StringChange(MyAppName, '&', '&&')}}"; Flags: nowait postinstall skipifsilent

[UninstallDelete]
; Strong uninstall (§W2-3): remove entire install directory and disposable state
Type: filesandordirs; Name: "{app}"
Type: files; Name: "{userappdata}\osupad\daemon.log"
Type: files; Name: "{userappdata}\osupad\tosu.log"

[Code]
// Strong uninstall contract (§W2-3)
// 1. Unconditionally remove Run autostart keys even if toggled inside the GUI.
// 2. Prompt user before deleting lifetime counters and config in %APPDATA%\osupad (default = keep).
// 3. Document why no cleanup is needed for:
//    - Named pipe: Windows kernel object, cleaned up when processes exit.
//    - COM port: managed dynamically by usbser.sys.
//    - Device NVS: hardware counters and calibration intentionally preserved (§W3-4).

// Stop running background processes so binaries are not locked during install or uninstall (§W2-3, §PKG-06)
procedure StopRunningProcesses;
var
  ResultCode: Integer;
begin
  Exec('taskkill.exe', '/F /IM osupad-daemon.exe /IM osupad-gui.exe /IM tosu.exe', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
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
    RegDeleteValue(HKEY_CURRENT_USER, 'Software\Microsoft\Windows\CurrentVersion\Run', 'osupad-daemon');
    RegDeleteValue(HKEY_CURRENT_USER, 'Software\Microsoft\Windows\CurrentVersion\Run', 'osupad-gui');

    AppDataDir := ExpandConstant('{userappdata}\osupad');
    if DirExists(AppDataDir) then
    begin
      // Prompt user whether to delete database and lifetime counters. Default is NO (keep).
      // In silent mode (UninstallSilent), preserve user data as per default (§W2-3).
      if (not UninstallSilent) and (MsgBox('Also delete your osu!pad settings and lifetime key counters? This cannot be undone.',
                mbConfirmation, MB_YESNO or MB_DEFBUTTON2) = IDYES) then
      begin
        DelTree(AppDataDir, True, True, True);
      end
      else
      begin
        // Keep osupad.db, but purge disposable log files
        DeleteFile(AppDataDir + '\daemon.log');
        DeleteFile(AppDataDir + '\tosu.log');
      end;
    end;
  end;
end;
