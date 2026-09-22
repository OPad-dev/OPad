#!/usr/bin/env bash
set -euo pipefail

# -----------------------------------------------------------------------------
# Complete build script for OPad (Firmware + Desktop Workspace)
# -----------------------------------------------------------------------------

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

echo "=========================================="
echo "  Building OPad Complete Project          "
echo "=========================================="

# 1. Build ESP-IDF Firmware
echo ""
echo "--- [1/2] Building ESP32-S3 Firmware ---"
if [ -n "${IDF_PATH:-}" ] && [ -f "${IDF_PATH}/export.sh" ]; then
    source "${IDF_PATH}/export.sh" >/dev/null 2>&1
elif [ -f "${HOME}/esp/esp-idf/export.sh" ]; then
    source "${HOME}/esp/esp-idf/export.sh" >/dev/null 2>&1
elif [ -f "${HOME}/export-esp.sh" ]; then
    source "${HOME}/export-esp.sh" >/dev/null 2>&1
fi

if command -v idf.py >/dev/null 2>&1; then
    cd "${REPO_ROOT}/firmware"
    idf.py build
    echo "✓ Firmware built: firmware/build/opad-firmware.bin"
else
    echo "WARNING: idf.py not found in PATH, skipping firmware compilation."
fi

# 2. Build Desktop Rust Workspace
echo ""
echo "--- [2/2] Building Desktop Rust Workspace ---"
cd "${REPO_ROOT}/desktop"
cargo build --workspace --release
echo "✓ Desktop binaries built in desktop/target/release/:"
echo "  - opad-daemon"
echo "  - opadctl"
echo "  - opad-gui"

echo ""
echo "=========================================="
echo "  Build Completed Successfully!           "
echo "=========================================="
