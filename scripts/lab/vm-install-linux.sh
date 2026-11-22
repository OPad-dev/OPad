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
*) echo "usage: $0 install DIR | check VERSION [DIR] | remove"; exit 2 ;;
esac
[ $FAILED -eq 0 ]
