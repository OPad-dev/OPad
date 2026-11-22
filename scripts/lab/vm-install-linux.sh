#!/usr/bin/env bash
# Runs inside a Linux lab VM as opad (NOPASSWD sudo), from its home directory.
# lab-install.ps1 copied each release's package into its own directory
# (~/old, ~/new). One phase per call:
#
#   install DIR      installs (or upgrades to) the package in ~/DIR the way
#                    this distro's user, or OPad's in-app updater, does: .deb
#                    with apt, .rpm with dnf, the AUR opad-bin PKGBUILD with
#                    makepkg. The VM then reboots, so the desktop's autostart
#                    runs
#   check VERSION [DIR]
#                    after that reboot: VERSION is the installed one, every
#                    installed file matches ~/DIR/FILES.sha256 (if the release
#                    has one), the tray
#                    GUI and the daemon run in the desktop session, opadctl
#                    reaches the daemon, tosu has its capability, the GUI's
#                    window opens on the desktop, and the app's database exists
#                    (the same one as before, after an upgrade)
#   update VERSION DIR
#                    after check, with the previous release installed: OPad
#                    updates itself to VERSION, automatically, from the signed
#                    feed in ~/DIR (beta.json, its .minisig, and for local
#                    test builds the packages it names), served on this VM's
#                    loopback. The daemon's service is restarted with
#                    OPAD_MANIFEST_URL pointing at it and asked over its IPC
#                    socket, as the app's Updates page does, to follow beta.
#                    With automatic updates on (the default) it downloads the
#                    package, and the update helper (pkexec, no password for
#                    the user at the computer) checks it against the feed and
#                    installs it with apt, dnf or pacman. The running OPad
#                    must keep running and report the new version as
#                    installed for the next start.
#   remove           uninstalls it and checks nothing of OPad is left: files,
#                    the user's data, autostart entries, processes
set -u
FAILED=0
fail() { echo "FAIL: $*"; FAILED=1; }
pass() { echo "ok:   $*"; }
cd ~ || exit 1
DB=~/.local/share/opad/opad.db

if command -v apt-get >/dev/null; then KIND=deb
elif command -v dnf >/dev/null; then KIND=rpm
elif command -v pacman >/dev/null; then KIND=aur
else echo "FAIL: no apt, dnf or pacman"; exit 1
fi

installed_version() {
  case $KIND in
    deb) dpkg-query -W -f '${Version}' opad 2>/dev/null | sed 's/~/-/; s/-[0-9]*$//' ;;
    rpm) rpm -q --qf '%{VERSION}' opad 2>/dev/null | sed 's/~/-/' ;;
    aur) pacman -Q opad-bin 2>/dev/null | awk '{print $2}' | sed 's/_/-/; s/-[0-9]*$//' ;;
  esac
}

case "${1:-}" in
install)
  dir=~/"${2:?usage: install DIR}"
  case $KIND in
    deb)
      # A fresh boot may have unattended-upgrades holding the dpkg lock
      sudo apt-get -o DPkg::Lock::Timeout=600 update -qq >install.log 2>&1
      # The in-app updater's command (opad-update apply_command)
      sudo DEBIAN_FRONTEND=noninteractive apt-get -o DPkg::Lock::Timeout=600 install -y "$dir"/opad_*_amd64.deb >>install.log 2>&1 ;;
    rpm)
      sudo dnf install -y "$dir"/opad-*.x86_64.rpm >install.log 2>&1 ;;
    aur)
      { pacman -Q base-devel >/dev/null 2>&1 || sudo pacman -Syu --needed --noconfirm base-devel; } >install.log 2>&1 &&
        (cd "$dir" && mv -f opad-bin.PKGBUILD PKGBUILD 2>/dev/null; makepkg -sif --noconfirm) >>install.log 2>&1 ;;
  esac
  rc=$?
  if [ $rc -eq 0 ]; then pass "installed $(installed_version) with $KIND"; else tail -15 install.log; fail "install ($KIND) exited $rc"; fi
  for f in /usr/bin/opad-gui /usr/bin/opad-daemon /usr/bin/opadctl /usr/lib/opad/bin/espflash \
           /usr/lib/opad/tosu/tosu /etc/xdg/autostart/opad-gui.desktop /usr/lib/udev/rules.d/70-opad.rules; do
    [ -e "$f" ] || fail "missing $f"
  done
  ;;
