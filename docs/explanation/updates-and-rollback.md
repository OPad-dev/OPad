---
title: Firmware Updates and Automatic Rollback
description: Dual OTA partition layout, bootloader trial boot verification, USB-driven automatic rollback, and software update channels.
category: explanation
source_files:
  - /firmware/main/app_main.c
  - /firmware/main/runtime/runtime.c
  - /firmware/main/protocol/protocol.c
  - /firmware/main/usb/usb_cdc.c
  - /desktop/crates/opad-device/src/flash.rs
  - /desktop/crates/opad-update/src/lib.rs
  - /desktop/daemon/src/firmware_update.rs
---

OPad guarantees that a firmware update can never strand or brick a keypad (Product Rule 5). Updating firmware is fully automated, requires no interaction with hardware BOOT or RESET buttons, and includes an automatic rollback mechanism: if a freshly flashed image crashes, hangs, or fails to enumerate on USB, the ESP32-S3 ROM and second-stage bootloaders automatically roll back to the previously functioning firmware slot. <!-- src: /firmware/main/app_main.c -->

## Dual OTA Partition Layout

The ESP32-S3 flash partition table defines two alternating application slots alongside boot selection metadata: <!-- src: /desktop/crates/opad-device/src/flash.rs -->

| Partition Name | Partition Type | Subtype | Flash Offset | Size | Purpose |
|---|---|---|---|---|---|
| **`nvs`** | Data | NVS | `0x9000` | 24 KB | Device settings, calibration, lifetime counters. |
| **`otadata`** | Data | OTA | `0xf000` | 8 KB (`0x2000`) | Two 4 KB sectors tracking active OTA slot and image state. <!-- src: /desktop/crates/opad-device/src/flash.rs#OTADATA_OFFSET --> |
| **`ota_0`** | App | OTA_0 | `0x20000` | 2 MB | Firmware application slot 0. |
| **`ota_1`** | App | OTA_1 | `0x220000` | 2 MB | Firmware application slot 1. <!-- src: /desktop/crates/opad-device/src/flash.rs#OTA_1_OFFSET --> |

When flashing via `opadctl flash` or the desktop updater: <!-- src: /desktop/crates/opad-device/src/flash.rs -->
1. The host queries `DeviceInfo.running_partition` over CDC serial (e.g., `ota_0`). <!-- src: /desktop/crates/opad-device/src/flash.rs -->
2. The flasher targets the **opposite slot** (`ota_1`), leaving the current working firmware unmodified in flash.
3. The flasher generates a fresh `otadata` image (`otadata_image`) containing two boot entries:
   - Sector 0: Retains the old slot marked as `OTA_IMG_VALID` (`0x2`). <!-- src: /desktop/crates/opad-device/src/flash.rs#OTA_IMG_VALID -->
   - Sector 1: Points to the new slot marked as `OTA_IMG_NEW` (`0x0`, trial boot). <!-- src: /desktop/crates/opad-device/src/flash.rs#OTA_IMG_NEW -->
4. The device is commanded to reboot into the new image.

## Trial Boot & USB-Driven Rollback

When the ESP32-S3 bootloader starts, it boots the newly flashed slot in state `ESP_OTA_IMG_PENDING_VERIFY`. The new firmware must explicitly validate itself or face rollback. <!-- src: /firmware/main/app_main.c#IMAGE_CONFIRM_TIMEOUT_US -->

```
[Bootloader Starts]
         |
         v
[Boot New Image (PENDING_VERIFY)]
         |
         +--------------------------------------------------------+
         |                                                        |
         v                                                        v
[USB Configured within 20s]                           [Crash Loop or USB Hangs > 20s]
         |                                                        |
         v                                                        v
esp_ota_mark_app_valid_cancel_rollback()                      esp_restart()
         |                                                        |
         v                                                        v
[Image Confirmed Permanently]                          [Bootloader Rolls Back to Previous Slot]
```

### Verification Criteria (`confirm_image_once_usb_works`)
The firmware's image confirmation logic runs at the end of `app_main.c`: <!-- src: /firmware/main/app_main.c -->
1. **Pending Check**: The image checks whether its running partition is in `ESP_OTA_IMG_PENDING_VERIFY`. If already validated, it proceeds immediately. <!-- src: /firmware/main/app_main.c -->
2. **USB Enumeration Requirement**: An image that boots and runs internal tasks but fails to enumerate as a USB HID keyboard is useless to the player. The pad waits for the USB host to mount the device (`tud_mounted()`). <!-- src: /firmware/main/app_main.c -->
3. **20-Second Timeout Window**: If a USB host connection is detected (`tud_connected()`) but the host fails to configure the TinyUSB HID stack within 20 seconds (`IMAGE_CONFIRM_TIMEOUT_US = 20,000,000`), the firmware logs an error and triggers `esp_restart()`. <!-- src: /firmware/main/app_main.c#IMAGE_CONFIRM_TIMEOUT_US -->
4. **Bootloader Rollback**: Upon restart, the ESP32 bootloader detects that the trial image failed verification, marks it invalid, and boots back into the previous valid partition.
5. **No Host Connected (Charger Exception)**: If the pad is powered from a wall charger rather than a computer USB host, `tud_connected()` remains false. The pad does not roll back; verification remains pending until the pad is next plugged into a computer host. <!-- src: /firmware/main/app_main.c -->
6. **Validation**: As soon as `tud_mounted()` succeeds, the pad calls `esp_ota_mark_app_valid_cancel_rollback()`, permanently confirming the slot. <!-- src: /firmware/main/app_main.c -->

## Serial Port Release & Download Mode Handshake

When flashing over USB serial CDC (`reboot_to_rom_download` in `firmware/main/usb/usb_cdc.c` and `desktop/crates/opad-device/src/flash.rs`): <!-- src: /firmware/main/usb/usb_cdc.c#reboot_to_rom_download -->
- `opadctl flash` sends `IpcRequest::PrepareFlash` to `opad-daemon`.
- The daemon closes its CDC handle, freeing `/dev/ttyACM0` or COM port access.
- The flasher instructs the firmware to enter ROM download mode. The firmware asserts USB D+ pull-up detachment (`SE0` line state for ~300 ms), restores `dp_pullup` to 1, calls `tud_disconnect()`, and triggers a hardware RTC watchdog reset. <!-- src: /firmware/main/usb/usb_cdc.c#reboot_to_rom_download -->
- Flashing proceeds via `espflash` directly into the inactive OTA partition.
- After flashing finishes, `opad-daemon` resumes polling (`IpcRequest::FinishFlash`), discovering the newly booted firmware.

## Desktop Software Update Channels

Desktop applications (`opad-daemon`, `opad-gui`, `opadctl`, and `tosu`) are managed by `opad-update`: <!-- src: /desktop/crates/opad-update/src/lib.rs -->
- **Release Channels**: Users can select between `stable` and `beta` channels in settings.
- **Manifest Signatures**: Update manifests are checked against cryptographic signatures. Binary archives are downloaded into temporary staging directories and unpacked atomically over the install prefix.
- **Independent Supervision**: The update engine replaces application binaries on disk while processes remain running; the GUI displays an unobtrusive "Restart to apply update" notification.
