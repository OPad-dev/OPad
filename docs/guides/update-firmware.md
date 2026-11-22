---
title: Updating Firmware
description: Step-by-step instructions for flashing and updating OPad firmware via opadctl or the desktop GUI with dual OTA slot protection and rollback.
category: how-to
source_files:
  - /desktop/cli/src/main.rs
  - /desktop/crates/opad-device/src/flash.rs
  - /desktop/daemon/src/firmware_update.rs
  - /desktop/daemon/src/updater.rs
  - /desktop/crates/opad-update/src/firmware.rs
  - /firmware/main/usb/usb_cdc.c
  - /firmware/main/app_main.c
  - /firmware/main/protocol/protocol.c
  - /firmware/main/runtime/runtime.c
---

Updating OPad firmware is completely safe and software-driven: it writes to the inactive OTA partition, syncs lifetime counters to the host before flashing, and automatically rolls back if the newly flashed image fails to enumerate on USB. Updating never requires pressing BOOT or RESET buttons (AGENTS.md §1.5). <!-- src: /desktop/crates/opad-update/src/firmware.rs -->

## Safety Sequence

Every firmware update follows an uncompromising four-step safety pipeline: <!-- src: /desktop/daemon/src/firmware_update.rs -->
1. **Consent & Idle Verification**: Updates are only initiated when the player explicitly confirms ("yes") and the pad is in `IDLE` mode (never during a map or cooldown).
2. **Pre-Flash Counter Sync**: The daemon immediately synchronizes pad lifetime counters to the host's local database. If anything goes wrong, the host can restore the exact counter total. <!-- src: /desktop/crates/opad-update/src/firmware.rs -->
3. **Dual Cryptographic & Header Check**: The binary is verified against the SHA-256 signature in the signed release manifest, and the ESP32-S3 chip magic ID is validated in the image header. <!-- src: /desktop/daemon/src/firmware_update.rs -->
4. **Opposite Slot Flashing**: The flasher queries the currently active partition (`ota_0` or `ota_1`) and flashes the new image into the *other* partition, updating `otadata` for a single trial boot. <!-- src: /desktop/crates/opad-device/src/flash.rs -->

## Flashing via opadctl (CLI)

The command-line tool `opadctl flash` is the primary mechanism for installing locally built or downloaded firmware binaries: <!-- src: /desktop/cli/src/main.rs -->

### Basic Update
```bash
opadctl flash firmware/build/opad-firmware.bin
```

### What opadctl flash Executes:
1. Contacts `opad-daemon` over IPC (`IpcRequest::PrepareFlash`). The daemon closes its serial CDC port and pauses background polling. <!-- src: /desktop/cli/src/main.rs -->
2. Sends the USB CDC command to reboot the ESP32-S3 into ROM download mode with a clean USB detachment sequence. <!-- src: /firmware/main/usb/usb_cdc.c#reboot_to_rom_download -->
3. Writes the binary into the inactive OTA partition (`OTA_0_OFFSET = 0x20000` or `OTA_1_OFFSET = 0x220000`) and commits the trial `otadata` record. <!-- src: /desktop/crates/opad-device/src/flash.rs#OTA_1_OFFSET -->
4. Reboots the pad via an RTC watchdog command.
5. Informs `opad-daemon` that flashing is finished (`IpcRequest::FinishFlash`), which reconnects to the newly booted firmware.

## Updating via Desktop GUI

When a new official firmware release is published on the active update channel (`stable` or `beta`): <!-- src: /desktop/daemon/src/firmware_update.rs -->
1. Open `opad-gui` and navigate to **Settings > Updates**.
2. If a firmware update is available, the interface displays the current version and new target version.
3. Click **Update Firmware**. A confirmation dialog details the release notes.
4. Click **Confirm**: the desktop downloads the image, verifies SHA-256, syncs counters, flashes the pad, and re-establishes the connection within 10–15 seconds.

## Rollback Behavior

If a newly flashed firmware image crashes during initialization, hangs in a task loop, or fails to enumerate on USB within 20 seconds, the trial boot fails: <!-- src: /firmware/main/app_main.c#confirm_image_once_usb_works -->
- The pad triggers an automatic hardware reset.
- The ESP32-S3 bootloader marks the trial slot invalid and boots back into the previously functioning slot.
- Your settings, calibration, and keypress counters in NVS remain intact.
