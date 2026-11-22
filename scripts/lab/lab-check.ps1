# Checks the test lab on the self-hosted runner (the Windows 11 mini PC):
# the runner sits in the logged-on desktop session, osu! and the tosu replay
# fixture are in place, and every VirtualBox VM boots from its "limpia"
# snapshot to a logged-in desktop that answers SSH (Linux and Windows). Each VM is put back to
# "limpia" and powered off afterwards. Run by .github/workflows/lab-check.yml.
#
#   powershell -File scripts/lab/lab-check.ps1 [-Vms ubuntu-24.04,arch] [-OutDir out]

param(
    [string[]]$Vms = @(),
    [string]$OutDir = "lab-check"
)

# Continue: VBoxManage and ssh print progress on stderr, which Windows
# PowerShell 5.1 would turn into a terminating error; exit codes are checked
$ErrorActionPreference = "Continue"
$VBox = "C:\Program Files\Oracle\VirtualBox\VBoxManage.exe"
$Lab = "C:\opad-lab"
# VM name -> the host port VirtualBox forwards to its SSH
$AllVms = [ordered]@{
    "ubuntu-22.04" = 2222
    "ubuntu-24.04" = 2224
    "ubuntu-26.04" = 2226
    "fedora-43"    = 2243
    "arch"         = 2250
    "windows-10"   = 2310
    "windows-11"   = 2311
}
# The desktop is up when the user's session sits on seat0 (Linux) or on the
# console (Windows), and the account can become root/administrator without a
# prompt. The Windows VMs run PowerShell as their SSH shell
$LinuxReady = "loginctl list-sessions --no-legend | grep -q 'opad *seat0' && . /etc/os-release && echo `$PRETTY_NAME && sudo -n true && echo sudo-ok"
$WindowsReady = 'if ((quser 2>$null) -match ''opad\s+console\s+\d+\s+Active'') { (Get-CimInstance Win32_OperatingSystem).Caption; if (([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) { ''sudo-ok'' } } else { exit 1 }'
$SshOpts = @("-o", "BatchMode=yes", "-o", "ConnectTimeout=5", "-o", "StrictHostKeyChecking=no",
             "-o", "UserKnownHostsFile=NUL", "-o", "LogLevel=ERROR")

# One ssh command with a hard time limit: while a VM boots, VirtualBox's port
# forward accepts the connection before the guest's sshd answers, and ssh
# then waits forever
function Invoke-VmSsh([int]$Port, [string]$Command, [int]$TimeoutSec = 20) {
    # Through files: ssh.exe hangs when PowerShell pipes its output
    $out = [System.IO.Path]::GetTempFileName()
    $err = [System.IO.Path]::GetTempFileName()
    $argLine = (($SshOpts + @("-n", "-p", "$Port", "opad@127.0.0.1")) -join " ") + " `"$Command`""
    $proc = Start-Process ssh.exe -ArgumentList $argLine -NoNewWindow -PassThru `
        -RedirectStandardOutput $out -RedirectStandardError $err
    $null = $proc.Handle  # without it ExitCode stays empty
    if (-not $proc.WaitForExit($TimeoutSec * 1000)) { $proc.Kill(); $code = -1 } else { $code = $proc.ExitCode }
    $text = (Get-Content $out -Raw -ErrorAction SilentlyContinue)
    Remove-Item $out, $err -ErrorAction SilentlyContinue
    return @{ Code = $code; Out = "$text".Trim() }
}

New-Item -ItemType Directory -Force $OutDir | Out-Null
$failures = New-Object System.Collections.Generic.List[string]
function Fail([string]$what) { Write-Host "FAIL: $what"; $failures.Add($what) }
function Pass([string]$what) { Write-Host "ok:   $what" }

