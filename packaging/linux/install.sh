#!/usr/bin/env bash
set -euo pipefail

# -----------------------------------------------------------------------------
# osu!pad Host Installation Script for Linux (Arch / Debian / Fedora)
# -----------------------------------------------------------------------------

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
BIN_DIR="${HOME}/.local/bin"
SYSTEMD_USER_DIR="${HOME}/.config/systemd/user"
APPLICATIONS_DIR="${HOME}/.local/share/applications"

echo "=== Installing osu!pad Desktop Suite ==="

# 1. Ensure target directories exist
mkdir -p "${BIN_DIR}" "${SYSTEMD_USER_DIR}" "${APPLICATIONS_DIR}"

# 2. Build binaries in release mode
echo "Building desktop binaries in release mode..."
cargo build --release --manifest-path "${REPO_ROOT}/desktop/Cargo.toml"

# 3. Copy binaries
echo "Installing binaries into ${BIN_DIR}..."
cp "${REPO_ROOT}/desktop/target/release/osupad-daemon" "${BIN_DIR}/"
cp "${REPO_ROOT}/desktop/target/release/osupadctl" "${BIN_DIR}/"
cp "${REPO_ROOT}/desktop/target/release/osupad-gui" "${BIN_DIR}/"
chmod +x "${BIN_DIR}/osupad-daemon" "${BIN_DIR}/osupadctl" "${BIN_DIR}/osupad-gui"

# 4. Install Desktop Entry
echo "Installing desktop application launcher..."
cp "${SCRIPT_DIR}/osupad.desktop" "${APPLICATIONS_DIR}/"

# 5. Install Systemd User Service or Autostart Fallback (§P2-8, §P2-9)
AUTOSTART_DIR="${HOME}/.config/autostart"
mkdir -p "${AUTOSTART_DIR}"

if systemctl --user show-environment >/dev/null 2>&1 || systemctl --user is-system-running >/dev/null 2>&1; then
    echo "Installing systemd user service..."
    cp "${SCRIPT_DIR}/systemd-user/osupad-daemon.service" "${SYSTEMD_USER_DIR}/"
    systemctl --user daemon-reload || true
    systemctl --user enable osupad-daemon.service || true
    systemctl --user import-environment DISPLAY WAYLAND_DISPLAY DBUS_SESSION_BUS_ADDRESS || true
    echo "✓ systemd user service installed."
else
    echo "systemctl --user not available; installing daemon XDG autostart fallback..."
    cp "${SCRIPT_DIR}/xdg-autostart/osupad-daemon.desktop" "${AUTOSTART_DIR}/"
    echo "✓ Daemon autostart entry installed."
fi

echo "Installing GUI tray autostart entry..."
cp "${SCRIPT_DIR}/xdg-autostart/osupad-gui.desktop" "${AUTOSTART_DIR}/"
echo "✓ GUI autostart entry installed."

# 6. Install udev rule if requested
echo ""
echo "Installing udev rules for serial port permissions requires sudo access."
if command -v sudo >/dev/null 2>&1; then
    sudo cp "${SCRIPT_DIR}/udev/99-osupad.rules" /etc/udev/rules.d/
    sudo udevadm control --reload-rules && sudo udevadm trigger || true
    echo "✓ udev rules installed and reloaded."
else
    echo "Notice: Please manually copy ${SCRIPT_DIR}/udev/99-osupad.rules to /etc/udev/rules.d/"
fi

echo ""
echo "=== Installation Finished ==="
echo "You can now run:"
echo "  systemctl --user start osupad-daemon.service  # Start daemon"
echo "  osupadctl status                              # Check status"
echo "  osupad-gui                                    # Launch GUI"