check)
  want="${2:?usage: check VERSION [DIR]}"
  [ "$(installed_version)" = "$want" ] && pass "version $want installed" || fail "installed version is $(installed_version), not $want"
  # Integrity: the installed files are the ones the release built. opad-bin
  # repackages the .deb, rewriting only install-origin; /usr/share/doc is
  # left out, as minimal systems tell dpkg not to install it
  sums=~/"${3:-none}"/FILES.sha256
  if [ -f "$sums" ]; then
    pkg=$KIND; [ $KIND = aur ] && pkg=deb
    n=0; bad=""
    while read -r sum entry; do
      case "$entry" in "$pkg:"*) ;; *) continue ;; esac
      path="${entry#"$pkg":}"
      case "$path" in /usr/share/doc/*) continue ;; esac
      [ $KIND = aur ] && case "$path" in */install-origin) continue ;; esac
      n=$((n + 1))
      [ "$(sha256sum "$path" 2>/dev/null | cut -d' ' -f1)" = "$sum" ] || bad="$bad
  $path"
    done < "$sums"
    if [ $n -gt 0 ] && [ -z "$bad" ]; then pass "all $n installed files match the release's FILES.sha256"
    else fail "installed files differ from the release's FILES.sha256 ($n listed)"; [ -n "$bad" ] && echo "  differ or missing:$bad"
    fi
  fi
  # The desktop's autostart entries start a while after the login
  for _ in $(seq 30); do
    pgrep -u "$USER" -x opad-gui >/dev/null && pgrep -u "$USER" -x opad-daemon >/dev/null && break
    sleep 3
  done
  pgrep -u "$USER" -x opad-gui >/dev/null && pass "the tray GUI started at login" || fail "opad-gui is not running 90 s after login"
  pgrep -u "$USER" -x opad-daemon >/dev/null && pass "the daemon started at login" || fail "opad-daemon is not running 90 s after login"
  for _ in $(seq 10); do
    out=$(timeout 10 opadctl status 2>&1) && echo "$out" | grep -q "Daemon Mode" && break
    sleep 3
  done
  echo "$out" | grep -q "Daemon Mode" && pass "opadctl status reaches the daemon" || fail "opadctl status: $(echo "$out" | tail -2)"
  getcap /usr/lib/opad/tosu/tosu | grep -q cap_sys_ptrace && pass "tosu has cap_sys_ptrace" || fail "tosu has no cap_sys_ptrace"
  # Open the window the way a click on the menu entry would, in the desktop
  # session: its display variables live in the systemd user manager
  while IFS= read -r kv; do export "$kv"; done < <(systemctl --user show-environment |
    grep -E '^(WAYLAND_DISPLAY|DISPLAY|XAUTHORITY|XDG_CURRENT_DESKTOP|XDG_SESSION_TYPE|XDG_SESSION_DESKTOP)=')
  setsid -f opad-gui >gui.log 2>&1 </dev/null
  sleep 12
  pgrep -u "$USER" -x opad-gui >/dev/null && pass "the GUI window was opened (see the screenshot)" || fail "opad-gui exited: $(tail -3 gui.log)"
  grep -iE "panic|error" gui.log | head -3
  # The data an upgrade must keep and an uninstall must delete
  if [ -f "$DB" ]; then
    id=$(stat -c %i "$DB")
    if [ ! -f .opad-lab-db ]; then echo "$id" > .opad-lab-db; pass "the app wrote its data ($DB)"
    elif [ "$(cat .opad-lab-db)" = "$id" ]; then pass "the upgrade kept the app's data ($DB)"
    else fail "the upgrade replaced $DB"
    fi
  else fail "no $DB after use"
  fi
  ;;
