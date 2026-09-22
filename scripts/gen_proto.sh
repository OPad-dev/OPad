#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

if [ -n "${IDF_PATH:-}" ] && [ -f "${IDF_PATH}/export.sh" ]; then
    source "${IDF_PATH}/export.sh" >/dev/null 2>&1
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
${NANOPB_GEN} -L '#include "nanopb/%s"' -I "${REPO_ROOT}/protocol" -D "${REPO_ROOT}/firmware/main/protocol" "${REPO_ROOT}/protocol/osupad.proto"
echo "Done."
