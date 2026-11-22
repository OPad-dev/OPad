# Tests an app release's installers in the lab VMs, on the self-hosted runner
# (the Windows 11 mini PC). It downloads the release's assets (what a user
# downloads) and checks them against SHA256SUMS. Then in each VM, from its
# clean "limpia" snapshot, with the package for that system (.deb, .rpm, the
# AUR opad-bin PKGBUILD, the Windows installer):
#
#   1. installs the previous app release, reboots, checks OPad starts at
#      login, its window opens and it wrote its data;
#   2. upgrades to the release under test the way the in-app updater does,
#      reboots, checks the same, the new version, and that the data was kept;
#   3. uninstalls it and checks nothing of OPad is left.
#
# Screenshots of the window after each step. With -From none (or no earlier
# release) step 1 is skipped. The VM goes back to "limpia" afterwards. Run by
# .github/workflows/lab-install.yml, also for every app release.
#
#   powershell -File scripts/lab/lab-install.ps1 [-Tag v1.0.0] [-From v1.0.0-rc.3|none] [-Vms arch,windows-11] [-OutDir out]

param(
    [string]$Tag = "",
    [string]$From = "",
    [string[]]$Vms = @(),
    [string]$OutDir = "lab-install"
)

# Continue: VBoxManage and ssh print progress on stderr, which Windows
# PowerShell 5.1 would turn into a terminating error; exit codes are checked
$ErrorActionPreference = "Continue"
$ProgressPreference = "SilentlyContinue"  # Invoke-WebRequest is slow with it
. (Join-Path $PSScriptRoot "lab-common.ps1")
$Repo = "OPad-dev/OPad"

New-Item -ItemType Directory -Force $OutDir | Out-Null
$OutDir = (Resolve-Path $OutDir).Path
$failures = New-Object System.Collections.Generic.List[string]
function Fail([string]$what) { Write-Host "FAIL: $what"; $failures.Add($what) }
function Pass([string]$what) { Write-Host "ok:   $what" }

$headers = @{ "User-Agent" = "opad-lab" }
if ($env:GITHUB_TOKEN) { $headers["Authorization"] = "Bearer $env:GITHUB_TOKEN" }
# GitHub answers 502/504 now and then
function Invoke-WithRetry([scriptblock]$Do) {
    foreach ($try in 1..4) {
        try { return & $Do } catch {
            if ($try -eq 4) { throw }
            Write-Host "    GitHub: $($_.Exception.Message), retrying"; Start-Sleep (10 * $try)
        }
    }
}

# --- The releases: the newest app release (pre-releases too) unless -Tag,
# upgraded to from the one before it unless -From
$releases = Invoke-WithRetry { Invoke-RestMethod -Headers $headers "https://api.github.com/repos/$Repo/releases?per_page=50" }
$appTags = @($releases | Where-Object { $_.tag_name -match '^v\d' -and -not $_.draft } | ForEach-Object { $_.tag_name })
if (-not $Tag) { $Tag = $appTags | Select-Object -First 1 }
if (-not $Tag) { Write-Host "FAIL: no app release found"; exit 1 }
if (-not $From) {
    $i = [array]::IndexOf($appTags, $Tag)
    $From = if ($i -ge 0 -and $i + 1 -lt $appTags.Count) { $appTags[$i + 1] } else { "none" }
}
if ($From -eq $Tag) { $From = "none" }
Write-Host "Release $Tag$(if ($From -ne 'none') { ", upgraded to from $From" })"

# Each release's files in packages\<tag>, checked against its SHA256SUMS
function Get-Release([string]$T) {
    $v = $T.TrimStart("v")
    $dir = Join-Path $OutDir "packages\$T"
    New-Item -ItemType Directory -Force $dir | Out-Null
    $files = @{
        deb = "opad_${v}_amd64.deb"; rpm = "opad-${v}.x86_64.rpm"
        aur = "opad-bin.PKGBUILD"; win = "opad-setup-${v}.exe"
    }
    foreach ($name in @($files.Values) + "SHA256SUMS") {
        $u = "https://github.com/$Repo/releases/download/$T/$name"
        Invoke-WithRetry { Invoke-WebRequest -UseBasicParsing -Headers $headers -OutFile (Join-Path $dir $name) $u } | Out-Null
    }
    # The PKGBUILD's own source: its install hooks, from the tagged tree
    $u = "https://raw.githubusercontent.com/$Repo/$T/packaging/linux/arch/opad-bin.install"
    Invoke-WithRetry { Invoke-WebRequest -UseBasicParsing -Headers $headers -OutFile (Join-Path $dir "opad-bin.install") $u } | Out-Null
    foreach ($line in Get-Content (Join-Path $dir "SHA256SUMS")) {
        $sum, $file = $line -split '\s+\*?', 2
        if ($files.Values -notcontains $file) { continue }
        if ((Get-FileHash (Join-Path $dir $file) -Algorithm SHA256).Hash -eq $sum.ToUpper()) { Pass "SHA256 matches: $T $file" }
        else { Fail "SHA256 mismatch: $T $file" }
    }
    return @{ Tag = $T; Version = $v; Dir = $dir; Files = $files }
}
try {
    $new = Get-Release $Tag
    $old = if ($From -ne "none") { Get-Release $From } else { $null }
} catch { Write-Host "FAIL: download: $_"; exit 1 }
if ($failures.Count -gt 0) { exit 1 }

