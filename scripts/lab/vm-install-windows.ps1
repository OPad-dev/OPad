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
#   update VERSION DIR
#                    after check, with the previous release installed: OPad
#                    updates itself to VERSION, automatically, from the signed
#                    feed in DIR (beta.json, its .minisig, and for local test
#                    builds the installer it names), served on this VM's
#                    loopback. OPad is restarted in the desktop session with
#                    OPAD_MANIFEST_URL pointing at it and its daemon asked
#                    over the named pipe, as the app's Updates page does, to
#                    follow beta. With automatic updates on (the default) it
#                    downloads and verifies the installer and leaves it for
#                    the next start: nothing is installed yet. The reboot
#                    lab-install.ps1 does next is what installs it.
#   remove           the uninstaller, silent; nothing of OPad is left: files,
#                    %APPDATA%\opad, Run keys, shortcuts, the Apps entry
param(
    [Parameter(Mandatory)][ValidateSet("install", "check", "update", "remove")][string]$Phase,
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
        # Not -Wait: in PowerShell 5.1 it also waits for the setup's children,
        # and the daemon the setup starts never exits
        $p = Start-Process $setup.FullName -PassThru `
            -ArgumentList "/SILENT", "/SUPPRESSMSGBOXES", "/NORESTART", "/LOG=$HOME\install-$Arg.log"
        $null = $p.Handle
        if (-not $p.WaitForExit(600 * 1000)) { $p.Kill(); Fail "installer still running after 600 s"; Get-Content "$HOME\install-$Arg.log" -Tail 15; break }
        if ($p.ExitCode -eq 0) { Pass "installed $(Get-InstalledVersion) ($($setup.Name))" }
        else { Fail "installer exited $($p.ExitCode)"; Get-Content "$HOME\install-$Arg.log" -Tail 15 }
        foreach ($f in "opad-gui.exe", "opad-daemon.exe", "opadctl.exe", "espflash.exe", "tosu\tosu.exe", "unins000.exe") {
            if (-not (Test-Path (Join-Path $app $f))) { Fail "missing $app\$f" }
        }
        $keys = Get-ItemProperty $run -ErrorAction SilentlyContinue
        if ($keys.'opad-daemon' -and $keys.'opad-gui') { Pass "autostart Run keys set" } else { Fail "autostart Run keys missing" }
    }
    "check" {
        # After an automatic update the installer runs at login: give it time
        $deadline = (Get-Date).AddSeconds(180)
        while ((Get-InstalledVersion) -ne $Arg -and (Get-Date) -lt $deadline) { Start-Sleep 5 }
        while ((Get-Process opad-update-* , opad-setup* -ErrorAction SilentlyContinue) -and (Get-Date) -lt $deadline) { Start-Sleep 2 }
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
        else {
            # NTSTATUS exit codes of a program Windows could not start
            $why = @{ -1073741515 = "0xC0000135: a DLL it needs is missing"; -1073741701 = "0xC000007B: a DLL of the wrong architecture" }[$code]
            Fail "opadctl status (exit $code$(if ($why) { ", $why" }))"
            if ($out) { Write-Output $out }
        }
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
    "update" {
        $want = $Arg
        $feed = Join-Path $HOME $Dir
        if (-not (Test-Path (Join-Path $feed "beta.json")) -or -not (Test-Path (Join-Path $feed "beta.json.minisig"))) {
            Fail "no signed feed in $feed"; break
        }
        $old = Get-InstalledVersion
        # Programs started from here would end with the SSH session: the feed
        # server and OPad run as tasks in the desktop session instead
        function Start-InSession([string]$Name, [string]$Exe, [string]$Arguments = "") {
            $action = if ($Arguments) { New-ScheduledTaskAction -Execute $Exe -Argument $Arguments } else { New-ScheduledTaskAction -Execute $Exe }
            Register-ScheduledTask -TaskName $Name -Force -Action $action `
                -Settings (New-ScheduledTaskSettingsSet -ExecutionTimeLimit (New-TimeSpan -Hours 2)) `
                -Principal (New-ScheduledTaskPrincipal -UserId $env:USERNAME -LogonType Interactive) | Out-Null
            Start-ScheduledTask -TaskName $Name
        }
        # The feed on loopback: the updater takes plain HTTP from there only
        $server = Join-Path $HOME "lab-feed-server.ps1"
        Set-Content $server -Encoding UTF8 -Value @'
param([string]$Dir)
$l = New-Object System.Net.Sockets.TcpListener ([System.Net.IPAddress]::Loopback), 8000
$l.Start()
while ($true) {
    $c = $l.AcceptTcpClient()
    try {
        $st = $c.GetStream()
        $r = New-Object System.IO.StreamReader $st
        $line = $r.ReadLine()
        while ($true) { $h = $r.ReadLine(); if (-not $h) { break } }
        $name = [System.IO.Path]::GetFileName((($line -split ' ')[1] -split '\?')[0])
        $f = Join-Path $Dir $name
        if ($name -and (Test-Path -LiteralPath $f -PathType Leaf)) {
            $b = [System.IO.File]::ReadAllBytes($f); $code = "200 OK"
        } else { $b = New-Object byte[] 0; $code = "404 Not Found" }
        $head = [System.Text.Encoding]::ASCII.GetBytes("HTTP/1.1 $code`r`nContent-Length: $($b.Length)`r`nContent-Type: application/octet-stream`r`nConnection: close`r`n`r`n")
        $st.Write($head, 0, $head.Length)
        if ($b.Length) { $st.Write($b, 0, $b.Length) }
        $st.Flush()
    } catch { } finally { $c.Close() }
}
'@
        Start-InSession "opad-lab-feed" "powershell.exe" "-NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File `"$server`" -Dir `"$feed`""
        # OPad again, reading it: a task takes the user's environment from the
        # registry. Both programs, so the GUI cannot start a daemon without it
        [Environment]::SetEnvironmentVariable("OPAD_MANIFEST_URL", "http://127.0.0.1:8000/beta.json", "User")
        # The lab's feeds are signed with the public test key, which OPad
        # trusts only where asked to (opad-update/src/verify.rs)
        [Environment]::SetEnvironmentVariable("OPAD_TRUST_TEST_KEY", "1", "User")
        Stop-Process -Name opad-gui, opad-daemon -Force -ErrorAction SilentlyContinue
        Start-Sleep 2
        Start-InSession "opad-lab-daemon" (Join-Path $app "opad-daemon.exe")
        Start-Sleep 3
        Start-InSession "opad-lab-gui" (Join-Path $app "opad-gui.exe") "--tray"

        # The daemon's IPC (opad-ipc: a little-endian u32 length, then JSON), as
        # the GUI's Updates page uses it: follow beta, then watch it update itself
        $pipe = "opad-ipc-" + [System.Security.Principal.WindowsIdentity]::GetCurrent().User.Value
        function Connect-Daemon {
            $p = New-Object System.IO.Pipes.NamedPipeClientStream ".", $pipe, ([System.IO.Pipes.PipeDirection]::InOut)
            $p.Connect(60000)
            return $p
        }
        function Invoke-Ipc($p, [string]$Json) {
            $data = [System.Text.Encoding]::UTF8.GetBytes($Json)
            $p.Write([BitConverter]::GetBytes([uint32]$data.Length), 0, 4)
            $p.Write($data, 0, $data.Length); $p.Flush()
            $read = { param([int]$n) $b = New-Object byte[] $n; $o = 0
                while ($o -lt $n) { $k = $p.Read($b, $o, $n - $o); if ($k -le 0) { throw "the daemon closed the pipe" }; $o += $k }; ,$b }
            $len = [BitConverter]::ToUInt32((& $read 4), 0)
            return [System.Text.Encoding]::UTF8.GetString((& $read ([int]$len)))
        }
        try {
            # The daemon accepts a client of its own version only: it names it
            $p = Connect-Daemon
            $r = Invoke-Ipc $p '{"Handshake":{"client_version":"0","client_protocol":1}}'
            $p.Dispose()
            $running = if ($r -match 'running daemon is (\S+?)\.? ') { $Matches[1] } else { "0" }
            Write-Output "info: the running daemon is $running"
            $p = Connect-Daemon
            $r = Invoke-Ipc $p ('{"Handshake":{"client_version":"' + $running + '","client_protocol":1}}')
            if ($r -notmatch 'HandshakeAck') { throw "the daemon refused the handshake: $r" }
            $r = Invoke-Ipc $p '{"SetUpdateChannel":{"channel":"beta"}}'
            if ($r -ne '"UpdateCheckStarted"') { throw "following the beta channel: $r" }
            $deadline = (Get-Date).AddSeconds(900); $seen = $false
            while ($true) {
                Start-Sleep 2
                $st = ((Invoke-Ipc $p '"GetUpdateStatus"') | ConvertFrom-Json).UpdateStatus
                if ($st.app.available -eq $want -and -not $seen) { $seen = $true; Pass "the updater found $want in the signed feed" }
                if ($st.restart_required -and $st.pending_version -eq $want) { break }
                if ((Get-Date) -gt $deadline) { throw "$want was not ready within 900 s of following beta (last error: $($st.last_error))" }
            }
            Pass "it downloaded and verified $want by itself, to install at the next start"
            if (-not $st.app.enabled) { Fail "automatic updates were off" }
            $p.Dispose()
        } catch {
            Fail "updating itself: $_"
            $log = Join-Path $env:APPDATA "opad\daemon.log"
            if (Test-Path $log) { Write-Output "  daemon log:"; Get-Content $log -Tail 40 | ForEach-Object { "  $_" } }
        }
        if ((Get-InstalledVersion) -eq $old) { Pass "nothing installed yet: OPad $old keeps running until the next start" }
        else { Fail "OPad was installed while running ($(Get-InstalledVersion))" }
        foreach ($t in "opad-lab-feed", "opad-lab-daemon", "opad-lab-gui") {
            Unregister-ScheduledTask -TaskName $t -Confirm:$false -ErrorAction SilentlyContinue
        }
        Get-CimInstance Win32_Process -Filter "Name='powershell.exe'" | Where-Object { $_.CommandLine -like "*lab-feed-server*" } |
            ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }
        [Environment]::SetEnvironmentVariable("OPAD_MANIFEST_URL", $null, "User")
        [Environment]::SetEnvironmentVariable("OPAD_TRUST_TEST_KEY", $null, "User")
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
