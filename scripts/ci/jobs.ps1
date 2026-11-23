# The Windows jobs of CI. .github/workflows/ci.yml runs them on GitHub's
# windows-latest, and scripts/ci/lab-ci.sh runs the same ones on the lab's
# mini PC (Windows 11, Rust + VS Build Tools), so a check passes or fails the
# same way in both places. Windows PowerShell 5.1 compatible.
#
#   scripts\ci\jobs.ps1 <job>...   jobs: rust-windows rust-windows-gui
#   scripts\ci\jobs.ps1 all        both
#   scripts\ci\jobs.ps1 list       their names
param([Parameter(ValueFromRemainingArguments = $true)][string[]]$Jobs)

# Not "Stop": PowerShell 5.1 turns a native command's stderr (cargo's
# progress) into errors. Failures are exit codes, checked by Run; throw
# still ends the script.
$ErrorActionPreference = "Continue"
Set-Location (Join-Path $PSScriptRoot "..\..\desktop")

$AllJobs = @("rust-windows", "rust-windows-gui")

# A native command's failure is its exit code, which PowerShell does not stop on.
# A bare -- in a call to Run is eaten by PowerShell itself: quote it ('--').
function Run {
    Write-Host "> $($args -join ' ')"
    & $args[0] $args[1..($args.Count - 1)]
    if ($LASTEXITCODE -ne 0) { throw "failed ($LASTEXITCODE): $($args -join ' ')" }
}

function Job-rust-windows {
    # Section W4-1: cargo check alone is not enough, the ksni/tray-icon split
    # and the IPC transport aliases fail at link time. opad-gui is the job
    # below, so that one crate's Windows port does not hide the other eleven.
    Run cargo fmt --check
    Run cargo clippy --workspace --exclude opad-gui --all-targets '--' -D warnings
    Run cargo build --workspace --exclude opad-gui
    Run cargo test --workspace --exclude opad-gui
}

function Job-rust-windows-gui {
    Run cargo clippy -p opad-gui --all-targets '--' -D warnings
    Run cargo build -p opad-gui
    Run cargo test -p opad-gui
}

if (-not $Jobs) { Write-Host "jobs: $($AllJobs -join ' '), or all"; exit 2 }
if ($Jobs -contains "list") { Write-Output ($AllJobs -join " "); exit 0 }
if ($Jobs -contains "all") { $Jobs = $AllJobs }
foreach ($job in $Jobs) {
    if (-not (Get-Command "Job-$job" -ErrorAction SilentlyContinue)) {
        Write-Host "unknown job: $job (jobs: $($AllJobs -join ' '))"
        exit 2
    }
    Write-Host "::group::$job"
    & "Job-$job"
    Write-Host "::endgroup::"
}
