#!/usr/bin/env bash
set -euo pipefail

# -----------------------------------------------------------------------------
# OPad AppImage packaging script (L-4)
# Assembles AppDir and builds opad-x86_64.AppImage
# -----------------------------------------------------------------------------

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../../.." && pwd)"
BUILD_DIR="${REPO_ROOT}/build/appimage"
APPDIR="${BUILD_DIR}/AppDir"
OUTPUT_DIR="${REPO_ROOT}/dist"

echo "=== Building OPad binaries ==="
make -C "${REPO_ROOT}" all

echo "=== Assembling AppDir ==="
rm -rf "${APPDIR}"
mkdir -p "${APPDIR}/usr/bin"
mkdir -p "${APPDIR}/usr/lib/opad"
mkdir -p "${OUTPUT_DIR}"

# Copy desktop binaries
cp "${REPO_ROOT}/desktop/target/release/opad-daemon" "${APPDIR}/usr/bin/"
cp "${REPO_ROOT}/desktop/target/release/opad-gui" "${APPDIR}/usr/bin/"
cp "${REPO_ROOT}/desktop/target/release/opadctl" "${APPDIR}/usr/bin/"

# Copy AppRun and desktop metadata
cp "${SCRIPT_DIR}/AppRun" "${APPDIR}/AppRun"
chmod +x "${APPDIR}/AppRun"
cp "${SCRIPT_DIR}/opad.desktop" "${APPDIR}/opad.desktop"
cp "${SCRIPT_DIR}/install-origin" "${APPDIR}/usr/lib/opad/install-origin"
mkdir -p "${APPDIR}/etc/opad"
cp "${SCRIPT_DIR}/install-origin" "${APPDIR}/etc/opad/install-origin"

# Provide icon matching desktop entry
if [ -f "${SCRIPT_DIR}/input-keyboard.svg" ]; then
    cp "${SCRIPT_DIR}/input-keyboard.svg" "${APPDIR}/input-keyboard.svg"
    cp "${SCRIPT_DIR}/input-keyboard.svg" "${APPDIR}/opad.svg"
elif [ -f "${REPO_ROOT}/packaging/linux/icons/hicolor/scalable/apps/opad.svg" ]; then
    cp "${REPO_ROOT}/packaging/linux/icons/hicolor/scalable/apps/opad.svg" "${APPDIR}/opad.svg"
elif [ -f "${REPO_ROOT}/packaging/linux/opad.png" ]; then
    cp "${REPO_ROOT}/packaging/linux/opad.png" "${APPDIR}/opad.png"
fi

# OPad's license and the Montserrat font license (the fonts are embedded in opad-gui)
mkdir -p "${APPDIR}/usr/share/licenses/opad"
cp "${REPO_ROOT}/LICENSE" "${APPDIR}/usr/share/licenses/opad/LICENSE"
cp "${REPO_ROOT}/desktop/gui/assets/fonts/Montserrat-OFL.txt" "${APPDIR}/usr/share/licenses/opad/Montserrat-OFL.txt"

# Bundle tosu if available in build/tosu/
if [ -d "${REPO_ROOT}/build/tosu" ]; then
    echo "=== Bundling tosu ==="
    mkdir -p "${APPDIR}/usr/lib/opad/tosu"
    cp -r "${REPO_ROOT}/build/tosu/dist" "${APPDIR}/usr/lib/opad/tosu/" 2>/dev/null || true
    cp "${REPO_ROOT}/build/tosu/tosu" "${APPDIR}/usr/lib/opad/tosu/tosu" 2>/dev/null || true
    cp "${REPO_ROOT}/licenses/tosu/VERSION" "${APPDIR}/usr/lib/opad/tosu/VERSION"
    cp "${REPO_ROOT}/licenses/tosu/NOTICE" "${APPDIR}/usr/lib/opad/tosu/NOTICE"
    cp "${REPO_ROOT}/licenses/tosu/LICENSE" "${APPDIR}/usr/lib/opad/tosu/LICENSE"
    cp "${REPO_ROOT}/licenses/tosu/THIRD_PARTY_NOTICES.txt" "${APPDIR}/usr/lib/opad/tosu/THIRD_PARTY_NOTICES.txt"
    # No setcap here: the AppImage runs from a nosuid mount, which ignores file
    # capabilities. opadctl setup and the GUI explain ptrace_scope instead.
fi

# The pinned espflash (make espflash); AppRun puts usr/lib/opad/bin on PATH
if [ ! -x "${REPO_ROOT}/build/espflash/espflash" ]; then
    make -C "${REPO_ROOT}" espflash
fi
install -Dm755 "${REPO_ROOT}/build/espflash/espflash" "${APPDIR}/usr/lib/opad/bin/espflash"

# Bundle udev rules for helper installation
cp "${REPO_ROOT}/packaging/linux/udev/70-opad.rules" "${APPDIR}/usr/lib/opad/70-opad.rules"

echo "=== AppDir assembled at ${APPDIR} ==="

if command -v appimagetool >/dev/null 2>&1; then
    echo "=== Packaging AppImage with appimagetool ==="
    ARCH=x86_64 appimagetool "${APPDIR}" "${OUTPUT_DIR}/opad-x86_64.AppImage"
    echo "✓ AppImage created at ${OUTPUT_DIR}/opad-x86_64.AppImage"
else
    echo "Note: 'appimagetool' not found in PATH."
    echo "To produce the final .AppImage, run:"
    echo "  ARCH=x86_64 appimagetool ${APPDIR} ${OUTPUT_DIR}/opad-x86_64.AppImage"
fi
