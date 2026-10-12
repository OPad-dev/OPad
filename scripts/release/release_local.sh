#!/usr/bin/env bash
set -euo pipefail

# -----------------------------------------------------------------------------
# Builds every installer on a Linux machine, unsigned, into dist-local/, with
# the names the release workflows publish (docs/specs/release-and-updates.md
# §11). For testing before a tag; official releases are built by GitHub
# Actions. Everything runs in containers, so the result does not depend on
# what is installed here.
#
#   scripts/release/release_local.sh [--skip-firmware] [--skip-linux]
#                                    [--skip-windows] [--skip-tests]
#   make release-local
#
# Needs: docker (the Linux build starts containers of its own, through the
# host's docker socket), and gh (logged in) for the fallback tosu.exe.
# The Windows build downloads the Microsoft CRT and Windows SDK through
# cargo-xwin, which means accepting Microsoft's licence for them.
# -----------------------------------------------------------------------------

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
OUT="${REPO_ROOT}/dist-local"
cd "${REPO_ROOT}"

FIRMWARE=1 LINUX=1 WINDOWS=1 SKIP_TESTS=0
for arg in "$@"; do
    case "$arg" in
        --skip-firmware) FIRMWARE=0 ;;
        --skip-linux) LINUX=0 ;;
        --skip-windows) WINDOWS=0 ;;
        --skip-tests) SKIP_TESTS=1 ;;
        -h|--help) sed -n '5,19p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; exit 0 ;;
        *) echo "Unknown argument: $arg" >&2; exit 1 ;;
    esac
done

