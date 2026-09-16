#!/usr/bin/env bash
set -euo pipefail

# -----------------------------------------------------------------------------
# Release build script for osu!pad v1.0 (Linux x86_64)
# Builds ESP32-S3 firmware .bin, release host binaries, packages, and SHA256SUMS
# -----------------------------------------------------------------------------

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
DIST_DIR="${REPO_ROOT}/dist"

echo "=========================================="
echo "  osu!pad v1.0 Release Build              "
echo "=========================================="

rm -rf "${DIST_DIR}"
mkdir -p "${DIST_DIR}"

# 1. Build Firmware (.bin)
echo ""
echo "--- [1/3] Building ESP32-S3 Firmware ---"
if [ -f "${HOME}/Documents/projects/esp32/v5.5.2/esp-idf/export.sh" ]; then
    source "${HOME}/Documents/projects/esp32/v5.5.2/esp-idf/export.sh" >/dev/null 2>&1
elif [ -f "${HOME}/export-esp.sh" ]; then
    source "${HOME}/export-esp.sh" >/dev/null 2>&1
fi

if ! command -v idf.py >/dev/null 2>&1; then
    echo "ERROR: idf.py not found in PATH or environment!" >&2
    exit 1
fi

cd "${REPO_ROOT}/firmware"
idf.py build

FW_BIN="${REPO_ROOT}/firmware/build/osupad-firmware.bin"
BOOT_BIN="${REPO_ROOT}/firmware/build/bootloader/bootloader.bin"
PART_BIN="${REPO_ROOT}/firmware/build/partition_table/partition-table.bin"
# Points the bootloader back at ota_0 (§U-3a). Without it a recovery flash onto
# an erased chip leaves otadata blank, which happens to boot ota_0 anyway, but
# only by falling back rather than by being told.
OTA_BIN="${REPO_ROOT}/firmware/build/ota_data_initial.bin"

if [ ! -f "${FW_BIN}" ]; then
    echo "ERROR: Firmware binary not found at ${FW_BIN}!" >&2
    exit 1
fi

cp "${FW_BIN}" "${DIST_DIR}/osupad-firmware.bin"
[ -f "${BOOT_BIN}" ] && cp "${BOOT_BIN}" "${DIST_DIR}/bootloader.bin"
[ -f "${PART_BIN}" ] && cp "${PART_BIN}" "${DIST_DIR}/partition-table.bin"
[ -f "${OTA_BIN}" ] && cp "${OTA_BIN}" "${DIST_DIR}/ota_data_initial.bin"
echo "✓ Firmware artifacts copied to dist/"

# 2. Build Desktop Host Binaries
echo ""
echo "--- [2/3] Building Desktop Release Binaries ---"
cd "${REPO_ROOT}/desktop"
cargo build --workspace --release

TARGET_RELEASE="${REPO_ROOT}/desktop/target/release"
for bin in osupad-daemon osupadctl osupad-gui; do
    if [ ! -f "${TARGET_RELEASE}/${bin}" ]; then
        echo "ERROR: Binary ${bin} not found at ${TARGET_RELEASE}/${bin}!" >&2
        exit 1
    fi
    cp "${TARGET_RELEASE}/${bin}" "${DIST_DIR}/${bin}"
    if command -v strip >/dev/null 2>&1; then
        strip "${DIST_DIR}/${bin}"
    fi
done
echo "✓ Host binaries copied and stripped in dist/"

# Create tarball archive for Linux distribution
ARCHIVE_NAME="osupad-linux-x86_64-1.0.0.tar.gz"
TAR_TMP="${DIST_DIR}/tar_staging"
mkdir -p "${TAR_TMP}/bin"
cp "${DIST_DIR}/osupad-daemon" "${DIST_DIR}/osupad-gui" "${DIST_DIR}/osupadctl" "${TAR_TMP}/bin/"
mkdir -p "${TAR_TMP}/packaging"
cp -r "${REPO_ROOT}/packaging/"* "${TAR_TMP}/packaging/"
cp "${REPO_ROOT}/README.md" "${REPO_ROOT}/LICENSE" "${TAR_TMP}/"

tar -czf "${DIST_DIR}/${ARCHIVE_NAME}" -C "${TAR_TMP}" .
rm -rf "${TAR_TMP}"
echo "✓ Release archive created: dist/${ARCHIVE_NAME}"

# 3. Generate SHA256SUMS
echo ""
echo "--- [3/3] Generating SHA256SUMS ---"
cd "${DIST_DIR}"
sha256sum * > SHA256SUMS
echo "✓ Checksums generated:"
cat SHA256SUMS

echo ""
echo "=========================================="
echo "  Release Build Finished Successfully!   "
echo "  Artifacts available in: ${DIST_DIR}     "
echo "=========================================="
