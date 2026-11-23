---
title: Updating Firmware
description: Step-by-step instructions for flashing and updating OPad firmware via opadctl or the desktop GUI with dual OTA slot protection and rollback.
sidebar:
  order: 1
---

OPad updates firmware over USB without ever requiring the hardware BOOT or RESET buttons. Updates write to the inactive OTA partition when the running slot is known, sync lifetime counters to the host before flashing (via the daemon updater), and roll back automatically if the new image crashes or fails USB configuration. <!-- src: /desktop/crates/opad-device/src/flash.rs#plan_app_update, /firmware/main/app_main.c#confirm_image_once_usb_works -->

## What you need

- Your OPad connected directly to a PC USB port (avoid unreliable hubs during flashing). <!-- src: /desktop/crates/opad-update/src/firmware.rs#consent_text -->
- `opad-daemon` running (for GUI updates, `opadctl firmware-update`, or automatic OTA slot detection in `opadctl flash`). `opadctl flash` also works without the daemon running, though without automatic slot detection. <!-- src: /desktop/cli/src/main.rs#Commands -->
- `espflash` available (bundled with OPad packages at `<prefix>/lib/opad/bin/espflash` on Linux or beside `opadctl.exe` on Windows, or installed in `PATH` / `%USERPROFILE%\.cargo\bin\espflash.exe`). <!-- src: /desktop/crates/opad-device/src/flash.rs#resolve_espflash -->

## Safety Sequence (Daemon-Managed Updates)

Both the GUI updater and `opadctl firmware-update` run the same daemon pipeline (`firmware_update::install`). **Update now** asks every time. With **Install firmware updates automatically** on (the default), the daemon also starts the same update by itself once the pad has gone 30 minutes unused: no key press and, unless the screen never sleeps, the screen asleep. Never during a map, and a version that failed is not tried again for a day: <!-- src: /desktop/daemon/src/firmware_update.rs#AUTO_IDLE, /desktop/daemon/src/firmware_update.rs#auto_wait, /desktop/daemon/src/main.rs, /desktop/crates/opad-update/src/firmware.rs -->

1. **Precondition Checks**: The daemon checks that the pad is connected, running from an OTA slot (`ota_0` or `ota_1`: an update goes to the other one), not guarded by an unresolved ownership takeover or replacement prompt, that the daemon is in `IDLE` mode (never `PLAYING`, `COOLDOWN`, or `SYNC`), that SQLite storage is available, that the user consented (Update now) or automatic updates are on, and that the desktop app satisfies any `min_app` version required by the manifest. <!-- src: /desktop/daemon/src/firmware_update.rs#install, /desktop/crates/opad-update/src/firmware.rs#blockers, /desktop/crates/opad-update/src/firmware.rs#is_ota_slot, /desktop/crates/opad-device/src/flash.rs#plan_app_update -->
2. **Pre-Flash Counter Sync**: Before downloading or writing anything, the daemon syncs the pad's lifetime counters to the local SQLite database (`perform_sync`). <!-- src: /desktop/daemon/src/firmware_update.rs#install -->
3. **Signature, Hash & Header Verification**: The release manifest is verified with a minisign signature against the feed key (`trusted_keys`; the public test key only where `OPAD_TRUST_TEST_KEY` allows it), the downloaded binary is checked against its SHA-256 hash in the manifest (`stage_bytes`), and the image header is checked for ESP magic byte `0xE9` and ESP32-S3 chip ID `0x0009` (`check_esp32s3_image`). <!-- src: /desktop/crates/opad-update/src/verify.rs#trusted_keys, /desktop/daemon/src/firmware_update.rs#install, /desktop/crates/opad-device/src/flash.rs#check_esp32s3_image -->
4. **Last-Moment Idle Re-check**: After the download finishes, `still_safe_to_flash` re-checks that the daemon is still `IDLE`, the pad is still connected, and no map started during the download. <!-- src: /desktop/daemon/src/firmware_update.rs#still_safe_to_flash -->
5. **Opposite-Slot Flash & Verification**: The daemon releases the serial port, flashes the app image into the inactive OTA slot (`plan_app_update`), writes the trial `otadata` record, reboots the pad (`reset_to_app`), resumes device discovery, and waits up to 45 seconds (`RECONNECT_TIMEOUT`) for the pad to reconnect reporting the target slot and new version. <!-- src: /desktop/daemon/src/firmware_update.rs#flash_and_verify, /desktop/crates/opad-device/src/flash.rs#plan_app_update -->

