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

# 1. Check if device is already in ROM bootloader (303a:1001)
get_bootloader_port() {
    for p in /dev/ttyACM0 /dev/ttyACM1 /dev/ttyACM2 /dev/ttyUSB0; do
        if [ -c "$p" ]; then
            local model_id vid
            vid=$(udevadm info -q property -n "$p" 2>/dev/null | grep "^ID_VENDOR_ID=" | cut -d= -f2 || true)
            model_id=$(udevadm info -q property -n "$p" 2>/dev/null | grep "^ID_MODEL_ID=" | cut -d= -f2 || true)
            if [ "$vid" = "303a" ] && [ "$model_id" = "1001" ]; then
                echo "$p"
                return 0
            fi
        fi
    done
    return 1
}

CURRENT_PORT=$(find_esp_port || echo "/dev/ttyACM0")
BOOT_PORT=$(get_bootloader_port || true)

if [ -z "$BOOT_PORT" ]; then
    echo "[1/3] Device running application on ${CURRENT_PORT} (303a:4001)."
    echo "      Attempting 1200-baud auto-reset touch..."
    python3 -c "
import serial, time
try:
    ser = serial.Serial('$CURRENT_PORT', 1200, timeout=0.1)
    time.sleep(0.05)
    ser.close()
except Exception:
    pass
" 2>/dev/null || true

    for i in {1..10}; do
        sleep 0.15
        BOOT_PORT=$(get_bootloader_port || true)
        if [ -n "$BOOT_PORT" ]; then
            break
        fi
    done
fi

if [ -z "$BOOT_PORT" ]; then
    echo ""
    echo "[2/3] The board currently has the previous build without the auto-reset hook."
    echo "      To enter download mode to flash the landscape firmware (REQUIRED ONCE):"
    echo ""
    echo "      >>> METHOD 1 (Recommended by Waveshare) <<<"
    echo "      1. Unplug the USB cable from the ESP32-S3."
    echo "      2. Hold down the BOOT button on the board."
    echo "      3. Plug the USB cable back in while holding BOOT."
    echo "      4. Release the BOOT button."
    echo ""
    echo "      >>> METHOD 2 (Hardware Buttons) <<<"
    echo "      1. Press and hold the BOOT button."
    echo "      2. Press and release the RESET button."
    echo "      3. Keep holding BOOT for 1 second, then release."
    echo ""
    echo "      Once this update is flashed, ALL future flashes will run"
    echo "      100% automatically without touching buttons or cables!"
    echo ""
    echo "Waiting for ESP32-S3 ROM Bootloader (303a:1001)..."

    while true; do
        BOOT_PORT=$(get_bootloader_port || true)
        if [ -n "$BOOT_PORT" ]; then
            echo "✓ ESP32-S3 ROM Bootloader detected on ${BOOT_PORT}!"
            break
        fi
        sleep 0.1
    done
else
    echo "[2/3] ✓ ESP32-S3 ROM Bootloader detected on ${BOOT_PORT}!"
fi

echo "[3/3] Flashing landscape firmware binary to ${BOOT_PORT}..."
cd "${REPO_ROOT}/firmware"

# Use --before=usb_reset for ESP32-S3 native USB-Serial/JTAG
if ! $ESPTOOL --chip esp32s3 -p "${BOOT_PORT}" -b 460800 --before=usb_reset --after=hard_reset write_flash \
    --flash_mode dio --flash_freq 80m --flash_size 16MB \
    0x0 "${BUILD_DIR}/bootloader/bootloader.bin" \
    0x10000 "${BUILD_DIR}/osupad-firmware.bin" \
    0x8000 "${BUILD_DIR}/partition_table/partition-table.bin"; then
    echo "Retrying with espflash..."
    espflash write-bin --chip esp32s3 -p "${BOOT_PORT}" --before usb-reset --non-interactive 0x10000 "${BUILD_DIR}/osupad-firmware.bin"
fi

echo ""
echo "=================================================="
echo "✓ Flashing successful! Landscape UI is now active."
echo "  The board is now running firmware with full auto-reset."
echo "  All future flashes will run hands-free automatically!"
echo "=================================================="