update)
  want="${2:?usage: update VERSION DIR}"
  feed=~/"${3:?usage: update VERSION DIR}"
  [ -f "$feed/beta.json" ] && [ -f "$feed/beta.json.minisig" ] || { fail "no signed feed in $feed"; exit 1; }
  # python3 serves the feed and talks to the daemon
  if ! command -v python3 >/dev/null; then
    case $KIND in
      aur) sudo pacman -S --needed --noconfirm python ;;
      deb) sudo apt-get -o DPkg::Lock::Timeout=600 install -y python3 ;;
      rpm) sudo dnf install -y python3 ;;
    esac >update.log 2>&1 || { fail "cannot install python3"; exit 1; }
  fi
  [ -x /usr/lib/opad/opad-update-helper ] && pass "the update helper is installed" ||
    fail "/usr/lib/opad/opad-update-helper is missing"
  [ -f /usr/share/polkit-1/actions/dev.opad.update.policy ] && pass "its polkit action is installed" ||
    fail "the polkit action dev.opad.update is missing"
  # The feed on loopback (the updater takes plain HTTP from there only), in
  # the user's systemd so it outlives SSH
  systemctl --user stop opad-lab-feed 2>/dev/null
  systemd-run --user --unit=opad-lab-feed --collect \
    python3 -m http.server 8000 --bind 127.0.0.1 --directory "$feed" >/dev/null 2>&1
  # The daemon's own service, pointed at it
  systemctl --user set-environment OPAD_MANIFEST_URL=http://127.0.0.1:8000/beta.json
  systemctl --user restart opad-daemon
  sleep 3
  before_pid=$(pgrep -u "$USER" -x opad-daemon | head -1)
  daemon_log() { journalctl --user -u opad-daemon -n "${1:-30}" --no-pager -o cat 2>/dev/null; }
  # The daemon's IPC (opad-ipc: a little-endian u32 length, then JSON), as
  # the GUI's Updates page uses it: follow beta, then watch it update itself
  cat > lab-ipc.py <<'PY'
import json, os, re, socket, struct, sys, time
want = sys.argv[1]
path = os.path.join(os.environ.get("XDG_RUNTIME_DIR") or "/run/user/%d" % os.getuid(), "opad", "daemon.sock")
def conn():
    for _ in range(60):
        try:
            s = socket.socket(socket.AF_UNIX); s.connect(path); return s
        except OSError:
            time.sleep(1)
    print("FAIL: the daemon's socket %s did not appear within 60 s" % path); sys.exit(1)
def ask(s, req):
    data = json.dumps(req).encode()
    s.sendall(struct.pack("<I", len(data)) + data)
    def read(n):
        b = b""
        while len(b) < n:
            c = s.recv(n - len(b))
            if not c: raise EOFError("the daemon closed the connection")
            b += c
        return b
    return json.loads(read(struct.unpack("<I", read(4))[0]))
def handshake():
    # The daemon accepts a client of its own version only: it names it
    s = conn()
    r = ask(s, {"Handshake": {"client_version": "0", "client_protocol": 1}})
    m = re.search(r"running daemon is (\S+?)\.? ", json.dumps(r))
    version = m.group(1) if m else "0"
    s = conn()
    r = ask(s, {"Handshake": {"client_version": version, "client_protocol": 1}})
    if "HandshakeAck" not in r:
        print("FAIL: the daemon refused the handshake: %s" % r); sys.exit(1)
    return s, version
s, running = handshake()
print("info: the running daemon is %s" % running)
r = ask(s, {"SetUpdateChannel": {"channel": "beta"}})
if r != "UpdateCheckStarted":
    print("FAIL: following the beta channel: %s" % r); sys.exit(1)