## Updating via Desktop GUI

When a newer firmware release is published on your selected update channel (`stable` or `beta`): <!-- src: /desktop/gui/src/pages.rs#updates, /desktop/gui/src/pages.rs#firmware_updates -->

1. Open `opad-gui`, select **Settings** in the sidebar, and click the **🔄 Updates & Firmware** tab. (You can also click **Update firmware** on the **Device** page to jump to **Settings**.) <!-- src: /desktop/gui/src/pages.rs#settings, /desktop/gui/src/pages.rs#device -->
2. In the **FIRMWARE UPDATE** card, review the installed version, `Running slot`, available version, release notes, and any `⚠` blockers. <!-- src: /desktop/gui/src/pages.rs#firmware_updates -->
3. Click **Update firmware** (enabled only when an update is available and there are no blockers). <!-- src: /desktop/gui/src/pages.rs#firmware_updates -->
4. Read the **Confirm Firmware Update** dialog, which notes that the pad is unusable as a keyboard for about 30 seconds (`TYPICAL_OUTAGE_SECONDS = 30`) and warns not to unplug it. Click **Confirm & Flash Pad**. <!-- src: /desktop/gui/src/main.rs#App, /desktop/crates/opad-update/src/firmware.rs#TYPICAL_OUTAGE_SECONDS -->
5. Wait through the **Flashing Firmware in Progress** modal until **✓ Firmware Update Complete** appears, then click **Close**. <!-- src: /desktop/gui/src/main.rs#App -->

## Updating via `opadctl` (CLI)

### Official Release Update (`opadctl firmware-update`)

To install the firmware version advertised by the daemon's verified release manifest using the same safety sequence as the GUI: <!-- src: /desktop/cli/src/main.rs#Commands -->

```sh
opadctl firmware-update
```

Review the version, notes, and consent text, then answer `y` at `Flash the pad now? [y/N]:` (or pass `--yes` in a non-interactive script). <!-- src: /desktop/cli/src/main.rs#Commands -->

### Local Binary Flash (`opadctl flash`)

To install a locally built or downloaded `.bin` file: <!-- src: /desktop/cli/src/main.rs#Commands -->

```sh
opadctl flash firmware/build/opad-firmware.bin
```

Pass `--port <port>` to override auto-detection, or `--full <dir>` with an ESP-IDF build directory or unpacked release directory to write the bootloader (`0x0`), partition table (`0x8000`), `ota_data_initial.bin` (`0xf000`), and app (`0x20000`). <!-- src: /desktop/cli/src/main.rs#Commands, /desktop/cli/src/main.rs#resolve_flash_set -->

What `opadctl flash` executes for an app update:

