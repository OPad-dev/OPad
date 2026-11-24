#!/usr/bin/env bash
# Runs all of CI (.github/workflows/ci.yml) on the lab's mini PC for one
# commit, before it may go to GitHub (owner, 2026-11-23): the Linux jobs in
# the ci-linux VM and the Windows jobs on the mini PC itself, at the same
# time, from a copy of exactly that commit. When every job passes it records
# the commit, and only then does the pre-push hook (scripts/git-hooks/
# pre-push) let it be pushed to main. `make push` runs this and pushes.
#
#   scripts/ci/lab-ci.sh [<commit>]    default HEAD; a passed commit is not
#                                      run again (--force runs it anyway)
#
# Logs: <git dir>/lab-ci/<sha>/{linux,windows}.log
# Environment: OPAD_LAB_HOST (default opad@192.168.0.150),
#              OPAD_LAB_CI_PORT (the VM's SSH port forward, default 2260)
set -euo pipefail

force=0
rev=HEAD
for a in "$@"; do
    case "$a" in
        --force) force=1 ;;
        -h | --help) sed -n '2,14p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
        *) rev="$a" ;;
    esac
done

LAB_HOST="${OPAD_LAB_HOST:-opad@192.168.0.150}"
VM_PORT="${OPAD_LAB_CI_PORT:-2260}"
VM=ci-linux
VBOX='C:\Program Files\Oracle\VirtualBox\VBoxManage.exe'

sha="$(git rev-parse --verify "$rev^{commit}")"
common="$(cd "$(git rev-parse --git-common-dir)" && pwd)"
out="$common/lab-ci/$sha"
mkdir -p "$out"

if [ -f "$out/ok" ] && [ "$force" = 0 ]; then
    echo "lab CI already passed ${sha:0:7} ($(cat "$out/ok"))"
    exit 0
fi
if [ "$(git rev-parse HEAD)" = "$sha" ] && [ -n "$(git status --porcelain --untracked-files=no)" ]; then
    echo "note: uncommitted changes are not tested; the lab runs commit ${sha:0:7}"
fi

exec 9>"$common/lab-ci/.lock"
flock -n 9 || { echo "another lab CI run is going on" >&2; exit 1; }

SSH_OPTS=(-o BatchMode=yes -o LogLevel=ERROR -o ServerAliveInterval=30 -o ConnectTimeout=10)
host() { ssh "${SSH_OPTS[@]}" "$LAB_HOST" "$@"; }
VM_SSH=("${SSH_OPTS[@]}" -o "ProxyCommand=ssh ${SSH_OPTS[*]} -W %h:%p $LAB_HOST" -o StrictHostKeyChecking=accept-new)
vm() { ssh "${VM_SSH[@]}" -p "$VM_PORT" opad@127.0.0.1 "$@"; }
vm_scp() { scp -q "${VM_SSH[@]}" -P "$VM_PORT" "$@"; }

echo "lab CI for ${sha:0:7}: $(git log -1 --format=%s "$sha")"

# Disk space on the mini PC. When it runs out, VirtualBox pauses ci-linux
# (its disk image cannot grow) and the Windows build dies writing its
# cache, both far from the cause. Under 20 GB free, the Windows build
# cache goes (it rebuilds in a few minutes); under 10 GB the run stops here.
free_gb() { host '[int]((Get-PSDrive C).Free / 1GB)' | tr -dc '0-9'; }
free="$(free_gb)"
if [ "${free:-0}" -lt 20 ]; then
    echo "the mini PC has ${free} GB free: removing the Windows build cache"
    host "\$t = 'C:\\opad-lab\\ci\\repo\\desktop\\target'; if (Test-Path \$t) { Remove-Item -Recurse -Force \$t }; exit 0"
    free="$(free_gb)"
fi
if [ "${free:-0}" -lt 10 ]; then
    echo "the mini PC has only ${free} GB free on C: (the VMs need room to grow): free some space, then run it again" >&2
    exit 1
fi

# The VM stays set up between runs; boot it if it is off, resume it if a
# full disk paused it
state="$(host "& '$VBOX' showvminfo $VM --machinereadable" | sed -n 's/^VMState="\(.*\)".*/\1/p')"
if [ "$state" = paused ]; then
    echo "resuming $VM (paused, probably by a full disk)"
    host "& '$VBOX' controlvm $VM resume" >/dev/null
elif [ "$state" != running ]; then
    echo "starting $VM"
    host "& '$VBOX' startvm $VM --type headless" >/dev/null
fi
for _ in $(seq 60); do vm true 2>/dev/null && break; sleep 3; done
vm true

# Exactly this commit, with its history (gen_proto's check diffs against it)
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"; git update-ref -d "refs/lab-ci/$sha" 2>/dev/null || true' EXIT
git update-ref "refs/lab-ci/$sha" "$sha"
git bundle create -q "$tmp/ci.bundle" "refs/lab-ci/$sha"
git show "$sha:scripts/ci/lab-run.sh" >"$tmp/lab-run.sh"
git show "$sha:scripts/ci/lab-run.ps1" >"$tmp/lab-run.ps1"

vm mkdir -p ci
vm_scp "$tmp/ci.bundle" "$tmp/lab-run.sh" opad@127.0.0.1:ci/
host "New-Item -ItemType Directory -Force C:\\opad-lab\\ci | Out-Null"
scp -q "${SSH_OPTS[@]}" "$tmp/ci.bundle" "$tmp/lab-run.ps1" "$LAB_HOST:C:/opad-lab/ci/"

echo "running the Linux jobs (ci-linux VM) and the Windows jobs (mini PC); logs in $out"
start=$SECONDS
(vm "bash ci/lab-run.sh ~/ci/ci.bundle $sha" >"$out/linux.log" 2>&1) &
linux=$!
(host "powershell -NoProfile -ExecutionPolicy Bypass -File C:\\opad-lab\\ci\\lab-run.ps1 C:\\opad-lab\\ci\\ci.bundle $sha" >"$out/windows.log" 2>&1) &
windows=$!
lstatus=0 wstatus=0
wait "$linux" || lstatus=$?
wait "$windows" || wstatus=$?

echo
results="$(grep -h '^RESULT ' "$out/linux.log" "$out/windows.log" | tr -d '\r' || true)"
printf '%s\n' "$results" | awk 'NF { printf "  %-22s %-4s %s\n", $2, $3, $4 }'
# Every job both halves listed must report, or the run did not finish
expected=0
for log in "$out/linux.log" "$out/windows.log"; do
    line="$(grep -m1 '^JOBS ' "$log" | tr -d '\r' || true)"
    [ -n "$line" ] || { lstatus=1; continue; }
    expected=$((expected + $(wc -w <<<"$line") - 1))
done
reported="$(printf '%s\n' "$results" | grep -c '^RESULT ' || true)"
passed="$(printf '%s\n' "$results" | grep -c ' pass ' || true)"
elapsed=$(((SECONDS - start) / 60))
if [ "$lstatus" = 0 ] && [ "$wstatus" = 0 ] && [ "$reported" = "$expected" ] && [ "$passed" = "$expected" ]; then
    date -Iseconds >"$out/ok"
    echo "lab CI passed ${sha:0:7} in ${elapsed} min: it may be pushed"
else
    rm -f "$out/ok"
    echo "lab CI FAILED ${sha:0:7} ($passed of $expected jobs passed, ${elapsed} min)."
    echo "See $out/linux.log and $out/windows.log"
    exit 1
fi