command -v docker >/dev/null || { echo "ERROR: docker is required" >&2; exit 1; }
python3 scripts/release/check_versions.py
VERSION="$(sed -n 's/^version = "\([^"]*\)".*/\1/p' desktop/Cargo.toml | head -n 1)"
FW_VERSION="$(sed -n 's/^set(PROJECT_VER "\([^"]*\)").*/\1/p' firmware/CMakeLists.txt | head -n 1)"
mkdir -p "${OUT}"
HOST_IDS="$(id -u):$(id -g)"

# Files written as root inside a container go back to the user afterwards
give_back() {
    docker run --rm -v "${REPO_ROOT}:${REPO_ROOT}" ubuntu:24.04 \
        chown -R "${HOST_IDS}" "$@" 2>/dev/null || true
}

if [ "${FIRMWARE}" = 1 ]; then
    echo "=== Firmware ${FW_VERSION} (espressif/idf:v5.5.2) ==="
    docker run --rm -v "${REPO_ROOT}:${REPO_ROOT}" -w "${REPO_ROOT}/firmware" espressif/idf:v5.5.2 \
        bash -c '. /opt/esp/idf/export.sh >/dev/null && idf.py set-target esp32s3 build'
    give_back "${REPO_ROOT}/firmware/build" "${REPO_ROOT}/firmware/managed_components" "${REPO_ROOT}/firmware/sdkconfig"
    cp firmware/build/opad-firmware.bin firmware/build/bootloader/bootloader.bin \
        firmware/build/partition_table/partition-table.bin firmware/build/ota_data_initial.bin "${OUT}/"
    cp firmware/THIRD_PARTY_NOTICES.md "${OUT}/FIRMWARE_THIRD_PARTY_NOTICES.md"
fi

if [ "${LINUX}" = 1 ]; then
    echo "=== Linux packages ${VERSION} ==="
    docker build -t opad-build-linux -f scripts/release/local/Dockerfile.linux scripts/release/local
    # The repository at the same path as here, so the containers make tosu and
    # test_packages.sh start can mount the same directories
    docker run --rm \
        -v /var/run/docker.sock:/var/run/docker.sock \
        -v "${REPO_ROOT}:${REPO_ROOT}" -w "${REPO_ROOT}" \
        -e OUT="${OUT}" -e VERSION="${VERSION}" -e SKIP_TESTS="${SKIP_TESTS}" \
        opad-build-linux scripts/release/local/build_linux.sh
    give_back "${REPO_ROOT}/build" "${REPO_ROOT}/dist" "${REPO_ROOT}/desktop/target" "${OUT}"
fi

if [ "${WINDOWS}" = 1 ]; then
    echo "=== Windows installer ${VERSION} ==="
    WIN="build/local-win"
    rm -rf "${WIN}" && mkdir -p "${WIN}/bin" "${WIN}/tosu"
    [ -d build/lvgl ] || make lvgl

    docker build -t opad-build-windows -f scripts/release/local/Dockerfile.windows scripts/release/local
    docker run --rm -v "${REPO_ROOT}:${REPO_ROOT}" -w "${REPO_ROOT}" \
        -e XWIN_ACCEPT_LICENSE=1 -e OPAD_REQUIRE_UI_PREVIEW=1 -e OPAD_LVGL_DIR="${REPO_ROOT}/build/lvgl" \
        -e CARGO_TARGET_DIR="${REPO_ROOT}/desktop/target-xwin" \
        opad-build-windows \
        cargo xwin build --release --locked --bins --target x86_64-pc-windows-msvc \
            --manifest-path desktop/Cargo.toml
    give_back "${REPO_ROOT}/desktop/target-xwin"
    cp desktop/target-xwin/x86_64-pc-windows-msvc/release/{opad-daemon,opad-gui,opadctl}.exe "${WIN}/bin/"

    # espflash, pinned and checksummed as in CI
    ESPFLASH_VERSION="$(sed -n 's/^ESPFLASH_VERSION ?= //p' Makefile)"
    ESPFLASH_WINDOWS_SHA256="b2cb4656b067716fe2b3794cf0604b461b2476d06dee7f481704cb6c8495b11c"
    curl -fsSL -o "${WIN}/espflash.zip" \
        "https://github.com/esp-rs/espflash/releases/download/v${ESPFLASH_VERSION}/espflash-x86_64-pc-windows-msvc.zip"
    echo "${ESPFLASH_WINDOWS_SHA256}  ${WIN}/espflash.zip" | sha256sum -c -
    unzip -o -q "${WIN}/espflash.zip" espflash.exe -d "${WIN}/bin"

    # tosu.exe: the latest tosu release's (CI builds it on Windows; its
    # native addon does not cross-compile yet), or TOSU_EXE=path
    if [ -n "${TOSU_EXE:-}" ]; then
        cp "${TOSU_EXE}" "${WIN}/tosu/tosu.exe"
    else
        command -v gh >/dev/null || { echo "ERROR: gh is needed for tosu.exe (or set TOSU_EXE)" >&2; exit 1; }
        tosu_tag="$(gh release list --limit 100 --json tagName -q '.[].tagName' | grep '^tosu-v' | head -n 1)"
        [ -n "${tosu_tag}" ] || { echo "ERROR: no tosu-v* release yet; run release-tosu.yml or set TOSU_EXE" >&2; exit 1; }
        gh release download "${tosu_tag}" -p tosu-windows-x86_64.exe -O "${WIN}/tosu/tosu.exe" --clobber
    fi

    [ -f dist/THIRD_PARTY_NOTICES.html ] || make notices
    # Inno Setup under Wine; paths relative to the script, Windows style
    docker run --rm -v "${REPO_ROOT}:/work" -w /work amake/innosetup \
        '/DSourceDir=..\..\build\local-win\bin' '/DTosuDir=..\..\build\local-win\tosu' \
        packaging/windows/installer.iss
    give_back "${REPO_ROOT}/build/installer"
    cp build/installer/opad-setup.exe "${OUT}/opad-setup-${VERSION}.exe"
fi

(cd "${OUT}" && rm -f SHA256SUMS && sha256sum -- * > SHA256SUMS)
echo ""
echo "✓ Unsigned build in ${OUT}:"
ls -l "${OUT}"
echo "Next: make feed-local, to update an installed OPad from these files."
