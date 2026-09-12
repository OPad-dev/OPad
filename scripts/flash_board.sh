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

trigger_auto_reset() {
    local port="$1"
    python3 -c "
import serial, time
try:
    s = serial.Serial('$port', 1200, timeout=0.2, write_timeout=0.2)
    time.sleep(0.05)
    s.dtr = False
    s.rts = False
    s.close()
except Exception:
    pass
" 2>/dev/null || true
}

CURRENT_PORT=$(find_esp_port || echo "/dev/ttyACM0")
BOOT_PORT=$(get_bootloader_port || true)

if [ -z "$BOOT_PORT" ]; then
    echo "[1/3] Device running application on ${CURRENT_PORT} (303a:4001)."
    echo "      Sending USB auto-reset signal..."
    trigger_auto_reset "$CURRENT_PORT"

    for i in {1..20}; do
        sleep 0.1
        BOOT_PORT=$(get_bootloader_port || true)
        if [ -n "$BOOT_PORT" ]; then
            break
        fi
    done
fi

if [ -z "$BOOT_PORT" ]; then
    echo ""
    echo "[2/3] The board currently has the previous build without the software auto-reset hook."
    echo "      To install this firmware update (REQUIRED ONCE):"
    echo ""
    echo "      ============================================================"
    echo "      1. Press and hold down the BOOT button on the ESP32-S3."
    echo "      2. While STILL HOLDING the BOOT button, press & release RESET."
    echo "      3. KEEP HOLDING the BOOT button until flashing starts!"
    echo "      ============================================================"
    echo ""
    echo "      Once this update is flashed, ALL future flashes will run"
    echo "      100% automatically over USB without touching any buttons!"
    echo ""
    echo "Waiting for ESP32-S3 ROM Bootloader (303a:1001)..."

    while true; do
        BOOT_PORT=$(get_bootloader_port || true)
        if [ -n "$BOOT_PORT" ]; then
            break
        fi
        sleep 0.05
    done
fi

echo "[2/3] ✓ ESP32-S3 ROM Bootloader detected on ${BOOT_PORT}!"
echo "      Waiting for serial port to stabilize..."

# Wait up to 3 seconds for port to become writable
for i in {1..30}; do
    if [ -w "${BOOT_PORT}" ]; then
        break
    fi
    sleep 0.1
done
sleep 0.4

echo "[3/3] Bootloader ready! Flashing firmware to ${BOOT_PORT}..."
cd "${REPO_ROOT}/firmware"

FLASH_SUCCESS=false

# Try 1: esptool with no_reset (chip is already in download mode)
echo "Writing flash with esptool (no_reset)..."
if $ESPTOOL --chip esp32s3 -p "${BOOT_PORT}" -b 460800 --before=no_reset --after=hard_reset write_flash \
    --flash_mode dio --flash_freq 80m --flash_size 16MB \
    0x0 "${BUILD_DIR}/bootloader/bootloader.bin" \
    0x10000 "${BUILD_DIR}/osupad-firmware.bin" \
    0x8000 "${BUILD_DIR}/partition_table/partition-table.bin"; then
    FLASH_SUCCESS=true
fi

# Try 2: esptool with usb_reset
if [ "$FLASH_SUCCESS" = false ]; then
    echo "Retrying esptool with usb_reset..."
    sleep 0.5
    if $ESPTOOL --chip esp32s3 -p "${BOOT_PORT}" -b 460800 --before=usb_reset --after=hard_reset write_flash \
        --flash_mode dio --flash_freq 80m --flash_size 16MB \
        0x0 "${BUILD_DIR}/bootloader/bootloader.bin" \
        0x10000 "${BUILD_DIR}/osupad-firmware.bin" \
        0x8000 "${BUILD_DIR}/partition_table/partition-table.bin"; then
        FLASH_SUCCESS=true
    fi
fi

# Try 3: espflash write-bin fallback
if [ "$FLASH_SUCCESS" = false ]; then
    echo "Retrying with espflash..."
    sleep 0.5
    espflash write-bin --chip esp32s3 -p "${BOOT_PORT}" --non-interactive 0x10000 "${BUILD_DIR}/osupad-firmware.bin"
fi

echo ""
echo "=================================================="
echo "✓ Flashing successful! Firmware is active."
echo "  All future flashes will now run hands-free automatically!"
echo "=================================================="
