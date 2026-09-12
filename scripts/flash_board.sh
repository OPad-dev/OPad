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
    echo "Building firmware now..."
    source "${HOME}/Documents/projects/esp32/v5.5.2/esp-idf/export.sh" >/dev/null 2>&1
    idf.py -C "${REPO_ROOT}/firmware" build
fi

DAEMON_WAS_ACTIVE=false
if systemctl --user is-active --quiet osupad-daemon.service 2>/dev/null; then
    echo "Temporarily pausing osupad-daemon service for exclusive flash access..."
    systemctl --user stop osupad-daemon.service
    DAEMON_WAS_ACTIVE=true
fi

cleanup() {
    if [ "$DAEMON_WAS_ACTIVE" = true ]; then
        echo "Restarting osupad-daemon service..."
        systemctl --user start osupad-daemon.service
    fi
}
trap cleanup EXIT

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

# Check if device is in ROM bootloader (303a:1001)
get_bootloader_port() {
    for p in /dev/serial/by-id/*Espressif* /dev/ttyACM0 /dev/ttyACM1 /dev/ttyACM2 /dev/ttyUSB0; do
        if [ -e "$p" ]; then
            local real_p
            real_p=$(realpath "$p" 2>/dev/null || echo "$p")
            local model_id vid
            vid=$(udevadm info -q property -n "$real_p" 2>/dev/null | grep "^ID_VENDOR_ID=" | cut -d= -f2 || true)
            model_id=$(udevadm info -q property -n "$real_p" 2>/dev/null | grep "^ID_MODEL_ID=" | cut -d= -f2 || true)
            if [ "$vid" = "303a" ] && [ "$model_id" = "1001" ]; then
                echo "$real_p"
                return 0
            fi
        fi
    done
    return 1
}

# Check if device is running osu!pad application (303a:4001)
get_app_port() {
    for p in /dev/serial/by-id/*osu_pad* /dev/ttyACM0 /dev/ttyACM1 /dev/ttyACM2 /dev/ttyUSB0; do
        if [ -e "$p" ]; then
            local real_p
            real_p=$(realpath "$p" 2>/dev/null || echo "$p")
            local model_id vid
            vid=$(udevadm info -q property -n "$real_p" 2>/dev/null | grep "^ID_VENDOR_ID=" | cut -d= -f2 || true)
            model_id=$(udevadm info -q property -n "$real_p" 2>/dev/null | grep "^ID_MODEL_ID=" | cut -d= -f2 || true)
            if [ "$vid" = "303a" ] && [ "$model_id" = "4001" ]; then
                echo "$real_p"
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
    s = serial.Serial('$port', 1200, timeout=0.2)
    s.write(b'BOOTLOADER\n')
    time.sleep(0.05)
    s.close()
except Exception:
    pass
" 2>/dev/null || true
}

BOOT_PORT=$(get_bootloader_port || true)

if [ -z "$BOOT_PORT" ]; then
    CURRENT_PORT=$(find_esp_port || echo "/dev/ttyACM0")
    echo "[1/3] Device running application on ${CURRENT_PORT} (303a:4001)."
    echo "      Sending hands-free USB bootloader trigger..."
    trigger_auto_reset "$CURRENT_PORT"

    for i in {1..30}; do
        sleep 0.1
        BOOT_PORT=$(get_bootloader_port || true)
        if [ -n "$BOOT_PORT" ]; then
            break
        fi
    done
fi

if [ -z "$BOOT_PORT" ]; then
    echo "[2/3] Waiting for ESP32-S3 ROM Bootloader (303a:1001)..."
    for i in {1..40}; do
        BOOT_PORT=$(get_bootloader_port || true)
        if [ -n "$BOOT_PORT" ]; then
            break
        fi
        sleep 0.1
    done
fi

if [ -z "$BOOT_PORT" ]; then
    echo "Error: ESP32-S3 ROM Bootloader (303a:1001) not detected." >&2
    exit 1
fi

echo "[2/3] ✓ ESP32-S3 ROM Bootloader detected on ${BOOT_PORT}!"

# Wait up to 2 seconds for port to become writable
for i in {1..20}; do
    if [ -w "${BOOT_PORT}" ]; then
        break
    fi
    sleep 0.05
done

echo "[3/3] Flashing firmware to ${BOOT_PORT}..."
cd "${REPO_ROOT}/firmware"

FLASH_SUCCESS=false

# Try 1: esptool with watchdog_reset (reboots cleanly back into application)
echo "Writing flash with esptool (watchdog_reset)..."
if $ESPTOOL --chip esp32s3 -p "${BOOT_PORT}" -b 460800 --before=no_reset --after=watchdog_reset write_flash \
    --flash_mode dio --flash_freq 80m --flash_size 16MB \
    0x0 "${BUILD_DIR}/bootloader/bootloader.bin" \
    0x10000 "${BUILD_DIR}/osupad-firmware.bin" \
    0x8000 "${BUILD_DIR}/partition_table/partition-table.bin"; then
    FLASH_SUCCESS=true
fi

# Try 2: esptool with usb_reset + watchdog_reset
if [ "$FLASH_SUCCESS" = false ]; then
    echo "Retrying esptool with usb_reset..."
    sleep 0.5
    if $ESPTOOL --chip esp32s3 -p "${BOOT_PORT}" -b 460800 --before=usb_reset --after=watchdog_reset write_flash \
        --flash_mode dio --flash_freq 80m --flash_size 16MB \
        0x0 "${BUILD_DIR}/bootloader/bootloader.bin" \
        0x10000 "${BUILD_DIR}/osupad-firmware.bin" \
        0x8000 "${BUILD_DIR}/partition_table/partition-table.bin"; then
        FLASH_SUCCESS=true
    fi
fi

# Try 3: espflash write-bin fallback. espflash's watchdog-reset is a no-op on the
# ESP32-S3 while FORCE_DOWNLOAD_BOOT is set, so stay in the stub and let esptool
# (which clears that bit via an RTC reset) reboot into the app.
if [ "$FLASH_SUCCESS" = false ]; then
    echo "Retrying with espflash..."
    sleep 0.5
    espflash write-bin --chip esp32s3 -p "${BOOT_PORT}" --before no-reset --after no-reset-no-stub --non-interactive 0x10000 "${BUILD_DIR}/osupad-firmware.bin"
    $ESPTOOL --chip esp32s3 -p "${BOOT_PORT}" --before=no_reset --after=watchdog_reset chip_id >/dev/null
fi

echo ""
echo "Waiting for osu!pad to reboot into application mode (303a:4001)..."
APP_PORT=""
for i in {1..50}; do
    APP_PORT=$(get_app_port || true)
    if [ -n "$APP_PORT" ]; then
        break
    fi
    sleep 0.1
done

if [ -z "$APP_PORT" ]; then
    echo "✗ Firmware was written, but the osu!pad app (303a:4001) did not come back within 5s."
    lsusb | grep -i 303a || true
    exit 1
fi

echo "=================================================="
echo "✓ Flashing successful! Firmware is active."
echo "  Device rebooted into application mode hands-free!"
echo "=================================================="