# The files of one release that go into a VM
function Get-VmFiles([string]$Vm, $Rel) {
    $f = $Rel.Files
    switch -Wildcard ($Vm) {
        "ubuntu-*"  { return @(Join-Path $Rel.Dir $f.deb) }
        "fedora-*"  { return @(Join-Path $Rel.Dir $f.rpm) }
        "arch"      { return @((Join-Path $Rel.Dir $f.aur), (Join-Path $Rel.Dir "opad-bin.install")) }
        "windows-*" { return @(Join-Path $Rel.Dir $f.win) }
    }
}
function Invoke-Phase([string]$Vm, [string]$Phase, [string]$Arg, [int]$TimeoutSec) {
    $cmd = if (Test-WindowsVm $Vm) {
        "powershell -NoProfile -ExecutionPolicy Bypass -File vm-install-windows.ps1 $Phase $Arg"
    } else { "bash vm-install-linux.sh $Phase $Arg" }
    $r = Invoke-VmSsh $AllVms[$Vm] $cmd $TimeoutSec
    $r.Out -split "`r?`n" | Where-Object { $_ } | ForEach-Object { Write-Host "    $_" }
    Add-Content (Join-Path $OutDir "$Vm.log") "== $Phase $Arg (exit $($r.Code))`n$($r.Out)"
    return $r.Code -eq 0
}
# Reboot, so the login starts OPad the way it does for a user
function Restart-Vm([string]$Vm) {
    $reboot = if (Test-WindowsVm $Vm) { "Restart-Computer -Force" } else { "sudo systemctl reboot" }
    $null = Invoke-VmSsh $AllVms[$Vm] $reboot
    Start-Sleep 20
    if (-not (Wait-VmReady $Vm)) { throw "no logged-in desktop within 300 s after the reboot" }
}
# Install (or upgrade to) one release, reboot, check it; $false if a check failed
function Test-Release([string]$Vm, [string]$Sub, $Rel) {
    if (-not (Invoke-Phase $Vm "install" $Sub 1800)) { throw "installing $($Rel.Tag) failed" }
    Restart-Vm $Vm
    $ok = Invoke-Phase $Vm "check" $Rel.Version 300
    Start-Sleep 3
    Save-VmScreenshot $Vm (Join-Path $OutDir "$Vm-$($Rel.Tag).png")
    if (-not $ok) { Fail "${Vm}: $($Rel.Tag) after login (see $Vm-$($Rel.Tag).png and $Vm.log)" }
    return $ok
}

# --- Each VM
if ($Vms.Count -eq 0) { $Vms = @($AllVms.Keys) }
foreach ($vm in $Vms) {
    Write-Host ""
    Write-Host "== $vm"
    if (-not $AllVms.Contains($vm)) { Fail "unknown VM $vm"; continue }
    if (Test-VmRunning $vm) { Fail "$vm is already running; left alone"; continue }
    $port = $AllVms[$vm]
    try {
        Start-LabVm $vm
        $os = Wait-VmReady $vm
        if (-not $os) { throw "no logged-in desktop within 300 s" }
        Write-Host "    $os"
        $script = if (Test-WindowsVm $vm) { "vm-install-windows.ps1" } else { "vm-install-linux.sh" }
        $mk = if (Test-WindowsVm $vm) { "New-Item -ItemType Directory -Force old,new | Out-Null" } else { "mkdir -p old new" }
        $null = Invoke-VmSsh $port $mk
        $c = Copy-ToVm $port @(Join-Path $PSScriptRoot $script)
        if ($c.Code -eq 0 -and $old) { $c = Copy-ToVm $port (Get-VmFiles $vm $old) "old/" }
        if ($c.Code -eq 0) { $c = Copy-ToVm $port (Get-VmFiles $vm $new) "new/" }
        if ($c.Code -ne 0) { throw "copy into the VM: $($c.Out)" }

        $ok = $true
        if ($old) { $ok = (Test-Release $vm "old" $old) -and $ok }
        $ok = (Test-Release $vm "new" $new) -and $ok
        if (-not (Invoke-Phase $vm "remove" "" 600)) { throw "uninstall left something behind or failed" }
        if ($ok) { Pass "${vm}: $(if ($old) { "$($old.Tag) -> " })$($new.Tag): installs, starts at login, opens, $(if ($old) { 'upgrades keeping its data, ' })uninstalls leaving nothing" }
    } catch { Fail "${vm}: $_" }
    finally { Stop-LabVm $vm }
}

$free = [math]::Round((Get-PSDrive C).Free / 1GB)
Write-Host ""
Write-Host "C: has $free GB free"
if ($failures.Count -gt 0) { Write-Host "$($failures.Count) check(s) failed"; exit 1 }
Write-Host "Installers OK ($(if ($old) { "$($old.Tag) -> " })$($new.Tag))"
