#!/usr/bin/env bash
# Download the datasheets the component notes cite (docs/hardware/components/)
# into a git-ignored folder, and check them against the recorded sha256.
# Usage: scripts/fetch_datasheets.sh
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
OUT="${SCRIPT_DIR}/../docs/hardware/components/.datasheets"
mkdir -p "${OUT}"

# name  url  sha256 (empty: not recorded yet, printed for the note)
DATASHEETS=(
    "ads7028.pdf https://www.ti.com/lit/ds/symlink/ads7028.pdf 794590931f11f1ca11bd9e8fe4683c92ccd6af7a1101d5f5158373f17f54188c"
)

status=0
for entry in "${DATASHEETS[@]}"; do
    read -r name url sum <<<"${entry}"
    dest="${OUT}/${name}"
    if [ ! -f "${dest}" ]; then
        echo "Fetching ${name}"
        curl -fsSL -A "Mozilla/5.0" -o "${dest}.part" "${url}"
        mv "${dest}.part" "${dest}"
    fi
    actual="$(sha256sum "${dest}" | cut -d' ' -f1)"
    if [ -z "${sum}" ]; then
        echo "${name}: sha256 ${actual} (record it in this script and the note)"
    elif [ "${actual}" != "${sum}" ]; then
        echo "${name}: sha256 ${actual} does not match ${sum}: a new revision? Check the note." >&2
        status=1
    else
        echo "${name}: ok"
    fi
done
exit "${status}"
