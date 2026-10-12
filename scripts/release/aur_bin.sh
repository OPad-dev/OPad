#!/usr/bin/env bash
set -euo pipefail

# -----------------------------------------------------------------------------
# Writes the AUR opad-bin PKGBUILD and .SRCINFO for one app release
# (docs/specs/release-and-updates.md §10.1).
#
#   aur_bin.sh VERSION DEB OUT_DIR
#
#   VERSION  the app version, e.g. 1.0.0-rc.1
#   DEB      the release's .deb (only its SHA-256 is read)
#   OUT_DIR  receives PKGBUILD, .SRCINFO and opad-bin.install
#
# Pushing them to the AUR is a manual step (it needs your AUR SSH key):
#   git clone ssh://aur@aur.archlinux.org/opad-bin.git && cd opad-bin
#   cp OUT_DIR/{PKGBUILD,.SRCINFO,opad-bin.install} . && git commit -am "v VERSION" && git push
# On Arch, `makepkg --printsrcinfo` should print the same .SRCINFO.
# -----------------------------------------------------------------------------

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"

VERSION="$1"
DEB="$2"
OUT="$3"

# pacman versions cannot contain "-": 1.0.0-rc.1 becomes 1.0.0_rc.1, which
# vercmp still orders before 1.0.0
PKGVER="${VERSION//-/_}"
SHA256="$(sha256sum "${DEB}" | cut -d' ' -f1)"
INSTALL="${REPO_ROOT}/packaging/linux/arch/opad-bin.install"
INSTALL_SHA256="$(sha256sum "${INSTALL}" | cut -d' ' -f1)"

mkdir -p "${OUT}"
sed -e "s|@VERSION@|${VERSION}|g" -e "s|@PKGVER@|${PKGVER}|g" \
    -e "s|@SHA256@|${SHA256}|g" -e "s|@INSTALL_SHA256@|${INSTALL_SHA256}|g" \
    "${REPO_ROOT}/packaging/linux/arch/PKGBUILD-bin.in" > "${OUT}/PKGBUILD"
cp "${INSTALL}" "${OUT}/opad-bin.install"

# What makepkg --printsrcinfo writes for this PKGBUILD
URL="https://github.com/OPad-dev/OPad"
cat > "${OUT}/.SRCINFO" <<EOF
pkgbase = opad-bin
	pkgdesc = Low-latency ESP32-S3 keypad manager, telemetry HUD, and tray applet for OPad (release binaries)
	pkgver = ${PKGVER}
	pkgrel = 1
	url = ${URL}
	install = opad-bin.install
	arch = x86_64
	license = GPL-3.0-or-later
	license = LGPL-3.0-only
	license = OFL-1.1
	depends = vulkan-loader
	depends = wayland
	depends = libx11
	depends = libxcursor
	depends = libxkbcommon
	depends = libxi
	depends = libxrandr
	depends = dbus
	optdepends = vulkan-driver: Hardware Vulkan acceleration for iced/wgpu GUI renderer
	optdepends = libcap: lets tosu read osu! under Yama ptrace_scope 1-2 (setcap at install)
	provides = opad
	conflicts = opad
	conflicts = opad-git
	noextract = opad_${VERSION}_amd64.deb
	source = opad_${VERSION}_amd64.deb::${URL}/releases/download/v${VERSION}/opad_${VERSION}_amd64.deb
	source = opad-bin.install
	sha256sums = ${SHA256}
	sha256sums = ${INSTALL_SHA256}

pkgname = opad-bin
EOF
echo "✓ ${OUT}/PKGBUILD and ${OUT}/.SRCINFO for opad-bin ${PKGVER}"
