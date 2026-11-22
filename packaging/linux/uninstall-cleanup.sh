#!/bin/sh
# Run as root by the .deb's prerm, the .rpm's %preun and opad-bin's
# pre_remove when OPad is uninstalled (never on an upgrade), while its files
# are still in place. It leaves nothing of OPad behind:
#
#  - stops OPad and the bundled tosu in every session;
#  - deletes each user's OPad data and logs (the database with settings,
#    layouts and the PC's play statistics; the pad's own counters live on the
#    pad and are not touched), the autostart entry the GUI may have written,
#    and an enabled user service link;
#  - deletes the update helper's package cache (/var/cache/opad-update);
#  - deletes tosu's directories only where OPad created them
#    (opad-tosu's OWNED_MARKER), never those of a tosu the user installed.
#
# Paths are the XDG defaults: a user's custom $XDG_*_HOME is not visible here.
# Installed as /usr/lib/opad/uninstall-cleanup.

for name in opad-gui opad-daemon opadctl; do
    pkill -x "$name" 2>/dev/null || true
done
pkill -f '^/usr/lib/opad/tosu/tosu' 2>/dev/null || true

# Where the update helper copies a package before installing it (normally
# already gone: it removes its copy afterwards)
rm -rf /var/cache/opad-update

# Every account with a home directory (users from 1000 up, and root)
getent passwd | while IFS=: read -r user _ uid _ _ home _; do
    [ "$uid" -ge 1000 ] || [ "$uid" -eq 0 ] || continue
    [ "$uid" -ne 65534 ] || continue
    case "$home" in /|"") continue ;; esac
    [ -d "$home" ] || continue

    rm -rf "$home/.local/share/opad" "$home/.local/state/opad"
    rm -f "$home/.config/autostart/opad-gui.desktop"
    link="$home/.config/systemd/user/default.target.wants/opad-daemon.service"
    if [ -L "$link" ] && [ "$(readlink "$link")" = /usr/lib/systemd/user/opad-daemon.service ]; then
        rm -f "$link"
    fi
    for dir in "$home/.config/tosu" "$home/.local/share/tosu" "$home/.cache/tosu"; do
        [ -f "$dir/.created-by-opad" ] && rm -rf "$dir"
    done
done
exit 0
