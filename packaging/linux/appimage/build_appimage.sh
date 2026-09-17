#!/usr/bin/env bash
set -euo pipefail

# -----------------------------------------------------------------------------
# osu!pad AppImage packaging script (L-4)
# Assembles AppDir and builds osupad-x86_64.AppImage
# -----------------------------------------------------------------------------

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../../.." && pwd)"
BUILD_DIR="${REPO_ROOT}/build/appimage"
APPDIR="${BUILD_DIR}/AppDir"
OUTPUT_DIR="${REPO_ROOT}/dist"

echo "=== Building osu!pad binaries ==="
make -C "${REPO_ROOT}" all

echo "=== Assembling AppDir ==="
rm -rf "${APPDIR}"
mkdir -p "${APPDIR}/usr/bin"
mkdir -p "${APPDIR}/usr/lib/osupad"
mkdir -p "${OUTPUT_DIR}"

# Copy desktop binaries
cp "${REPO_ROOT}/desktop/target/release/osupad-daemon" "${APPDIR}/usr/bin/"
cp "${REPO_ROOT}/desktop/target/release/osupad-gui" "${APPDIR}/usr/bin/"
cp "${REPO_ROOT}/desktop/target/release/osupadctl" "${APPDIR}/usr/bin/"

# Copy AppRun and desktop metadata
cp "${SCRIPT_DIR}/AppRun" "${APPDIR}/AppRun"
chmod +x "${APPDIR}/AppRun"
cp "${SCRIPT_DIR}/osupad.desktop" "${APPDIR}/osupad.desktop"
cp "${SCRIPT_DIR}/install-origin" "${APPDIR}/usr/lib/osupad/install-origin"
mkdir -p "${APPDIR}/etc/osupad"
cp "${SCRIPT_DIR}/install-origin" "${APPDIR}/etc/osupad/install-origin"

# Provide icon matching desktop entry
if [ -f "${SCRIPT_DIR}/input-keyboard.svg" ]; then
    cp "${SCRIPT_DIR}/input-keyboard.svg" "${APPDIR}/input-keyboard.svg"
    cp "${SCRIPT_DIR}/input-keyboard.svg" "${APPDIR}/osupad.svg"
elif [ -f "${REPO_ROOT}/packaging/linux/icons/hicolor/scalable/apps/osupad.svg" ]; then
    cp "${REPO_ROOT}/packaging/linux/icons/hicolor/scalable/apps/osupad.svg" "${APPDIR}/osupad.svg"
elif [ -f "${REPO_ROOT}/packaging/linux/osupad.png" ]; then
    cp "${REPO_ROOT}/packaging/linux/osupad.png" "${APPDIR}/osupad.png"
fi

# Bundle tosu if available in build/tosu/
if [ -d "${REPO_ROOT}/build/tosu" ]; then
    echo "=== Bundling tosu ==="
    mkdir -p "${APPDIR}/usr/lib/osupad/tosu"
    cp -r "${REPO_ROOT}/build/tosu/dist" "${APPDIR}/usr/lib/osupad/tosu/" 2>/dev/null || true
    cp "${REPO_ROOT}/build/tosu/tosu" "${APPDIR}/usr/lib/osupad/tosu/tosu" 2>/dev/null || true
    cp "${REPO_ROOT}/licenses/tosu/VERSION" "${APPDIR}/usr/lib/osupad/tosu/VERSION"
    cp "${REPO_ROOT}/licenses/tosu/NOTICE" "${APPDIR}/usr/lib/osupad/tosu/NOTICE"
    cp "${REPO_ROOT}/licenses/tosu/LICENSE" "${APPDIR}/usr/lib/osupad/tosu/LICENSE"
fi

echo "=== AppDir assembled at ${APPDIR} ==="

if command -v appimagetool >/dev/null 2>&1; then
    echo "=== Packaging AppImage with appimagetool ==="
    ARCH=x86_64 appimagetool "${APPDIR}" "${OUTPUT_DIR}/osupad-x86_64.AppImage"
    echo "✓ AppImage created at ${OUTPUT_DIR}/osupad-x86_64.AppImage"
else
    echo "Note: 'appimagetool' not found in PATH."
    echo "To produce the final .AppImage, run:"
    echo "  ARCH=x86_64 appimagetool ${APPDIR} ${OUTPUT_DIR}/osupad-x86_64.AppImage"
fi
