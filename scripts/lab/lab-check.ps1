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
. (Join-Path $PSScriptRoot "lab-common.ps1")

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
foreach ($vm in $Vms) {
    if (-not $AllVms.Contains($vm)) { Fail "unknown VM $vm"; continue }
    if (Test-VmRunning $vm) { Fail "$vm is already running; left alone"; continue }
    try {
        Start-LabVm $vm
        $clock = [System.Diagnostics.Stopwatch]::StartNew()
        $os = Wait-VmReady $vm
        if ($os) {
            Start-Sleep 10
            Save-VmScreenshot $vm (Join-Path (Resolve-Path $OutDir) "$vm.png")
            Pass "$vm boots to a logged-in desktop in $([int]$clock.Elapsed.TotalSeconds) s: $os"
        } else { Fail "${vm}: no logged-in desktop over SSH within 300 s" }
    } catch { Fail "${vm}: $_" }
    finally { Stop-LabVm $vm }
}

$free = [math]::Round((Get-PSDrive C).Free / 1GB)
if ($free -ge 30) { Pass "C: has $free GB free" } else { Fail "C: has only $free GB free" }

Write-Host ""
if ($failures.Count -gt 0) { Write-Host "$($failures.Count) check(s) failed"; exit 1 }
Write-Host "Lab OK"