# --- The desktop session: osu! and the screenshots need one, session 0 has none
$session = (Get-Process -Id $PID).SessionId
if ($session -gt 0) { Pass "runner runs in desktop session $session" } else { Fail "runner runs in session 0 (a service), not on the desktop" }
try {
    Add-Type -AssemblyName System.Windows.Forms, System.Drawing
    $b = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
    $bmp = New-Object System.Drawing.Bitmap $b.Width, $b.Height
    [System.Drawing.Graphics]::FromImage($bmp).CopyFromScreen($b.Location, [System.Drawing.Point]::Empty, $b.Size)
    $bmp.Save((Join-Path $OutDir "host-desktop.png"))
    Pass "desktop screenshot ($($b.Width)x$($b.Height))"
} catch { Fail "desktop screenshot: $_" }

# --- osu! and the tosu acceptance fixture
foreach ($p in @("$env:LOCALAPPDATA\osu!\osu!.exe", "$env:LOCALAPPDATA\osulazer\current\osu!.exe")) {
    if (Test-Path -LiteralPath $p) { Pass "installed: $p" } else { Fail "missing: $p" }
}
$expected = Join-Path $Lab "replays\expected.json"
if (Test-Path $expected) {
    $e = Get-Content $expected -Raw | ConvertFrom-Json
    $osr = Join-Path $Lab "replays\$($e.replay)"
    if (Test-Path -LiteralPath $osr) { Pass "replay fixture: $($e.map), $($e.n300)/$($e.n100)/$($e.n50)/$($e.miss), combo $($e.max_combo)" }
    else { Fail "replay file named in expected.json is missing" }
    $osu = Get-ChildItem -LiteralPath (Join-Path $Lab "replays\map") -Filter *.osu -ErrorAction SilentlyContinue |
        Where-Object { (Get-FileHash -LiteralPath $_.FullName -Algorithm MD5).Hash.ToLower() -eq $e.beatmap_md5 }
    if ($osu) { Pass "map matches the replay: $($osu.Name)" } else { Fail "no .osu in replays\map matches the replay's beatmap MD5" }
} else { Fail "missing $expected" }

# --- The VMs
if ($Vms.Count -eq 0) { $Vms = @($AllVms.Keys) }
$running = (& $VBox list runningvms) -join "`n"
foreach ($vm in $Vms) {
    if (-not $AllVms.Contains($vm)) { Fail "unknown VM $vm"; continue }
    if ($running -match "`"$vm`"") { Fail "$vm is already running; left alone"; continue }
    $port = $AllVms[$vm]
    try {
        & $VBox snapshot $vm restore limpia 2>&1 | Out-Null
        if ($LASTEXITCODE -ne 0) { throw "cannot restore snapshot limpia" }
        & $VBox startvm $vm --type headless 2>&1 | Out-Null
        if ($LASTEXITCODE -ne 0) { throw "cannot start" }
        $ok = $false
        $clock = [System.Diagnostics.Stopwatch]::StartNew()
        while ($clock.Elapsed.TotalSeconds -lt 300) {
            $r = Invoke-VmSsh $port $(if ($vm -like "windows-*") { $WindowsReady } else { $LinuxReady })
            if ($r.Code -eq 0 -and $r.Out -match "sudo-ok") { $ok = $true; $out = @($r.Out -split "`n"); break }
            Start-Sleep 5
        }
        if ($ok) {
            Start-Sleep 10
            & $VBox controlvm $vm screenshotpng (Join-Path (Resolve-Path $OutDir) "$vm.png") 2>&1 | Out-Null
            Pass "$vm boots to a logged-in desktop in $([int]$clock.Elapsed.TotalSeconds) s: $($out[0])"
        } else { Fail "${vm}: no logged-in desktop over SSH within 300 s" }
    } catch { Fail "${vm}: $_" }
    finally {
        & $VBox controlvm $vm poweroff 2>&1 | Out-Null
        Start-Sleep 3
        & $VBox snapshot $vm restore limpia 2>&1 | Out-Null
    }
}

$free = [math]::Round((Get-PSDrive C).Free / 1GB)
if ($free -ge 30) { Pass "C: has $free GB free" } else { Fail "C: has only $free GB free" }

Write-Host ""
if ($failures.Count -gt 0) { Write-Host "$($failures.Count) check(s) failed"; exit 1 }
Write-Host "Lab OK"
