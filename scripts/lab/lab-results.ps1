# Records a lab run's results for the lab dashboard (the private
# OPad-dev/opad-lab-dashboard repo, which reads them; its
# docs/results-format.md is the format, schema 1). Dot-sourced by
# lab-common.ps1.
#
#   <results>\runs\<id>\run.json      the run: groups of checks
#   <results>\runs\<id>\<files>       its screenshots and logs
#
# <results> is $env:OPAD_LAB_RESULTS, or C:\opad-lab\results. run.json is
# rewritten after every check, so the dashboard shows a run while it goes.
# Only the newest $LabRunsKept runs are kept.

$LabResults = if ($env:OPAD_LAB_RESULTS) { $env:OPAD_LAB_RESULTS } else { "C:\opad-lab\results" }
$LabRunsKept = 30
$script:LabRun = $null
$script:LabRunDir = $null
$script:LabGroup = $null

function New-LabRun([string]$Kind, [string]$Title) {
    $runs = Join-Path $LabResults "runs"
    New-Item -ItemType Directory -Force $runs | Out-Null
    # One lab job runs at a time (the opad-lab concurrency group): a run still
    # "running" was cancelled or its runner stopped. Mark it so
    foreach ($old in Get-ChildItem $runs -Directory) {
        $f = Join-Path $old.FullName "run.json"
        if (-not (Test-Path $f)) { continue }
        try {
            $r = [System.IO.File]::ReadAllText($f) | ConvertFrom-Json
            if ($r.status -ne "running") { continue }
            $r.status = "fail"
            $r.summary.fail = [int]$r.summary.fail + 1
            foreach ($g in $r.groups) { if ($g.status -eq "running") { $g.status = "fail" } }
            $r.groups += [pscustomobject][ordered]@{
                id = "interrupted"; name = "Interrupted"; status = "fail"; os = $null; started = $null; duration_s = $null
                checks = @([pscustomobject][ordered]@{ status = "fail"; name = "the run stopped before it finished (cancelled, or the runner stopped)"; detail = $null })
                files = @()
            }
            [System.IO.File]::WriteAllText($f, ($r | ConvertTo-Json -Depth 10), (New-Object System.Text.UTF8Encoding $false))
        } catch { }
    }
    $id = "$((Get-Date).ToUniversalTime().ToString('yyyyMMdd-HHmmss'))-$Kind"
    $script:LabRunDir = Join-Path $runs $id
    New-Item -ItemType Directory -Force $script:LabRunDir | Out-Null
    $source = $null
    if ($env:GITHUB_RUN_ID) {
        $source = [ordered]@{
            repo     = $env:GITHUB_REPOSITORY
            sha      = $env:GITHUB_SHA
            workflow = $env:GITHUB_WORKFLOW
            run_url  = "$env:GITHUB_SERVER_URL/$env:GITHUB_REPOSITORY/actions/runs/$env:GITHUB_RUN_ID"
        }
    }
    $script:LabRun = [ordered]@{
        schema   = 1
        id       = $id
        kind     = $Kind
        title    = $Title
        started  = (Get-Date).ToUniversalTime().ToString("o")
        finished = $null
        status   = "running"
        source   = $source
        release  = $null
        summary  = [ordered]@{ pass = 0; fail = 0; skip = 0 }
        groups   = New-Object System.Collections.ArrayList
    }
    # Retention: the oldest runs beyond the newest $LabRunsKept
    Get-ChildItem $runs -Directory | Sort-Object Name -Descending | Select-Object -Skip $LabRunsKept |
        Remove-Item -Recurse -Force -ErrorAction SilentlyContinue
    Save-LabRun
}

function Set-LabRelease([string]$Tag, [string]$From) {
    $script:LabRun.release = [ordered]@{ tag = $Tag; from = $(if ($From -and $From -ne "none") { $From } else { $null }) }
    Save-LabRun
}

function Set-LabTitle([string]$Title) { $script:LabRun.title = $Title; Save-LabRun }

# Starts (or returns to) a group; checks go to the current group
function Use-LabGroup([string]$Id, [string]$Name = $Id) {
    $g = $script:LabRun.groups | Where-Object { $_.id -eq $Id } | Select-Object -First 1
    if (-not $g) {
        $g = [ordered]@{
            id = $Id; name = $Name; status = "running"; os = $null
            started = (Get-Date).ToUniversalTime().ToString("o"); duration_s = $null
            checks = New-Object System.Collections.ArrayList
            files = New-Object System.Collections.ArrayList
        }
        [void]$script:LabRun.groups.Add($g)
    }
    $script:LabGroup = $g
    Save-LabRun
}

