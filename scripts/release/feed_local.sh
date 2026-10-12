#!/usr/bin/env bash
set -euo pipefail

# -----------------------------------------------------------------------------
# Serves an update feed over the files in dist-local/ (release_local.sh), so
# an installed OPad can be updated from them without publishing anything
# (docs/specs/release-and-updates.md §11).
#
#   scripts/release/feed_local.sh [--port 8000] [--dir dist-local]
#   make feed-local
#
# The feeds are signed with the committed test key, which every build trusts
# until releases are signed for real (§5.4), and served over HTTP on
# 127.0.0.1, which the updater accepts for loopback only. Then point OPad at
# it, e.g. on Linux:
#
#   systemctl --user stop opad-daemon
#   OPAD_MANIFEST_URL=http://127.0.0.1:8000/updates/beta.json opad-daemon
#
# and in the GUI choose the beta channel and press Check now.
# Needs: cargo, minisign, python3.
# -----------------------------------------------------------------------------

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
PORT=8000
DIR="${REPO_ROOT}/dist-local"
while [ $# -gt 0 ]; do
    case "$1" in
        --port) PORT="$2"; shift 2 ;;
        --dir) DIR="$2"; shift 2 ;;
        -h|--help) sed -n '5,22p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; exit 0 ;;
        *) echo "Unknown argument: $1" >&2; exit 1 ;;
    esac
done
[ -d "${DIR}" ] || { echo "ERROR: ${DIR} does not exist; run make release-local first" >&2; exit 1; }
command -v minisign >/dev/null || { echo "ERROR: minisign is required" >&2; exit 1; }

cd "${REPO_ROOT}"
VERSION="$(sed -n 's/^version = "\([^"]*\)".*/\1/p' desktop/Cargo.toml | head -n 1)"
FW_VERSION="$(sed -n 's/^set(PROJECT_VER "\([^"]*\)").*/\1/p' firmware/CMakeLists.txt | head -n 1)"
TOSU_VERSION="$(sed -n 's/^TOSU_VERSION ?= v//p' Makefile)"

cargo build --quiet --locked -p opad-update --bin opad-feed --manifest-path desktop/Cargo.toml
FEED="desktop/target/debug/opad-feed"

"${FEED}" local --dist "${DIR}" --base-url "http://127.0.0.1:${PORT}" --out-dir "${DIR}/updates" \
    --app-version "${VERSION}" --firmware-version "${FW_VERSION}" \
    --tosu-version "${TOSU_VERSION}" --tosu-revision 1
for channel in stable beta; do
    rm -f "${DIR}/updates/${channel}.json.minisig"
    minisign -S -s desktop/crates/opad-update/test-key/opad-test.key \
        -m "${DIR}/updates/${channel}.json" -t "OPad local ${channel} feed (TEST KEY)" < /dev/null
    "${FEED}" verify --feed "${DIR}/updates/${channel}.json" --channel "${channel}"
done

echo ""
echo "Serving ${DIR} on http://127.0.0.1:${PORT} (Ctrl+C to stop)"
echo "  OPAD_MANIFEST_URL=http://127.0.0.1:${PORT}/updates/beta.json"
exec python3 -m http.server "${PORT}" --bind 127.0.0.1 --directory "${DIR}"
