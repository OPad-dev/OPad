#!/usr/bin/env bash
set -euo pipefail

# -----------------------------------------------------------------------------
# Package build script for OPad (Linux .deb, .rpm, AppImage, Windows .exe)
# Requirements: §L-1, §L-3, §L-4, §T-2, §T-3, §W2-1
# -----------------------------------------------------------------------------

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
DIST_DIR="${REPO_ROOT}/dist"

mkdir -p "${DIST_DIR}"

echo "=========================================="
echo "  OPad Package Build                      "
echo "=========================================="

# 1. Ensure prerequisites (tosu bundle & host release binaries)
echo ""
echo "--- [1/5] Ensuring bundled tosu and host binaries ---"
if [ ! -f "${REPO_ROOT}/build/tosu/tosu" ] || [ ! -f "${REPO_ROOT}/build/tosu/VERSION" ]; then
    echo "Building bundled tosu (v4.26.2)..."
    make -C "${REPO_ROOT}" tosu
else
    echo "✓ Bundled tosu already present in build/tosu"
fi

if [ ! -x "${REPO_ROOT}/build/espflash/espflash" ]; then
    make -C "${REPO_ROOT}" espflash
fi

echo "Building release binaries..."
make -C "${REPO_ROOT}" all
echo "✓ Host release binaries ready"

# Ensure templated systemd user units have @BINDIR@ replaced with /usr/bin (§L-1)
sed 's|@BINDIR@|/usr/bin|g' "${REPO_ROOT}/packaging/linux/systemd-user/opad-daemon.service.in" > "${REPO_ROOT}/packaging/linux/deb/opad-daemon.service"
sed 's|@BINDIR@|/usr/bin|g' "${REPO_ROOT}/packaging/linux/systemd-user/opad-daemon.service.in" > "${REPO_ROOT}/packaging/linux/rpm/opad-daemon.service"

# Ensure install-origin markers have no trailing newlines
printf "%s" "deb" > "${REPO_ROOT}/packaging/linux/deb/install-origin"
printf "%s" "rpm" > "${REPO_ROOT}/packaging/linux/rpm/install-origin"
printf "%s" "appimage" > "${REPO_ROOT}/packaging/linux/appimage/install-origin"
printf "%s" "windows" > "${REPO_ROOT}/packaging/windows/install-origin"

# 2. Build Debian package (.deb)
echo ""
echo "--- [2/5] Building Debian package (.deb) ---"
if command -v cargo-deb >/dev/null 2>&1; then
    (cd "${REPO_ROOT}/desktop" && cargo deb -p opad-gui --no-build -o "${DIST_DIR}/")
    echo "✓ Debian package built in dist/"
else
    echo "ERROR: 'cargo-deb' not found in PATH!" >&2
    exit 1
fi

# 3. Build RPM package (.rpm)
echo ""
echo "--- [3/5] Building RPM package (.rpm) ---"
if command -v cargo-generate-rpm >/dev/null 2>&1; then
    (cd "${REPO_ROOT}/desktop/gui" && cargo generate-rpm --auto-req disabled -o "${DIST_DIR}/")
    echo "✓ RPM package built in dist/"
else
    echo "ERROR: 'cargo-generate-rpm' not found in PATH!" >&2
    exit 1
fi

# 4. Build AppImage
echo ""
echo "--- [4/5] Building Linux AppImage ---"
"${REPO_ROOT}/packaging/linux/appimage/build_appimage.sh"
echo "✓ AppImage built in dist/"

# 5. Build Windows installer (.exe) if iscc is available
echo ""
echo "--- [5/5] Building Windows installer (.exe) ---"
if command -v iscc >/dev/null 2>&1; then
    echo "Running Inno Setup compiler (iscc)..."
    iscc "${REPO_ROOT}/packaging/windows/installer.iss" "/O${DIST_DIR}"
    echo "✓ Windows installer built in dist/"
else
    echo "WARNING: 'iscc' (Inno Setup Compiler) not found; skipping Windows installer build on Linux."
fi

# 6. Generate / update SHA256SUMS in dist/
echo ""
echo "--- Generating SHA256SUMS ---"
cd "${DIST_DIR}"
rm -f SHA256SUMS test.deb test.rpm
sha256sum * > SHA256SUMS
echo "✓ Checksums generated in dist/SHA256SUMS:"
cat SHA256SUMS

echo ""
echo "=========================================="
echo "  Package Build Finished Successfully!    "
echo "  Artifacts available in: ${DIST_DIR}     "
echo "=========================================="
