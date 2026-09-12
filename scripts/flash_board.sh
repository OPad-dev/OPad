#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

echo "=================================================="
echo "  osu!pad ESP32-S3 Auto Flasher                   "
echo "=================================================="

if [ -f "${HOME}/Documents/projects/esp32/v5.5.2/esp-idf/export.sh" ]; then
    source "${HOME}/Documents/projects/esp32/v5.5.2/esp-idf/export.sh" >/dev/null 2>&1
fi

PORT="${1:-/dev/ttyACM0}"

# 1. Try 1200-baud touch first in case the firmware already has the hook
echo "[1/3] Probing device with 1200-baud auto-reset touch..."
python3 -c "
import serial, time
try:
    ser = serial.Serial('$PORT', 1200, timeout=0.2)
    time.sleep(0.1)
    ser.close()
except Exception:
    pass
" 2>/dev/null || true
sleep 1

# 2. Check if device is in ROM bootloader mode (303a:1001)
IS_BOOTLOADER=$(lsusb 2>/dev/null | grep -i "303a:1001" || true)

if [ -z "$IS_BOOTLOADER" ]; then
    echo "[2/3] Board is running application firmware (303a:4001)."
    echo "      Waiting for download mode..."
    echo "      -> Hold the BOOT button, press & release RESET, then release BOOT."
    echo ""
    while true; do
        if lsusb 2>/dev/null | grep -q -i "303a:1001"; then
            echo "✓ Detected ESP32-S3 ROM Bootloader (303a:1001)!"
            break
        fi
        sleep 0.3
    done
    sleep 1
else
    echo "[2/3] ✓ ESP32-S3 already in ROM Bootloader mode (303a:1001)!"
fi

# Find active ACM port
TARGET_PORT=$(ls /dev/ttyACM* 2>/dev/null | head -n 1 || echo "/dev/ttyACM0")
echo "[3/3] Flashing landscape firmware binary to ${TARGET_PORT}..."
cd "${REPO_ROOT}/firmware"
python -m esptool --chip esp32s3 -p "${TARGET_PORT}" -b 460800 --before=default_reset --after=hard_reset write_flash --flash_mode dio --flash_freq 80m --flash_size 16MB 0x0 build/bootloader/bootloader.bin 0x10000 build/osupad-firmware.bin 0x8000 build/partition_table/partition-table.bin

echo ""
echo "=================================================="
echo "✓ Flashing successful! Landscape UI is now active."
echo "  All future updates will flash 100% automatically"
echo "  via 'osupadctl flash' without touching buttons! "
echo "=================================================="