function Set-LabGroupOs([string]$Os) { $script:LabGroup.os = $Os; Save-LabRun }

# Closes the current group: its status, how long it took
function Complete-LabGroup {
    $g = $script:LabGroup
    if (-not $g) { return }
    $g.status = if (@($g.checks | Where-Object { $_.status -eq "fail" }).Count) { "fail" } else { "pass" }
    $g.duration_s = [int]((Get-Date).ToUniversalTime() - [datetime]::Parse($g.started).ToUniversalTime()).TotalSeconds
    Save-LabRun
}

function Add-LabCheck([ValidateSet("pass", "fail", "skip", "info")][string]$Status, [string]$Name, [string]$Detail = "") {
    if (-not $script:LabGroup) { Use-LabGroup "lab" "Lab" }
    [void]$script:LabGroup.checks.Add([ordered]@{ status = $Status; name = $Name; detail = $(if ($Detail) { $Detail } else { $null }) })
    if ($script:LabRun.summary.Contains($Status)) { $script:LabRun.summary[$Status]++ }
    Save-LabRun
}

function Pass([string]$what) { Write-Host "ok:   $what"; Add-LabCheck pass $what }
function Fail([string]$what, [string]$Detail = "") {
    Write-Host "FAIL: $what"
    if ($Detail) { $Detail -split "`r?`n" | ForEach-Object { Write-Host "      $_" } }
    Add-LabCheck fail $what $Detail
}
function Skip([string]$what, [string]$Detail = "") { Write-Host "skip: $what"; Add-LabCheck skip $what $Detail }
function Get-LabFailures { return $script:LabRun.summary.fail }

# Turns a VM script's output ("ok:   ...", "FAIL: ...", other lines) into
# checks; the other lines after a FAIL become its detail. With -Previous
# (the release upgraded from) a failure is recorded as "skip": a problem of
# a release already out must not fail the one under test
function Add-LabOutput([string]$Text, [string]$Prefix = "", [switch]$Previous) {
    $pending = $null
    $flush = {
        if ($pending) {
            $st = if ($Previous -and $pending.status -eq "fail") { "skip" } else { $pending.status }
            $d = ($pending.detail -join "`n").Trim()
            if ($Previous -and $pending.status -eq "fail") { $d = "Failed in the previous release (not the one under test). $d".Trim() }
            Add-LabCheck $st "$Prefix$($pending.name)" $d
        }
    }
    foreach ($line in ($Text -split "`r?`n")) {
        if ($line -match '^\s*ok:\s+(.*)$') { & $flush; $pending = @{ status = "pass"; name = $Matches[1].Trim(); detail = @() } }
        elseif ($line -match '^\s*FAIL:\s+(.*)$') { & $flush; $pending = @{ status = "fail"; name = $Matches[1].Trim(); detail = @() } }
        elseif ($line.Trim() -and $pending -and $pending.status -eq "fail") { $pending.detail += $line }
    }
    & $flush
}

# Copies a screenshot or log into the run and lists it under the current group
function Add-LabFile([string]$Path, [ValidateSet("screenshot", "log")][string]$Kind, [string]$Caption = "") {
    if (-not (Test-Path $Path)) { return }
    $name = Split-Path $Path -Leaf
    Copy-Item $Path (Join-Path $script:LabRunDir $name) -Force
    if (-not ($script:LabGroup.files | Where-Object { $_.name -eq $name })) {
        [void]$script:LabGroup.files.Add([ordered]@{ kind = $Kind; name = $name; caption = $(if ($Caption) { $Caption } else { $null }) })
    }
    Save-LabRun
}

function Complete-LabRun {
    if ($script:LabGroup -and $script:LabGroup.status -eq "running") { Complete-LabGroup }
    $script:LabRun.finished = (Get-Date).ToUniversalTime().ToString("o")
    $script:LabRun.status = if ($script:LabRun.summary.fail -gt 0) { "fail" } else { "pass" }
    Save-LabRun
}

# Written to a temporary file and moved, so a reader never sees half of it
function Save-LabRun {
    if (-not $script:LabRun) { return }
    $json = $script:LabRun | ConvertTo-Json -Depth 10
    $tmp = Join-Path $script:LabRunDir "run.json.tmp"
    [System.IO.File]::WriteAllText($tmp, $json, (New-Object System.Text.UTF8Encoding $false))
    Move-Item $tmp (Join-Path $script:LabRunDir "run.json") -Force
}
