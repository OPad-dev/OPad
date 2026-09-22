#!/usr/bin/env bash
# Installs dist/*.deb and dist/*.rpm in clean distro containers and checks each
# works there: dependencies resolve, the post-install scripts run (tosu gets
# cap_sys_ptrace), no shared library is missing, espflash runs, the daemon
# starts and answers opadctl, tosu starts, and the package removes cleanly.
#
#   scripts/release/test_packages.sh [image ...]      (needs docker)
#
# opad-gui cannot open a window in a container; it is checked to load (all
# libraries resolve, no symbol errors), which is where packaging breaks it.
set -euo pipefail
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
DIST="${REPO_ROOT}/dist"
IMAGES=("$@")
[ ${#IMAGES[@]} -gt 0 ] || IMAGES=(ubuntu:22.04 ubuntu:24.04 debian:12 debian:13 rockylinux:9 fedora:43)

INNER='
set -u
. /etc/os-release
echo "===== $PRETTY_NAME (glibc $(ldd --version | head -1 | grep -o "[0-9.]*$")) ====="
fail() { echo "FAIL: $*"; FAILED=1; }
FAILED=0
if command -v apt-get >/dev/null; then
  export DEBIAN_FRONTEND=noninteractive
  apt-get update -qq >/dev/null
  apt-get install -y -qq libcap2-bin procps >/dev/null 2>&1
  apt-get install -y -qq /dist/*.deb >/tmp/install.log 2>&1 || { tail -5 /tmp/install.log; fail "install"; }
  REMOVE="apt-get remove -y -qq opad"
else
  dnf install -y -q libcap procps-ng >/dev/null 2>&1
  dnf install -y -q /dist/*.rpm >/tmp/install.log 2>&1 || { tail -5 /tmp/install.log; fail "install"; }
  REMOVE="dnf remove -y -q opad"
fi
getcap /usr/lib/opad/tosu/tosu | grep -q cap_sys_ptrace || fail "tosu has no cap_sys_ptrace"
for b in /usr/bin/opad-daemon /usr/bin/opad-gui /usr/bin/opadctl /usr/lib/opad/bin/espflash /usr/lib/opad/tosu/tosu; do
  ldd "$b" | grep "not found" && fail "$b: missing library"
done
out=$(opad-gui --version 2>&1); echo "$out" | grep -qiE "symbol lookup error|error while loading" && fail "opad-gui: $out"
/usr/lib/opad/bin/espflash --version >/dev/null || fail "espflash"
for f in /etc/xdg/autostart/opad-gui.desktop /usr/lib/udev/rules.d/70-opad.rules \
         /usr/lib/systemd/user/opad-daemon.service /usr/lib/opad/tosu/THIRD_PARTY_NOTICES.txt; do
  [ -f "$f" ] || fail "missing $f"
done
export HOME=/root
# Before the daemon, whose supervisor would start its own tosu on the port.
# tosu must still be running (serving its dashboard) when the timeout ends it
timeout 8 /usr/lib/opad/tosu/tosu >/tmp/tosu.log 2>&1; rc=$?
[ $rc -eq 124 ] && sed "s/\x1b\[[0-9;]*m//g" /tmp/tosu.log | grep -q "Dashboard server started" \
  || fail "tosu did not start (exit $rc): $(sed "s/\x1b\[[0-9;]*m//g" /tmp/tosu.log | tail -3)"
export HOME=/root XDG_RUNTIME_DIR=/tmp/xdg; mkdir -p -m 700 $XDG_RUNTIME_DIR
opad-daemon >/tmp/daemon.log 2>&1 &
sleep 3
opadctl status 2>&1 | grep -q "Daemon Mode" || fail "daemon did not answer opadctl status"
grep -qi panic /tmp/daemon.log && fail "daemon panicked"
kill %1 2>/dev/null; sleep 1
$REMOVE >/dev/null 2>&1 || fail "remove"
[ $FAILED -eq 0 ] && echo "PASS" || exit 1
'
status=0
for img in "${IMAGES[@]}"; do
  # SYS_PTRACE: a file capability the container cannot grant makes exec fail
  docker run --rm --cap-add SYS_PTRACE -v "${DIST}:/dist:ro" "$img" bash -c "$INNER" || status=1
done
exit $status
