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
# Each installed program is checked against the release's FILES.sha256 (the
# hash of every file each package installs), when the release has one.
# Results go to the lab dashboard (lab-results.ps1).
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
New-LabRun "install" "Installers"
Use-LabGroup "release" "Release files"

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
if (-not $Tag) { Fail "no app release found"; Complete-LabRun; exit 1 }
if (-not $From) {
    $i = [array]::IndexOf($appTags, $Tag)
    $From = if ($i -ge 0 -and $i + 1 -lt $appTags.Count) { $appTags[$i + 1] } else { "none" }
}
if ($From -eq $Tag) { $From = "none" }
Write-Host "Release $Tag$(if ($From -ne 'none') { ", upgraded to from $From" })"
Set-LabRelease $Tag $From
Set-LabTitle "Installers $(if ($From -ne 'none') { "$From -> " })$Tag"

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
    # The hash of every file each package installs (releases after rc.3)
    try {
        Invoke-WebRequest -UseBasicParsing -Headers $headers -OutFile (Join-Path $dir "FILES.sha256") `
            "https://github.com/$Repo/releases/download/$T/FILES.sha256" | Out-Null
    } catch { Remove-Item (Join-Path $dir "FILES.sha256") -ErrorAction SilentlyContinue }
    # The PKGBUILD's own source: its install hooks, from the tagged tree
    $u = "https://raw.githubusercontent.com/$Repo/$T/packaging/linux/arch/opad-bin.install"
    Invoke-WithRetry { Invoke-WebRequest -UseBasicParsing -Headers $headers -OutFile (Join-Path $dir "opad-bin.install") $u } | Out-Null
    foreach ($line in Get-Content (Join-Path $dir "SHA256SUMS")) {
        $sum, $file = $line -split '\s+\*?', 2
        if (@($files.Values) + "FILES.sha256" -notcontains $file) { continue }
        if ((Get-FileHash (Join-Path $dir $file) -Algorithm SHA256).Hash -eq $sum.ToUpper()) { Pass "SHA256 matches: $T $file" }
        else { Fail "SHA256 mismatch: $T $file" }
    }
    return @{ Tag = $T; Version = $v; Dir = $dir; Files = $files }
}
try {
    $new = Get-Release $Tag
    $old = if ($From -ne "none") { Get-Release $From } else { $null }
} catch { Fail "download" "$_"; Complete-LabRun; exit 1 }
foreach ($r in @($new, $old) | Where-Object { $_ }) {
    if (-not (Test-Path (Join-Path $r.Dir "FILES.sha256"))) { Skip "$($r.Tag) has no FILES.sha256: its installed files cannot be checked" }
}
Complete-LabGroup
if ((Get-LabFailures) -gt 0) { Complete-LabRun; exit 1 }

# The files of one release that go into a VM
function Get-VmFiles([string]$Vm, $Rel) {
    $f = $Rel.Files
    $list = switch -Wildcard ($Vm) {
        "ubuntu-*"  { @(Join-Path $Rel.Dir $f.deb) }
        "fedora-*"  { @(Join-Path $Rel.Dir $f.rpm) }
        "arch"      { @((Join-Path $Rel.Dir $f.aur), (Join-Path $Rel.Dir "opad-bin.install")) }
        "windows-*" { @(Join-Path $Rel.Dir $f.win) }
    }
    $sums = Join-Path $Rel.Dir "FILES.sha256"
    if (Test-Path $sums) { $list += $sums }
    return $list
}
function Invoke-Phase([string]$Vm, [string]$Phase, [string]$Arg, [string]$Dir, [int]$TimeoutSec, [string]$Prefix = "", [switch]$Previous) {
    $cmd = if (Test-WindowsVm $Vm) {
        "powershell -NoProfile -ExecutionPolicy Bypass -File vm-install-windows.ps1 $Phase $Arg $Dir"
    } else { "bash vm-install-linux.sh $Phase $Arg $Dir" }
    $r = Invoke-VmSsh $AllVms[$Vm] $cmd $TimeoutSec
    $r.Out -split "`r?`n" | Where-Object { $_ } | ForEach-Object { Write-Host "    $_" }
    $log = Join-Path $OutDir "$Vm.log"
    Add-Content $log "== $Phase $Arg $Dir (exit $($r.Code))`n$($r.Out)"
    Add-LabOutput $r.Out $Prefix -Previous:$Previous
    if ($r.Code -eq -1) {
        if ($Previous) { Skip "$Prefix$Phase timed out after $TimeoutSec s" } else { Fail "$Prefix$Phase timed out after $TimeoutSec s" }
    }
    Add-LabFile $log log "Everything the VM printed"
    return $r.Code -eq 0
}
# Reboot, so the login starts OPad the way it does for a user
function Restart-Vm([string]$Vm) {
    $reboot = if (Test-WindowsVm $Vm) { "Restart-Computer -Force" } else { "sudo systemctl reboot" }
    $null = Invoke-VmSsh $AllVms[$Vm] $reboot
    Start-Sleep 20
    if (-not (Wait-VmReady $Vm)) { throw "no logged-in desktop within 300 s after the reboot" }
}
# Install (or upgrade to) one release, reboot, check it. The previous
# release's checks are prefixed with its tag and cannot fail the run
function Test-Release([string]$Vm, [string]$Sub, $Rel, [switch]$Previous) {
    $prefix = "$($Rel.Tag): "
    if (-not (Invoke-Phase $Vm "install" $Sub "" 1800 $prefix -Previous:$Previous)) {
        if ($Previous) { return }
        throw "installing $($Rel.Tag) failed"
    }
    Restart-Vm $Vm
    $null = Invoke-Phase $Vm "check" $Rel.Version $Sub 300 $prefix -Previous:$Previous
    Start-Sleep 3
    $shot = Join-Path $OutDir "$Vm-$($Rel.Tag).png"
    Save-VmScreenshot $Vm $shot
    Add-LabFile $shot screenshot "$($Rel.Tag): the window after login"
}