1. Validates the image header (`0xE9`, ESP32-S3 chip ID `0x0009`). <!-- src: /desktop/cli/src/main.rs#Commands, /desktop/crates/opad-device/src/flash.rs#check_esp32s3_image_file -->
2. Queries `opad-daemon` for the running slot (`ota_0` or `ota_1`) and plans an opposite-slot write (`OTA_0` at `0x20000` or `OTA_1` at `0x220000`) plus trial `otadata` (`0xf000`). If the running slot is unknown (no daemon or legacy firmware), it falls back to writing `ota_0` in place with no fallback. <!-- src: /desktop/cli/src/main.rs#Commands, /desktop/crates/opad-device/src/flash.rs#plan_app_update -->
3. Sends `IpcRequest::PrepareFlash` so `opad-daemon` releases the serial CDC port. <!-- src: /desktop/cli/src/main.rs#prepare_flash -->
4. Triggers ROM download mode (`enter_bootloader` via `BOOTLOADER\n` or 1200-baud touch; firmware `reboot_to_rom_download`). <!-- src: /desktop/crates/opad-device/src/flash.rs#enter_bootloader, /firmware/main/usb/usb_cdc.c#reboot_to_rom_download -->
5. Writes the images with `espflash write-bin --chip esp32s3 --before no-reset --after no-reset-no-stub --non-interactive` and reboots the pad via an RTC watchdog reset (`reset_to_app`). <!-- src: /desktop/crates/opad-device/src/flash.rs#write_images, /desktop/crates/opad-device/src/flash.rs#reset_to_app -->
6. Sends `IpcRequest::FinishFlash` so `opad-daemon` reconnects, and waits up to 45 seconds (`ROLLBACK_WAIT`) to confirm the target slot or detect rollback. <!-- src: /desktop/cli/src/main.rs#finish_flash, /desktop/cli/src/main.rs#report_slot_after_flash -->

## Rollback Behavior

A newly flashed OTA image boots once in `ESP_OTA_IMG_PENDING_VERIFY` state. At the end of `app_main`, `confirm_image_once_usb_works` waits for the host to configure USB (`tud_mounted()`): <!-- src: /firmware/main/app_main.c#confirm_image_once_usb_works -->

- **Confirmed**: As soon as the host configures USB (`tud_mounted()`), the firmware calls `esp_ota_mark_app_valid_cancel_rollback()` to confirm the new slot permanently. <!-- src: /firmware/main/app_main.c#confirm_image_once_usb_works -->
- **USB configuration timeout**: If a USB host connects (`tud_connected()`) and does not configure USB within 20 seconds (`IMAGE_CONFIRM_TIMEOUT_US`), the firmware calls `esp_restart()`. The bootloader marks the trial slot aborted and boots the previous valid slot. <!-- src: /firmware/main/app_main.c#confirm_image_once_usb_works, /firmware/main/app_main.c#IMAGE_CONFIRM_TIMEOUT_US -->
- **Crash loop**: If the new image crashes and resets before reaching confirmation, the bootloader rolls back to the previous valid slot on the next boot. <!-- src: /firmware/main/app_main.c#confirm_image_once_usb_works, /firmware/sdkconfig.defaults -->
- **No host connected**: When powered from a charger without a USB host (`!tud_connected()`), the image does not roll back; it stays pending until connected to a computer USB port. <!-- src: /firmware/main/app_main.c#confirm_image_once_usb_works -->
- Settings, Hall Effect calibration, ownership, and lifetime counters in NVS are untouched by OTA updates and rollbacks. <!-- src: /firmware/partitions.csv -->

## How to check it worked

Run `opadctl status` and check: <!-- src: /desktop/cli/src/main.rs#Commands -->

- `ESP32 Device: Connected`
- `Firmware Version:` shows the version you flashed
- `Running Slot:` shows `ota_0` or `ota_1`

In the GUI, open **Device** and check **Firmware** and **Running slot**. <!-- src: /desktop/gui/src/pages.rs#device -->

## What to do if it did not work

- If the pad rolled back to the previous slot, it keeps working as a keyboard on its earlier firmware; see [Recovery §7.6](../recovery.md#76-when-flashing-is-interrupted).
- If flashing failed or the pad no longer enumerates as OPad (`303a:4001`), run `opadctl recover` as described in [Recovery §7.7](../recovery.md#77-a-pad-that-does-not-show-up-as-opad-opadctl-recover).
- If the serial port is busy or permissions fail, see [Troubleshooting](../troubleshooting.md).
