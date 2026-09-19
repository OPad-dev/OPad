#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

if [ -f "${HOME}/Documents/projects/esp32/v5.5.2/esp-idf/export.sh" ]; then
    source "${HOME}/Documents/projects/esp32/v5.5.2/esp-idf/export.sh" >/dev/null 2>&1
elif [ -f "${HOME}/export-esp.sh" ]; then
    source "${HOME}/export-esp.sh" >/dev/null 2>&1
fi

NANOPB_GEN=""
if command -v nanopb_generator.py >/dev/null 2>&1; then
    NANOPB_GEN="nanopb_generator.py"
elif command -v nanopb_generator >/dev/null 2>&1; then
    NANOPB_GEN="nanopb_generator"
else
    PY_NANOPB=$(find "${HOME}/.espressif/python_env" -name "nanopb_generator.py" 2>/dev/null | head -n 1 || true)
    if [ -n "${PY_NANOPB}" ] && [ -f "${PY_NANOPB}" ]; then
        NANOPB_GEN="python3 ${PY_NANOPB}"
    fi
fi

if [ -z "${NANOPB_GEN}" ]; then
    echo "ERROR: nanopb_generator.py not found!" >&2
    exit 1
fi

echo "Generating protobuf files with: ${NANOPB_GEN}"
${NANOPB_GEN} -L '#include "nanopb/%s"' -I "${REPO_ROOT}/protocol" -D "${REPO_ROOT}/firmware/main/protocol" "${REPO_ROOT}/protocol/opad.proto"
echo "Done."
