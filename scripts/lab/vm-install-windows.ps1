# Runs inside a Windows lab VM over SSH as opad (an administrator), from its
# home directory. lab-install.ps1 copied each release's opad-setup-<version>.exe
# into its own directory (old, new). One phase per call, like
# scripts/lab/vm-install-linux.sh:
#
#   install DIR      runs the installer in DIR as OPad's in-app updater does
#                    (/SILENT, here with /SUPPRESSMSGBOXES), with its default
#                    tasks (autostart on); also an upgrade over an older one.
#                    The VM then reboots, so the Run keys start OPad at login
#   check VERSION [DIR]
#                    after that reboot: VERSION is the installed one, the
#                    programs match DIR\FILES.sha256 (if the release has one),
#                    the tray
#                    GUI and the daemon run in the desktop session, opadctl
#                    reaches the daemon, the GUI's window opens on the
#                    desktop, and the app's database exists (the same one as
#                    before, after an upgrade)
#   remove           the uninstaller, silent; nothing of OPad is left: files,
#                    %APPDATA%\opad, Run keys, shortcuts, the Apps entry
param(
    [Parameter(Mandatory)][ValidateSet("install", "check", "remove")][string]$Phase,
    [string]$Arg = "",
    [string]$Dir = ""
)

$ErrorActionPreference = "Continue"
$failed = $false
function Fail([string]$what) { Write-Output "FAIL: $what"; $script:failed = $true }
function Pass([string]$what) { Write-Output "ok:   $what" }
$app = Join-Path $env:LOCALAPPDATA "Programs\opad"
$run = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Run"
$db = Join-Path $env:APPDATA "opad\opad.db"
Set-Location $HOME

function Get-InstalledVersion {
    $e = Get-ItemProperty "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\*" -ErrorAction SilentlyContinue |
        Where-Object { $_.DisplayName -eq "OPad" -or $_.DisplayName -like "OPad *" } | Select-Object -First 1
    if ($e) { return $e.DisplayVersion }
    return ""
}

