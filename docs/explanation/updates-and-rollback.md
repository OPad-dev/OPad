---
title: Firmware Updates and Automatic Rollback
description: Dual OTA partition layout, bootloader trial boot verification, USB-driven automatic rollback, and software update channels.
sidebar:
  order: 6
---

OPad updates firmware over USB without touching the hardware BOOT or RESET buttons (AGENTS.md §1.5). Two mechanisms prevent a bad update from stranding a pad: a dual-slot OTA partition table with USB-gated automatic rollback, and a mask-ROM recovery path (`opadctl recover`) when an image does not enumerate at all. <!-- src: /firmware/main/app_main.c#confirm_image_once_usb_works, /desktop/crates/opad-device/src/flash.rs#recover -->

## Dual OTA Partition Layout

The ESP32-S3 partition table in [`firmware/partitions.csv`](/firmware/partitions.csv) defines two 2 MB application slots alongside NVS and OTA selection metadata (`CONFIG_BOOTLOADER_APP_ROLLBACK_ENABLE=y` in [`firmware/sdkconfig.defaults`](/firmware/sdkconfig.defaults)):

| Partition Name | Partition Type | Subtype | Flash Offset | Size | Purpose |
|---|---|---|---|---|---|
| **`nvs`** | Data | NVS | `0x9000` | 24 KB (`0x6000`) | Device settings, ownership (`owner_id`), calibration, layouts, lifetime counters. |
| **`otadata`** | Data | OTA | `0xf000` | 8 KB (`0x2000`) | Two 4 KB sectors tracking active OTA slot and image state ([`OTADATA_OFFSET`](/desktop/crates/opad-device/src/flash.rs)). |
| **`phy_init`** | Data | PHY | `0x11000` | 4 KB (`0x1000`) | RF/PHY initialization data. |
| **`ota_0`** | App | OTA_0 | `0x20000` | 2 MB (`0x200000`) | Firmware application slot 0 ([`APP_PARTITION_OFFSET`](/desktop/crates/opad-device/src/flash.rs)). |
| **`ota_1`** | App | OTA_1 | `0x220000` | 2 MB (`0x200000`) | Firmware application slot 1 ([`OTA_1_OFFSET`](/desktop/crates/opad-device/src/flash.rs)). |

When flashing via `opadctl flash` (with the running slot known) or the daemon firmware updater ([`plan_app_update()`](/desktop/crates/opad-device/src/flash.rs)):

