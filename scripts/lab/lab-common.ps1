# Shared by the lab scripts on the self-hosted runner (the Windows 11 mini PC):
# the VirtualBox VMs, SSH/SCP into them with a time limit, and booting one
# from its "limpia" snapshot until its desktop is logged in, and recording
# the run's results for the lab dashboard (lab-results.ps1). Dot-source it.

. (Join-Path $PSScriptRoot "lab-results.ps1")

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

function Test-WindowsVm([string]$Vm) { return $Vm -like "windows-*" }

# One native command with a hard time limit, its output through files:
# ssh.exe hangs when PowerShell pipes its output, and while a VM boots
# VirtualBox's port forward accepts the connection before the guest's sshd
# answers, so ssh would otherwise wait forever
function Invoke-Native([string]$Exe, [string]$ArgLine, [int]$TimeoutSec) {
    $out = [System.IO.Path]::GetTempFileName()
    $err = [System.IO.Path]::GetTempFileName()
    $proc = Start-Process $Exe -ArgumentList $ArgLine -NoNewWindow -PassThru `
        -RedirectStandardOutput $out -RedirectStandardError $err
    $null = $proc.Handle  # without it ExitCode stays empty
    if (-not $proc.WaitForExit($TimeoutSec * 1000)) { $proc.Kill(); $code = -1 } else { $code = $proc.ExitCode }
    $text = "$(Get-Content $out -Raw -ErrorAction SilentlyContinue)$(Get-Content $err -Raw -ErrorAction SilentlyContinue)"
    Remove-Item $out, $err -ErrorAction SilentlyContinue
    return @{ Code = $code; Out = $text.Trim() }
}

# $Command must not contain double quotes
function Invoke-VmSsh([int]$Port, [string]$Command, [int]$TimeoutSec = 20) {
    $argLine = (($SshOpts + @("-n", "-p", "$Port", "opad@127.0.0.1")) -join " ") + " `"$Command`""
    return Invoke-Native ssh.exe $argLine $TimeoutSec
}

# Copies host files into the VM user's home directory, or a directory in it
# ending in "/" that exists
function Copy-ToVm([int]$Port, [string[]]$Files, [string]$Dest = "", [int]$TimeoutSec = 300) {
    $quoted = $Files | ForEach-Object { "`"$_`"" }
    $argLine = (($SshOpts + @("-P", "$Port")) -join " ") + " " + ($quoted -join " ") + " opad@127.0.0.1:$Dest"
    return Invoke-Native scp.exe $argLine $TimeoutSec
}

# Polls until the VM's desktop is logged in; returns the OS name, or $null
function Wait-VmReady([string]$Vm, [int]$TimeoutSec = 300) {
    $port = $AllVms[$Vm]
    $ready = if (Test-WindowsVm $Vm) { $WindowsReady } else { $LinuxReady }
    $clock = [System.Diagnostics.Stopwatch]::StartNew()
    while ($clock.Elapsed.TotalSeconds -lt $TimeoutSec) {
        $r = Invoke-VmSsh $port $ready
        if ($r.Code -eq 0 -and $r.Out -match "sudo-ok") { return @($r.Out -split "`n")[0].Trim() }
        Start-Sleep 5
    }
    return $null
}

function Start-LabVm([string]$Vm) {
    & $VBox snapshot $Vm restore limpia 2>&1 | Out-Null
    if ($LASTEXITCODE -ne 0) { throw "cannot restore snapshot limpia" }
    & $VBox startvm $Vm --type headless 2>&1 | Out-Null
    if ($LASTEXITCODE -ne 0) { throw "cannot start" }
}

# Powers the VM off and puts it back to its clean snapshot
function Stop-LabVm([string]$Vm) {
    & $VBox controlvm $Vm poweroff 2>&1 | Out-Null
    Start-Sleep 3
    & $VBox snapshot $Vm restore limpia 2>&1 | Out-Null
}

function Save-VmScreenshot([string]$Vm, [string]$Path) {
    & $VBox controlvm $Vm screenshotpng $Path 2>&1 | Out-Null
}

function Test-VmRunning([string]$Vm) {
    return [bool]((& $VBox list runningvms) -match "`"$Vm`"")
}
