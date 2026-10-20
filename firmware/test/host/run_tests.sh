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
echo "[1/12] Running test_debounce..."
gcc -Wall -Wextra -Werror -I "${FW_MAIN}" \
    "${FW_MAIN}/input/debounce.c" \
    "${HOST_TEST_DIR}/test_debounce.c" \
    -o "${BIN_DIR}/test_debounce"
"${BIN_DIR}/test_debounce"

# 2. Counter sync rules tests (P1-1 / §13)
echo "[2/12] Running test_counters..."
gcc -Wall -Wextra -Werror -I "${FW_MAIN}/counters" \
    "${FW_MAIN}/counters/counter_sync_rules.c" \
    "${HOST_TEST_DIR}/test_counters.c" \
    -o "${BIN_DIR}/test_counters"
"${BIN_DIR}/test_counters"

# 3. Config validation tests (P0-3)
echo "[3/12] Running test_config..."
gcc -Wall -Wextra -Werror -I "${FW_MAIN}" -I "${FW_MAIN}/config" \
    "${FW_MAIN}/config/config_validate.c" \
    "${FW_MAIN}/config/he_config_record.c" \
    "${HOST_TEST_DIR}/test_config.c" \
    -o "${BIN_DIR}/test_config"
# Again with the bench debug GPIO on one of the header pins
gcc -Wall -Wextra -Werror -I "${FW_MAIN}" -I "${FW_MAIN}/config" \
    -DCONFIG_OSUPAD_BENCH_DEBUG_GPIO=1 -DCONFIG_OSUPAD_BENCH_DEBUG_GPIO_NUM=4 \
    "${FW_MAIN}/config/config_validate.c" \
    "${FW_MAIN}/config/he_config_record.c" \
    "${HOST_TEST_DIR}/test_config.c" \
    -o "${BIN_DIR}/test_config_debug_gpio"
"${BIN_DIR}/test_config_debug_gpio"
"${BIN_DIR}/test_config"

# 4. Diag ring buffer tests (P2-1)
echo "[4/12] Running test_diag..."
gcc -Wall -Wextra -Werror -I "${FW_MAIN}" \
    "${FW_MAIN}/diag/diag.c" \
    "${HOST_TEST_DIR}/test_diag.c" \
    -o "${BIN_DIR}/test_diag"
"${BIN_DIR}/test_diag"

# 5. Protocol frame parser tests
echo "[5/12] Running test_frame_parser..."
gcc -Wall -Wextra -Werror -I "${FW_MAIN}" -I "${FW_MAIN}/protocol" \
    "${FW_MAIN}/protocol/frame_parser.c" \
    "${HOST_TEST_DIR}/test_frame_parser.c" \
    -o "${BIN_DIR}/test_frame_parser"
"${BIN_DIR}/test_frame_parser"

# 6. Pad ownership decision tests (W3-2)
echo "[6/12] Running test_owner..."
gcc -Wall -Wextra -Werror -I "${FW_MAIN}" \
    "${FW_MAIN}/config/owner.c" \
    "${HOST_TEST_DIR}/test_owner.c" \
    -o "${BIN_DIR}/test_owner"
"${BIN_DIR}/test_owner"

# 7. Key press log (tap rate, issue #2)
echo "[7/12] Running test_press_log..."
gcc -Wall -Wextra -Werror -I "${FW_MAIN}" \
    "${FW_MAIN}/input/press_log.c" \
    "${HOST_TEST_DIR}/test_press_log.c" \
    -o "${BIN_DIR}/test_press_log"
"${BIN_DIR}/test_press_log"

# 8. Touchscreen swipe classifier
echo "[8/12] Running test_gesture..."
gcc -Wall -Wextra -Werror -I "${FW_MAIN}" \
    "${FW_MAIN}/input/gesture.c" \
    "${HOST_TEST_DIR}/test_gesture.c" \
    -o "${BIN_DIR}/test_gesture"
"${BIN_DIR}/test_gesture"

# 9. Swipe feedback labels
echo "[9/12] Running test_swipe_labels..."
gcc -Wall -Wextra -Werror -I "${FW_MAIN}" \
    "${FW_MAIN}/input/swipe_labels.c" \
    "${HOST_TEST_DIR}/test_swipe_labels.c" \
    -o "${BIN_DIR}/test_swipe_labels"
"${BIN_DIR}/test_swipe_labels"

# 10. Hall Effect decision engine and settings builder
echo "[10/16] Running test_he_rt..."
gcc -Wall -Wextra -Werror -I "${FW_MAIN}" \
    "${FW_MAIN}/input/he_rt.c" \
    "${FW_MAIN}/input/he_params.c" \
    "${HOST_TEST_DIR}/test_he_rt.c" \
    -o "${BIN_DIR}/test_he_rt"
"${BIN_DIR}/test_he_rt"

# 11. Input module swapped while powered
echo "[11/16] Running test_module_watch..."
gcc -Wall -Wextra -Werror -I "${FW_MAIN}" \
    "${FW_MAIN}/input/module_watch.c" \
    "${HOST_TEST_DIR}/test_module_watch.c" \
    -o "${BIN_DIR}/test_module_watch"
"${BIN_DIR}/test_module_watch"

# 12. Hall Effect DMA frame parser
echo "[12/16] Running test_he_frame..."
gcc -Wall -Wextra -Werror -I "${FW_MAIN}" \
    "${FW_MAIN}/input/he_frame.c" \
    "${HOST_TEST_DIR}/test_he_frame.c" \
    -o "${BIN_DIR}/test_he_frame"
"${BIN_DIR}/test_he_frame"

# 13. Hall Effect calibration record tests
echo "[13/16] Running test_he_cal..."
gcc -Wall -Wextra -Werror -I "${FW_MAIN}" \
    "${FW_MAIN}/input/he_cal_record.c" \
    "${HOST_TEST_DIR}/test_he_cal.c" \
    -o "${BIN_DIR}/test_he_cal"
"${BIN_DIR}/test_he_cal"

# 14. Hall Effect calibration capture algorithm tests
echo "[14/16] Running test_he_cal_capture..."
gcc -Wall -Wextra -Werror -I "${FW_MAIN}" \
    "${FW_MAIN}/input/he_cal_capture.c" \
    "${HOST_TEST_DIR}/test_he_cal_capture.c" \
    -o "${BIN_DIR}/test_he_cal_capture"
"${BIN_DIR}/test_he_cal_capture"

# 15. Keypad config staging rule tests
echo "[15/16] Running test_keypad_staged..."
gcc -Wall -Wextra -Werror -I "${FW_MAIN}" \
    "${HOST_TEST_DIR}/test_keypad_staged.c" \
    -o "${BIN_DIR}/test_keypad_staged"
"${BIN_DIR}/test_keypad_staged"

# 16. Hall Effect health state machine tests
echo "[16/16] Running test_he_health..."
gcc -Wall -Wextra -Werror -I "${FW_MAIN}" \
    "${FW_MAIN}/input/he_health.c" \
    "${FW_MAIN}/input/he_rt.c" \
    "${FW_MAIN}/input/he_params.c" \
    "${HOST_TEST_DIR}/test_he_health.c" \
    -o "${BIN_DIR}/test_he_health"
"${BIN_DIR}/test_he_health"

echo "=== All firmware host unit tests passed! ==="