1. The pad reports its active partition in `HelloAck.running_partition` ([`protocol_send_hello_ack()`](/firmware/main/protocol/protocol.c)), which the daemon stores in `DeviceInfo.running_partition`.
2. [`plan_app_update()`](/desktop/crates/opad-device/src/flash.rs) targets the **opposite slot** (`ota_1` when running `ota_0`, or `ota_0` when running `ota_1`), leaving the working slot untouched. (Note: the daemon's host-driven updater currently checks [`is_ota_0()`](/desktop/crates/opad-update/src/firmware.rs) before starting, so it only offers updates when the pad is on `ota_0`.)
3. [`otadata_image()`](/desktop/crates/opad-device/src/flash.rs) builds an 8 KB `otadata` image containing two `esp_ota_select_entry_t` records with zlib CRC-32 checksums (`otadata_crc`):
   - Sector 0 keeps the previous slot marked `OTA_IMG_VALID` (`0x2`).
   - Sector 1 selects the newly written slot with a higher `ota_seq` marked `OTA_IMG_NEW` (`0x0`).
4. The app binary is written first and `otadata` second, so an interrupted write leaves the old slot selected. Finally, [`reset_to_app()`](/desktop/crates/opad-device/src/flash.rs) clears `RTC_CNTL_FORCE_DOWNLOAD_BOOT` and triggers an RTC watchdog reset into the new image.

## Trial Boot & USB-Driven Rollback

When the ESP32-S3 second-stage bootloader boots an `OTA_IMG_NEW` slot, it transitions that slot to `ESP_OTA_IMG_PENDING_VERIFY`. The running firmware must confirm itself or the bootloader will roll back on the next reset ([`confirm_image_once_usb_works()`](/firmware/main/app_main.c)).

```
[Bootloader Starts]
         |
         v
[Boot New Image (PENDING_VERIFY)]
         |
         +------------------------------------------------------------------+
         |                                                                  |
         v                                                                  v
[Host configures USB (tud_mounted())]             [Crash loop or host does not configure USB within 20 s]
         |                                                                  |
         v                                                                  v
esp_ota_mark_app_valid_cancel_rollback()                             esp_restart() / reset
         |                                                                  |
         v                                                                  v
[Image Confirmed Permanently]                     [Bootloader marks slot aborted and boots previous slot]
```

### Verification Criteria (`confirm_image_once_usb_works`)

[`confirm_image_once_usb_works()`](/firmware/main/app_main.c) runs at the end of `app_main`, after input, TinyUSB, CDC, counters, runtime, display, and touch have initialized:

1. **Pending Check**: Reads `esp_ota_get_state_partition()` for the running partition. If it is not `ESP_OTA_IMG_PENDING_VERIFY`, the function returns immediately.
2. **USB Configuration Requirement**: Loops on `!tud_mounted()`, polling every 100 ms, because firmware that boots internally without becoming a usable USB HID keyboard must not be confirmed.
3. **20-Second Timeout Window**: Once a USB host attaches (`tud_connected()`), a timer starts. If `tud_mounted()` does not become true within 20 seconds (`IMAGE_CONFIRM_TIMEOUT_US = 20 * 1000 * 1000`), the firmware logs an error and calls `esp_restart()`.
4. **Bootloader Rollback**: On restart (whether from `esp_restart()` or from a crash loop before confirmation), the ESP32-S3 bootloader sees the slot is still unconfirmed, marks it aborted, and boots the previous `OTA_IMG_VALID` slot.
5. **No Host Connected (Charger Exception)**: When powered from a wall charger or USB power bank, `tud_connected()` stays false and `host_since_us` resets to `0`. The image does not roll back; it stays pending until the pad is next connected to a computer USB host.
6. **Permanent Confirmation**: As soon as `tud_mounted()` returns true, the firmware calls `esp_ota_mark_app_valid_cancel_rollback()`.

## Serial Port Release & Download Mode Handshake

Flashing over USB coordinates the daemon, the running firmware, and `espflash` ([`reboot_to_rom_download()`](/firmware/main/usb/usb_cdc.c), [`enter_bootloader()`](/desktop/crates/opad-device/src/flash.rs)):

1. `opadctl flash` sends `IpcRequest::PrepareFlash` (or the daemon updater calls `device.pause_and_release()`), closing the CDC handle so the port is free (mandatory on Windows, where serial handles are exclusive).
2. [`enter_bootloader()`](/desktop/crates/opad-device/src/flash.rs) tries up to two triggers (`BOOT_TRIGGERS`): sending `BOOTLOADER\n` over CDC, then opening and closing the port at 1200 baud (dropping DTR).
3. Inside [`reboot_to_rom_download()`](/firmware/main/usb/usb_cdc.c), the firmware flushes CDC TX, sleeps 50 ms, calls `tud_disconnect()` (never `tinyusb_driver_uninstall()`), routes the internal USB PHY back to USB-Serial-JTAG, holds SE0 (both lines pulled down, no pull-up) for 300 ms so the host sees a real detach, restores `dp_pu = true` *before* disabling the pull override, waits up to 1.5 s for `USB_SERIAL_JTAG_INTR_BUS_RESET`, writes `RTC_CNTL_FORCE_DOWNLOAD_BOOT` to `RTC_CNTL_OPTION1_REG`, and calls `esp_restart()`.
4. [`write_images()`](/desktop/crates/opad-device/src/flash.rs) invokes `espflash write-bin --chip esp32s3 --before no-reset --after no-reset-no-stub --non-interactive` for each image.
5. [`reset_to_app()`](/desktop/crates/opad-device/src/flash.rs) sends SLIP `WRITE_REG` commands directly to the ROM bootloader stub to clear `RTC_CNTL_OPTION1_REG` and trigger an RTC watchdog reset into the application.
6. The daemon resumes polling (`IpcRequest::FinishFlash` or `device.resume()`) and waits for the pad to reconnect.

## Desktop Software Update Channels

Desktop components (the OPad app and bundled `tosu`) and firmware offers are coordinated by [`opad-update`](/desktop/crates/opad-update/src/lib.rs) and the daemon's update worker ([`desktop/daemon/src/updater.rs`](/desktop/daemon/src/updater.rs)):

- **Release Channels & Schedule**: Users select `stable` (default) or `beta` in **Settings** -> **🔄 Updates & Firmware** ([`pages.rs`](/desktop/gui/src/pages.rs)). The update worker wakes every 15 minutes (`TICK`), runs a daily ETag-cached check only while the daemon is `IDLE` (never during `PLAYING` or `COOLDOWN`), and applies gradual rollout bucketing via the install ID (`rollout::for_install`).
- **Verification**: Manifest bytes are verified with a detached **minisign** signature against `TRUSTED_KEYS` before parsing, and every downloaded artifact is verified against its **SHA-256** hash inside that signed manifest ([`verify.rs`](/desktop/crates/opad-update/src/verify.rs)).
- **Origin-Aware Application**: How an app update is applied depends on the `install-origin` marker ([`origin.rs`](/desktop/crates/opad-update/src/origin.rs)):
  - `windows`: runs the signed Inno Setup installer with `/SILENT /NORESTART /RELAUNCH=1` ([`apply_command()`](/desktop/crates/opad-update/src/app.rs)) (which stops the daemon; startup recovery [`resolve_interrupted()`](/desktop/daemon/src/updater.rs) records the result after restart).
  - `deb` / `rpm`: upgrades through `apt` / `dnf` (using `pkcon` when the OPad repo is configured, or `pkexec` on the downloaded package).
  - `appimage` / `user`: replaces the `$APPIMAGE` file atomically (`replace_file`) or extracts the archive over `install_prefix()` with `tar -xzf`.
  - `aur` / `source` / `unknown`: `NotifyOnly` (reports the available version without modifying files).
- **Autonomy & Restart**: Bundled `tosu` auto-updates when enabled on origins where OPad owns the binary (`windows`, `user`, `appimage`). The desktop app never auto-installs; it waits for the user to click **Install now**, then sets `restart_required` so the GUI shows `"OPad was updated. Restart the app and the daemon to finish — the pad keeps working as a keyboard meanwhile."` ([`main.rs`](/desktop/gui/src/main.rs)). Firmware updates are never automatic and always require explicit consent.
