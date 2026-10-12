#!/usr/bin/env bash
set -euo pipefail

# -----------------------------------------------------------------------------
# Runs inside Dockerfile.linux (started by release_local.sh): the steps of
# release-app.yml's Linux job, writing the staged files to $OUT.
# -----------------------------------------------------------------------------

: "${OUT:?}" "${VERSION:?}"
ESPFLASH_VERSION="${ESPFLASH_VERSION:-4.6.0}"
export OPAD_REQUIRE_UI_PREVIEW=1

make tosu
make lvgl
export OPAD_LVGL_DIR="$PWD/build/lvgl"

cargo zigbuild --release --locked --bins \
    --manifest-path desktop/Cargo.toml --target x86_64-unknown-linux-gnu.2.31
mkdir -p desktop/target/release
cp desktop/target/x86_64-unknown-linux-gnu/release/{opad-daemon,opad-gui,opadctl} desktop/target/release/

# Upstream's Linux espflash needs glibc 2.34; the packages carry one built
# against 2.31, as in CI
if [ ! -x build/espflash/espflash ]; then
    rm -rf build/espflash-src
    git clone --depth 1 --branch "v${ESPFLASH_VERSION}" https://github.com/esp-rs/espflash.git build/espflash-src
    (cd build/espflash-src && cargo zigbuild --release --locked -p espflash --target x86_64-unknown-linux-gnu.2.31)
    mkdir -p build/espflash
    install -m 755 build/espflash-src/target/x86_64-unknown-linux-gnu/release/espflash build/espflash/espflash
fi

OPAD_PREBUILT=1 ./scripts/release/build_packages.sh

if [ "${SKIP_TESTS:-0}" != 1 ]; then
    scripts/release/test_packages.sh ubuntu:22.04 debian:12 fedora:43
fi

make notices
make tosu-source
make lazer-calculator-source

# The same names release-app.yml publishes
rm -rf build/local-tar && mkdir -p build/local-tar/bin "$OUT"
cp desktop/target/release/{opad-daemon,opad-gui,opadctl} build/local-tar/bin/
cp README.md LICENSE dist/THIRD_PARTY_NOTICES.html build/local-tar/
tar -czf "$OUT/opad-linux-x86_64-${VERSION}.tar.gz" -C build/local-tar .
cp dist/*.deb "$OUT/opad_${VERSION}_amd64.deb"
cp dist/*.rpm "$OUT/opad-${VERSION}.x86_64.rpm"
cp dist/*.AppImage "$OUT/opad-${VERSION}-x86_64.AppImage"
cp dist/THIRD_PARTY_NOTICES.html dist/*-source.tar.gz "$OUT/"
