#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../../.." && pwd)"
FW_MAIN="${REPO_ROOT}/firmware/main"
HOST_TEST_DIR="${SCRIPT_DIR}"
BIN_DIR="${HOST_TEST_DIR}/bin"

mkdir -p "${BIN_DIR}"
cleanup() {
    rm -rf "${BIN_DIR}"
}
trap cleanup EXIT

echo "=== Building and Running Firmware Host Unit Tests ==="

# 1. Debounce tests (P0-2)
echo "[1/6] Running test_debounce..."
gcc -Wall -Wextra -Werror -I "${FW_MAIN}" \
    "${FW_MAIN}/input/debounce.c" \
    "${HOST_TEST_DIR}/test_debounce.c" \
    -o "${BIN_DIR}/test_debounce"
"${BIN_DIR}/test_debounce"

# 2. Counter sync rules tests (P1-1 / §13)
echo "[2/6] Running test_counters..."
gcc -Wall -Wextra -Werror -I "${FW_MAIN}/counters" \
    "${FW_MAIN}/counters/counter_sync_rules.c" \
    "${HOST_TEST_DIR}/test_counters.c" \
    -o "${BIN_DIR}/test_counters"
"${BIN_DIR}/test_counters"

# 3. Config validation tests (P0-3)
echo "[3/6] Running test_config..."
gcc -Wall -Wextra -Werror -I "${FW_MAIN}" -I "${FW_MAIN}/config" \
    "${FW_MAIN}/config/config_validate.c" \
    "${HOST_TEST_DIR}/test_config.c" \
    -o "${BIN_DIR}/test_config"
# Again with the bench debug GPIO on one of the header pins
gcc -Wall -Wextra -Werror -I "${FW_MAIN}" -I "${FW_MAIN}/config" \
    -DCONFIG_OSUPAD_BENCH_DEBUG_GPIO=1 -DCONFIG_OSUPAD_BENCH_DEBUG_GPIO_NUM=4 \
    "${FW_MAIN}/config/config_validate.c" \
    "${HOST_TEST_DIR}/test_config.c" \
    -o "${BIN_DIR}/test_config_debug_gpio"
"${BIN_DIR}/test_config_debug_gpio"
"${BIN_DIR}/test_config"

# 4. Diag ring buffer tests (P2-1)
echo "[4/6] Running test_diag..."
gcc -Wall -Wextra -Werror -I "${FW_MAIN}" \
    "${FW_MAIN}/diag/diag.c" \
    "${HOST_TEST_DIR}/test_diag.c" \
    -o "${BIN_DIR}/test_diag"
"${BIN_DIR}/test_diag"

# 5. Protocol frame parser tests
echo "[5/6] Running test_frame_parser..."
gcc -Wall -Wextra -Werror -I "${FW_MAIN}" -I "${FW_MAIN}/protocol" \
    "${FW_MAIN}/protocol/frame_parser.c" \
    "${HOST_TEST_DIR}/test_frame_parser.c" \
    -o "${BIN_DIR}/test_frame_parser"
"${BIN_DIR}/test_frame_parser"

# 6. Pad ownership decision tests (W3-2)
echo "[6/6] Running test_owner..."
gcc -Wall -Wextra -Werror -I "${FW_MAIN}" \
    "${FW_MAIN}/config/owner.c" \
    "${HOST_TEST_DIR}/test_owner.c" \
    -o "${BIN_DIR}/test_owner"
"${BIN_DIR}/test_owner"

echo "=== All firmware host unit tests passed! ==="