switch ($Phase) {
    "install" {
        $setup = Get-ChildItem (Join-Path $HOME "$Arg\opad-setup-*.exe") | Select-Object -First 1
        if (-not $setup) { Fail "no opad-setup-*.exe in $HOME\$Arg"; break }
        $p = Start-Process $setup.FullName -PassThru -Wait `
            -ArgumentList "/SILENT", "/SUPPRESSMSGBOXES", "/NORESTART", "/LOG=$HOME\install-$Arg.log"
        if ($p.ExitCode -eq 0) { Pass "installed $(Get-InstalledVersion) ($($setup.Name))" }
        else { Fail "installer exited $($p.ExitCode)"; Get-Content "$HOME\install-$Arg.log" -Tail 15 }
        foreach ($f in "opad-gui.exe", "opad-daemon.exe", "opadctl.exe", "espflash.exe", "tosu\tosu.exe", "unins000.exe") {
            if (-not (Test-Path (Join-Path $app $f))) { Fail "missing $app\$f" }
        }
        $keys = Get-ItemProperty $run -ErrorAction SilentlyContinue
        if ($keys.'opad-daemon' -and $keys.'opad-gui') { Pass "autostart Run keys set" } else { Fail "autostart Run keys missing" }
    }
    "check" {
        $v = Get-InstalledVersion
        if ($v -eq $Arg) { Pass "version $Arg installed" } else { Fail "installed version is '$v', not $Arg" }
        # Integrity: the installed programs are the ones the release built
        $sums = Join-Path $HOME "$Dir\FILES.sha256"
        if ($Dir -and (Test-Path $sums)) {
            $n = 0; $bad = @()
            foreach ($line in Get-Content $sums) {
                if ($line -notmatch '^([0-9a-f]{64})\s+windows:(.+)$') { continue }
                $n++
                $path = Join-Path $app ($Matches[2] -replace '/', '\')
                $h = if (Test-Path $path) { (Get-FileHash $path -Algorithm SHA256).Hash.ToLower() } else { "" }
                if ($h -ne $Matches[1]) { $bad += $Matches[2] }
            }
            if ($n -gt 0 -and $bad.Count -eq 0) { Pass "all $n installed programs match the release's FILES.sha256" }
            else { Fail "installed programs differ from the release's FILES.sha256 ($n listed)"; $bad | ForEach-Object { Write-Output "  differs or missing: $_" } }
        }
        # The Run keys start both a while after the login (over 20 s on
        # Windows 11). A process that is merely there proves little: one that
        # cannot load a DLL sits behind Windows' error dialog, so the daemon
        # must answer opadctl too
        $t = 0
        do {
            $procs = @(Get-Process opad-gui, opad-daemon -ErrorAction SilentlyContinue | Where-Object { $_.SessionId -gt 0 })
            if ($procs.Count -ge 2) { break }
            Start-Sleep 3; $t += 3
        } while ($t -lt 90)
        foreach ($n in "opad-gui", "opad-daemon") {
            $p = $procs | Where-Object { $_.Name -eq $n }
            if ($p) { Pass "$n started at login (session $($p[0].SessionId))" } else { Fail "$n is not running in the desktop session 90 s after login" }
        }
        $t = 0
        do {
            $out = (& (Join-Path $app "opadctl.exe") status 2>&1) -join "`n"
            $code = $LASTEXITCODE
            if ($out -match "Daemon Mode") { break }
            Start-Sleep 3; $t += 3
        } while ($t -lt 30)
        if ($out -match "Daemon Mode") { Pass "opadctl status reaches the daemon" }
        else { Fail "opadctl status (exit $code): $out" }
        # Open the window the way a click on the Start menu entry would: a
        # process started over SSH has no desktop, a scheduled task for the
        # logged-on user does
        $task = "opad-lab-open-gui"
        Register-ScheduledTask -TaskName $task -Force `
            -Action (New-ScheduledTaskAction -Execute (Join-Path $app "opad-gui.exe")) `
            -Principal (New-ScheduledTaskPrincipal -UserId $env:USERNAME -LogonType Interactive) | Out-Null
        Start-ScheduledTask -TaskName $task
        Start-Sleep 12
        Unregister-ScheduledTask -TaskName $task -Confirm:$false
        if (Get-Process opad-gui -ErrorAction SilentlyContinue) { Pass "the GUI window was opened (see the screenshot)" }
        else { Fail "opad-gui is not running after opening it" }
        # The data an upgrade must keep and an uninstall must delete
        if (Test-Path $db) {
            $id = (Get-Item $db).CreationTimeUtc.Ticks
            $mark = Join-Path $HOME ".opad-lab-db"
            if (-not (Test-Path $mark)) { Set-Content $mark $id; Pass "the app wrote its data ($db)" }
            elseif ((Get-Content $mark).Trim() -eq "$id") { Pass "the upgrade kept the app's data ($db)" }
            else { Fail "the upgrade replaced $db" }
        } else { Fail "no $db after use" }
    }
    "remove" {
        $unins = Join-Path $app "unins000.exe"
        if (-not (Test-Path $unins)) { Fail "no uninstaller at $unins"; break }
        # The uninstaller copies itself to %TEMP% and returns at once
        Start-Process $unins -ArgumentList "/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART" -Wait
        $t = 0
        while ((Test-Path (Join-Path $app "opad-gui.exe")) -and $t -lt 120) { Start-Sleep 2; $t += 2 }
        Start-Sleep 5
        if (Test-Path (Join-Path $app "opad-gui.exe")) { Fail "files still there 120 s after the uninstaller" } else { Pass "removed" }
        $keys = Get-ItemProperty $run -ErrorAction SilentlyContinue
        if ($keys.'opad-daemon' -or $keys.'opad-gui') { Fail "autostart Run keys left behind" }
        foreach ($n in "opad-gui", "opad-daemon", "tosu") {
            if (Get-Process $n -ErrorAction SilentlyContinue) { Fail "$n still running after uninstall" }
        }
        foreach ($d in $app, (Join-Path $env:APPDATA "opad")) {
            if (Test-Path $d) { Fail "left behind: $d" }
        }
        foreach ($l in "$env:APPDATA\Microsoft\Windows\Start Menu\Programs\OPad.lnk", "$HOME\Desktop\OPad.lnk") {
            if (Test-Path $l) { Fail "left behind: $l" }
        }
        if (Get-InstalledVersion) { Fail "left behind: the Apps & features entry" }
        if (-not $script:failed) { Pass "nothing of OPad left" }
    }
}
if ($failed) { exit 1 }
