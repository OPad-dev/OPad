#!/usr/bin/env bash
set -euo pipefail

# -----------------------------------------------------------------------------
# Release build script for OPad v1.0 (Linux x86_64)
# Builds ESP32-S3 firmware .bin, release host binaries, packages, and SHA256SUMS
# -----------------------------------------------------------------------------

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
DIST_DIR="${REPO_ROOT}/dist"

echo "=========================================="
echo "  OPad v1.0 Release Build                 "
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

FW_BIN="${REPO_ROOT}/firmware/build/opad-firmware.bin"
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

cp "${FW_BIN}" "${DIST_DIR}/opad-firmware.bin"
[ -f "${BOOT_BIN}" ] && cp "${BOOT_BIN}" "${DIST_DIR}/bootloader.bin"
[ -f "${PART_BIN}" ] && cp "${PART_BIN}" "${DIST_DIR}/partition-table.bin"
[ -f "${OTA_BIN}" ] && cp "${OTA_BIN}" "${DIST_DIR}/ota_data_initial.bin"
cp "${REPO_ROOT}/firmware/THIRD_PARTY_NOTICES.md" "${DIST_DIR}/FIRMWARE_THIRD_PARTY_NOTICES.md"
echo "✓ Firmware artifacts copied to dist/"

# 2. Build Desktop Host Binaries
echo ""
echo "--- [2/3] Building Desktop Release Binaries ---"
cd "${REPO_ROOT}/desktop"
cargo build --workspace --release

TARGET_RELEASE="${REPO_ROOT}/desktop/target/release"
for bin in opad-daemon opadctl opad-gui; do
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

# Third-party notices shipped with every archive: the Rust crates (cargo-about,
# desktop/about.toml) and the firmware's components
if ! cargo about --version >/dev/null 2>&1; then
    echo "ERROR: cargo-about is required (cargo install cargo-about --locked --features cli)" >&2
    exit 1
fi
(cd "${REPO_ROOT}/desktop" && cargo about generate --offline --fail about.hbs \
    -o "${DIST_DIR}/THIRD_PARTY_NOTICES.html")
echo "✓ THIRD_PARTY_NOTICES.html generated"

# Create tarball archive for Linux distribution. The version comes from the
# workspace rather than a literal: it reads 1.0.0-rc until W4 passes (§0), and a
# tarball claiming 1.0.0 while the binaries inside report 1.0.0-rc is the kind of
# mismatch nobody notices until a bug report cites the wrong version.
VERSION="$(sed -n 's/^version = "\(.*\)"$/\1/p' "${REPO_ROOT}/desktop/Cargo.toml" | head -n 1)"
if [ -z "${VERSION}" ]; then
    echo "ERROR: could not read the workspace version from desktop/Cargo.toml!" >&2
    exit 1
fi
ARCHIVE_NAME="opad-linux-x86_64-${VERSION}.tar.gz"
TAR_TMP="${DIST_DIR}/tar_staging"
mkdir -p "${TAR_TMP}/bin"
cp "${DIST_DIR}/opad-daemon" "${DIST_DIR}/opad-gui" "${DIST_DIR}/opadctl" "${TAR_TMP}/bin/"
mkdir -p "${TAR_TMP}/packaging"
cp -r "${REPO_ROOT}/packaging/"* "${TAR_TMP}/packaging/"
cp "${REPO_ROOT}/README.md" "${REPO_ROOT}/LICENSE" "${TAR_TMP}/"
cp "${DIST_DIR}/THIRD_PARTY_NOTICES.html" "${TAR_TMP}/"

tar -czf "${DIST_DIR}/${ARCHIVE_NAME}" -C "${TAR_TMP}" .
rm -rf "${TAR_TMP}"
echo "✓ Release archive created: dist/${ARCHIVE_NAME}"

# 3. Generate SHA256SUMS
echo ""
echo "--- [3/4] Generating SHA256SUMS ---"
cd "${DIST_DIR}"
sha256sum * > SHA256SUMS
echo "✓ Checksums generated:"
cat SHA256SUMS

# 4. Generate and sign release manifest (§U-0.3)
KEY_FILE="${HOME}/.config/opad/opad-manifest.key"
BASE_URL="${BASE_URL:-https://github.com/OPad-dev/OPad/releases/latest/download}"
FW_VER="${FIRMWARE_VERSION:-1.0.0}"
if [ -f "${KEY_FILE}" ]; then
    echo ""
    echo "--- [4/4] Generating signed release manifest ---"
    cargo run --manifest-path "${REPO_ROOT}/desktop/Cargo.toml" -p opad-update --bin opad-manifest -- \
        --dist "${DIST_DIR}" \
        --base-url "${BASE_URL}" \
        --firmware-version "${FW_VER}"
    echo "✓ Release manifest and signature created in dist/"
else
    echo "Note: Secret key not found at ${KEY_FILE}; skipping manifest signing."
fi

echo ""
echo "=========================================="
echo "  Release Build Finished Successfully!   "
echo "  Artifacts available in: ${DIST_DIR}     "
echo "=========================================="