# Automatic updates are on by default: the check finds it, then it installs
deadline = time.time() + 900
seen = False
while True:
    time.sleep(2)
    st = ask(s, "GetUpdateStatus")["UpdateStatus"]
    if st["app"].get("available") == want and not seen:
        seen = True
        print("ok:   the updater found %s in the signed feed" % want)
    if st.get("restart_required") and st.get("pending_version") == want:
        break
    if time.time() > deadline:
        print("FAIL: %s was not installed within 900 s of following beta (last error: %s, last result: %s)"
              % (want, st.get("last_error"), st.get("last_result"))); sys.exit(1)
print("ok:   it installed %s by itself, for the next start (%s)" % (want, st.get("last_result")))
if not st["app"].get("enabled"):
    print("FAIL: automatic updates were off")
PY
  out=$(python3 lab-ipc.py "$want" 2>&1); rc=$?
  echo "$out"
  if [ $rc -ne 0 ]; then echo "  daemon log:"; daemon_log 40 | sed 's/^/  /'; fail "updating itself"; fi
  if [ "$(installed_version)" = "$want" ]; then pass "the $KIND package is now $want"
  else fail "the package is still $(installed_version), not $want"; fi
  # Never restarted: the old OPad keeps running until the next start
  [ -n "$before_pid" ] && [ "$(pgrep -u "$USER" -x opad-daemon | head -1)" = "$before_pid" ] &&
    pass "the running daemon was left running" || fail "the daemon was restarted by the update"
  systemctl --user unset-environment OPAD_MANIFEST_URL
  systemctl --user stop opad-lab-feed 2>/dev/null
  ;;
remove)
  # tosu's directories go only where OPad created them (its marker); one
  # made by a release before the marker existed is kept, like a user's own
  marked=""
  for d in ~/.config/tosu ~/.local/share/tosu ~/.cache/tosu; do
    [ -f "$d/.created-by-opad" ] && marked="$marked $d"
  done
  case $KIND in
    deb) sudo apt-get -o DPkg::Lock::Timeout=600 remove -y opad >remove.log 2>&1 ;;
    rpm) sudo dnf remove -y opad >remove.log 2>&1 ;;
    aur) sudo pacman -Rns --noconfirm opad-bin >remove.log 2>&1 ;;
  esac
  rc=$?
  if [ $rc -eq 0 ]; then pass "removed"; else tail -10 remove.log; fail "remove exited $rc"; fi
  sleep 2
  # Nothing of OPad is left: files, the user's data, autostart, processes
  for f in /usr/bin/opad-gui /usr/bin/opad-daemon /usr/bin/opadctl /usr/lib/opad /etc/opad \
           /etc/xdg/autostart/opad-gui.desktop ~/.local/share/opad ~/.local/state/opad \
           ~/.config/autostart/opad-gui.desktop; do
    [ -e "$f" ] && fail "left behind: $f"
  done
  for d in ~/.config/tosu ~/.local/share/tosu ~/.cache/tosu; do
    case " $marked " in
      *" $d "*) [ -e "$d" ] && fail "left behind: $d (tosu's, created by OPad)" ;;
      *) [ -e "$d" ] && echo "info: kept $d: not marked as OPad's (a tosu of the user's, or made by a release before rc.4)" ;;
    esac
  done
  for n in opad-gui opad-daemon; do
    pgrep -x $n >/dev/null && fail "$n still running after removal"
  done
  pgrep -f '^/usr/lib/opad/tosu/tosu' >/dev/null && fail "tosu still running after removal"
  [ $FAILED -eq 0 ] && pass "nothing of OPad left"
  ;;
*) echo "usage: $0 install DIR | check VERSION [DIR] | update VERSION DIR | remove"; exit 2 ;;
esac
[ $FAILED -eq 0 ]
