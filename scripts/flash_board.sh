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

ESPTOOL="${HOME}/.espressif/python_env/idf5.5_py3.14_env/bin/python -m esptool"
BUILD_DIR="${REPO_ROOT}/firmware/build"

if [ ! -f "${BUILD_DIR}/osupad-firmware.bin" ]; then
    echo "Error: Firmware binary not found at ${BUILD_DIR}/osupad-firmware.bin"
    echo "Building firmware now..."
    source "${HOME}/Documents/projects/esp32/v5.5.2/esp-idf/export.sh" >/dev/null 2>&1
    idf.py -C "${REPO_ROOT}/firmware" build
fi

find_esp_port() {
    # Check tty devices with Espressif vendor ID (303a)
    for p in /dev/ttyACM0 /dev/ttyACM1 /dev/ttyACM2 /dev/ttyUSB0 /dev/ttyUSB1; do
        if [ -c "$p" ]; then
            local vid
            vid=$(udevadm info -q property -n "$p" 2>/dev/null | grep "^ID_VENDOR_ID=" | cut -d= -f2 || true)
            if [ "$vid" = "303a" ]; then
                echo "$p"
                return 0
            fi
        fi
    done
    for p in /dev/ttyACM0 /dev/ttyACM1 /dev/ttyACM2; do
        if [ -c "$p" ]; then
            echo "$p"
            return 0
        fi
    done
    return 1
}

# 1. Determine current state
CURRENT_PORT=$(find_esp_port || echo "/dev/ttyACM0")
IS_BOOTLOADER=$(lsusb 2>/dev/null | grep -i "303a:1001" || true)

if [ -z "$IS_BOOTLOADER" ]; then
    echo "[1/3] Device running application on ${CURRENT_PORT} (303a:4001)."
    echo "      Sending 1200-baud auto-reset touch to trigger bootloader..."
    
    python3 -c "
import serial, time
try:
    ser = serial.Serial('$CURRENT_PORT', 1200, timeout=0.1)
    time.sleep(0.05)
    ser.close()
except Exception:
    pass
" 2>/dev/null || true

    # Wait up to 2.5s for bootloader mode
    for i in {1..12}; do
        sleep 0.2
        if lsusb 2>/dev/null | grep -q -i "303a:1001"; then
            IS_BOOTLOADER="yes"
            break
        fi
    done
fi

if [ -z "$IS_BOOTLOADER" ]; then
    echo ""
    echo "[2/3] NOTE: The board is currently running the previous firmware build"
    echo "      which does not have the auto-bootloader hook."
    echo ""
    echo "      To flash this update (REQUIRED ONCE):"
    echo "      1. Press and hold the BOOT button"
    echo "      2. Press and release the RESET button"
    echo "      3. Release the BOOT button"
    echo ""
    echo "      Once this update is flashed, ALL future updates will reboot"
    echo "      and flash 100% automatically without touching any buttons."
    echo ""
    echo "Waiting for ESP32-S3 ROM Bootloader (303a:1001)..."
    while true; do
        if lsusb 2>/dev/null | grep -q -i "303a:1001"; then
            echo "✓ ESP32-S3 ROM Bootloader detected!"
            break
        fi
        sleep 0.3
    done
else
    echo "[2/3] ✓ ESP32-S3 ROM Bootloader (303a:1001) detected!"
fi

# Wait for Linux udev device node to be created and settled
echo "Waiting for serial port to settle..."
sleep 1.5

TARGET_PORT=""
for i in {1..30}; do
    TARGET_PORT=$(find_esp_port || true)
    if [ -n "$TARGET_PORT" ] && [ -c "$TARGET_PORT" ]; then
        break
    fi
    sleep 0.2
done

if [ -z "$TARGET_PORT" ]; then
    TARGET_PORT="/dev/ttyACM0"
fi

echo "[3/3] Port ready: ${TARGET_PORT}. Flashing landscape firmware binary..."
cd "${REPO_ROOT}/firmware"

# Use --before=usb_reset to properly synchronize ESP32-S3 native USB-Serial/JTAG
if ! $ESPTOOL --chip esp32s3 -p "${TARGET_PORT}" -b 460800 --before=usb_reset --after=hard_reset write_flash \
    --flash_mode dio --flash_freq 80m --flash_size 16MB \
    0x0 "${BUILD_DIR}/bootloader/bootloader.bin" \
    0x10000 "${BUILD_DIR}/osupad-firmware.bin" \
    0x8000 "${BUILD_DIR}/partition_table/partition-table.bin"; then
    echo "Retrying with espflash..."
    espflash write-bin --chip esp32s3 -p "${TARGET_PORT}" --before usb-reset --non-interactive 0x10000 "${BUILD_DIR}/osupad-firmware.bin"
fi

echo ""
echo "=================================================="
echo "✓ Flashing successful! Landscape UI is now active."
echo "  The board is now running firmware with full auto-reset."
echo "  All future flashes will run hands-free automatically!"
echo "=================================================="