# --- Each VM
if ($Vms.Count -eq 0) { $Vms = @($AllVms.Keys) }
foreach ($vm in $Vms) {
    Write-Host ""
    Write-Host "== $vm"
    Use-LabGroup $vm $vm
    if (-not $AllVms.Contains($vm)) { Fail "unknown VM $vm"; Complete-LabGroup; continue }
    # Left on by a cancelled run: the lab's VMs are only ever used from "limpia"
    if (Test-VmRunning $vm) { Stop-LabVm $vm; Add-LabCheck info "was left running (a cancelled run?): powered off and restored to limpia" }
    $port = $AllVms[$vm]
    try {
        Start-LabVm $vm
        $os = Wait-VmReady $vm
        if (-not $os) { throw "no logged-in desktop within 300 s" }
        Write-Host "    $os"
        Set-LabGroupOs $os
        $script = if (Test-WindowsVm $vm) { "vm-install-windows.ps1" } else { "vm-install-linux.sh" }
        $mk = if (Test-WindowsVm $vm) { "New-Item -ItemType Directory -Force old,new | Out-Null" } else { "mkdir -p old new" }
        $null = Invoke-VmSsh $port $mk
        $c = Copy-ToVm $port @(Join-Path $PSScriptRoot $script)
        if ($c.Code -eq 0 -and $old) { $c = Copy-ToVm $port (Get-VmFiles $vm $old) "old/" }
        if ($c.Code -eq 0) { $c = Copy-ToVm $port (Get-VmFiles $vm $new) "new/" }
        if ($c.Code -ne 0) { throw "copy into the VM: $($c.Out)" }

        if ($old) { Test-Release $vm "old" $old -Previous }
        Test-Release $vm "new" $new
        $null = Invoke-Phase $vm "remove" "" "" 600 "$($new.Tag): "
    } catch { Fail "$_" }
    finally { Stop-LabVm $vm; Complete-LabGroup }
}
Complete-LabRun

$free = [math]::Round((Get-PSDrive C).Free / 1GB)
Write-Host ""
Write-Host "C: has $free GB free"
if ((Get-LabFailures) -gt 0) { Write-Host "$(Get-LabFailures) check(s) failed"; exit 1 }
Write-Host "Installers OK ($(if ($old) { "$($old.Tag) -> " })$($new.Tag))"
