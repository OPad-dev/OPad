# The mini PC's half of scripts/ci/lab-ci.sh: checks the commit out of the
# bundle lab-ci.sh copied in, with Git for Windows' default CRLF line ends as
# on GitHub's windows-latest, and runs every Windows job of CI (jobs.ps1),
# each to the end even after another failed. Prints "JOBS <names>", then
# one "RESULT <job> pass|fail <time>" line per job; exits 1 if any failed.
#
#   lab-run.ps1 <bundle> <sha>
param([string]$Bundle, [string]$Sha)

$ErrorActionPreference = "Continue"
$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"
$env:CARGO_TERM_COLOR = "never"
# A smaller build cache on a small disk: no incremental state and no debug
# info, neither of which changes what the jobs check
$env:CARGO_INCREMENTAL = "0"
$env:CARGO_PROFILE_DEV_DEBUG = "0"
$env:CARGO_PROFILE_TEST_DEBUG = "0"
$work = "C:\opad-lab\ci\repo"

# Not named Git: PowerShell names are case-insensitive, so it would call itself
function Invoke-Git { & git.exe @args; if ($LASTEXITCODE -ne 0) { Write-Host "git $args failed"; exit 1 } }
if (-not (Test-Path "$work\.git")) { Invoke-Git init -q $work }
Set-Location $work
Invoke-Git fetch -q $Bundle "refs/lab-ci/$Sha"
Invoke-Git checkout -q -f --detach $Sha
# A clean checkout, keeping only the build cache
Invoke-Git clean -q -ffdx -e /desktop/target/

$failed = 0
$list = & powershell -NoProfile -ExecutionPolicy Bypass -File scripts\ci\jobs.ps1 list
Write-Output "JOBS $list"
$jobs = $list.Split(" ")
foreach ($job in $jobs) {
    $t = Get-Date
    & powershell -NoProfile -ExecutionPolicy Bypass -File scripts\ci\jobs.ps1 $job
    if ($LASTEXITCODE -eq 0) { $r = "pass" } else { $r = "fail"; $failed = 1 }
    Write-Output ("RESULT {0} {1} {2}s" -f $job, $r, [int]((Get-Date) - $t).TotalSeconds)
}
exit $failed
