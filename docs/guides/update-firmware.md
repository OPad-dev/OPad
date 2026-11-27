---
title: Updating the Firmware
description: Update OPad's firmware over USB from the app or opadctl. The new firmware goes to the other slot and rolls back by itself if it does not work.
sidebar:
  order: 1
---

OPad's firmware updates over the same USB cable you play on. You never press the board's BOOT or RESET buttons, and a failed update cannot strand the pad: the new firmware goes into the other of the pad's two firmware slots, and if it does not come up as a working USB device the pad goes back to the old one by itself. <!-- src: /desktop/crates/opad-device/src/flash.rs#plan_app_update --> <!-- src: /firmware/main/app_main.c#confirm_image_once_usb_works -->

The app and the pad update independently. You can update the firmware without updating the app, and the other way round. <!-- src: /AGENTS.md#1. Product rules -->

## Before you start

- Plug the pad straight into the PC rather than through a hub you might knock. <!-- src: /desktop/crates/opad-update/src/firmware.rs#consent_text -->
- Have the desktop app installed and running. `opadctl flash` also works without the daemon, but then it cannot keep the old firmware as a fallback (see [below](#flash-a-firmware-file-opadctl-flash)). <!-- src: /desktop/cli/src/main.rs#Commands::Flash -->
- The pad stops working as a keyboard for about 30 seconds while it updates, so do it outside a map. OPad refuses to start an update during a map anyway. <!-- src: /desktop/crates/opad-update/src/firmware.rs#TYPICAL_OUTAGE_SECONDS --> <!-- src: /desktop/crates/opad-update/src/firmware.rs#blockers -->

Your lifetime counters, settings, layouts, Hall Effect calibration and owner record are kept: an update writes only the firmware slot. <!-- src: /desktop/daemon/src/firmware_update.rs --> <!-- src: /firmware/partitions.csv -->

## Update from the app

1. Open **Settings** → **🔄 Updates & Firmware**. (The **Update firmware** button on the **Device** page takes you to **Settings**.) <!-- src: /desktop/gui/src/pages.rs#settings --> <!-- src: /desktop/gui/src/pages.rs#device -->
2. The **FIRMWARE UPDATE** card shows the installed version, the slot it runs from, and the newer version if there is one, with its release notes. Anything that stops an update right now (no pad, a map running, an open ownership question) is listed with a `⚠`. <!-- src: /desktop/gui/src/pages.rs#firmware_updates -->
3. Click **Update now**. It is only enabled when an update is available and nothing is in the way. <!-- src: /desktop/gui/src/pages.rs#firmware_updates -->
4. Read the **Confirm Firmware Update** dialog and click **Confirm & Flash Pad**. <!-- src: /desktop/gui/src/main.rs#App -->
5. Leave the pad plugged in until **✓ Firmware Update Complete** appears, then click **Close**. <!-- src: /desktop/gui/src/main.rs#App -->

### Automatic updates

**Install firmware updates automatically** is on by default. With it on, the daemon installs a new firmware version by itself once the pad has gone 30 minutes unused: no key press for 30 minutes and, unless you set the screen never to sleep, the screen asleep. It never updates during a map. If an automatic update of a version fails, it does not try that version again for a day. Untick the box to only update when you click **Update now**. <!-- src: /desktop/daemon/src/firmware_update.rs#AUTO_IDLE --> <!-- src: /desktop/daemon/src/firmware_update.rs#auto_wait --> <!-- src: /desktop/daemon/src/main.rs --> <!-- src: /desktop/daemon/src/updater.rs#read_flag -->

## Update from a terminal (`opadctl firmware-update`)

This installs the same release as the app, the same way:

```sh
opadctl firmware-update
```

It shows the installed and available versions, the release notes and what the update will do, then asks `Flash the pad now? [y/N]:`. Answer `y`. In a script, pass `--yes` to give that answer up front. <!-- src: /desktop/cli/src/main.rs#Commands::FirmwareUpdate -->

## Flash a firmware file (`opadctl flash`)

To install a firmware file you built or downloaded yourself:

```sh
opadctl flash firmware/build/opad-firmware.bin
```

`opadctl flash`: <!-- src: /desktop/cli/src/main.rs#Commands::Flash -->

1. checks that the file is an ESP32-S3 firmware image; <!-- src: /desktop/crates/opad-device/src/flash.rs#check_esp32s3_image_file -->
2. asks the daemon which slot the pad runs from and picks the other one (`ota_0` at `0x20000` or `ota_1` at `0x220000`); <!-- src: /desktop/crates/opad-device/src/flash.rs#plan_app_update -->
3. asks the daemon to release the pad's serial port and reboots the pad into download mode over USB; <!-- src: /desktop/cli/src/main.rs#prepare_flash --> <!-- src: /desktop/crates/opad-device/src/flash.rs#enter_bootloader -->
4. writes the image with the bundled `espflash`, then restarts the pad into it; <!-- src: /desktop/crates/opad-device/src/flash.rs#write_images --> <!-- src: /desktop/crates/opad-device/src/flash.rs#reset_to_app -->
5. waits up to 45 seconds for the pad to come back, and tells you whether it runs the new slot or rolled back. <!-- src: /desktop/cli/src/main.rs#report_slot_after_flash -->

> [!WARNING]
> Without the daemon (or with firmware too old to report its slot), `opadctl flash` cannot tell which slot is running. It then writes `ota_0`, blanks the boot selection so `ota_0` is what boots, and prints `Running slot unknown: writing ota_0 and making it the slot that boots, with no fallback`. If that flash is interrupted, recover the pad with `opadctl recover` ([Recovery §7.7](../recovery.md#77-a-pad-that-does-not-show-up-as-opad-opadctl-recover)). <!-- src: /desktop/cli/src/main.rs#Commands::Flash -->

Options: <!-- src: /desktop/cli/src/main.rs#Commands::Flash -->

- `--port <port>`: the pad's port, when you do not want auto-detection (or several pads are connected).
- `--full`: a recovery flash. Give it a folder (an ESP-IDF build folder, or a firmware release's `.bin` files) and it also writes the bootloader, partition table and OTA data. It writes `ota_0` in place, so use it only for recovery; see [Recovery §7.4](../recovery.md#74-erase-then-flash).

The packages bundle `espflash`; `opadctl` looks for it next to itself, then in `<prefix>/lib/opad/bin/`, then on your `PATH`. <!-- src: /desktop/crates/opad-device/src/flash.rs#resolve_espflash -->

## What an app update checks for you

Before the daemon writes anything, it checks that the pad is connected and idle, runs from one of its two slots, has no open ownership or replacement question, and that this app version is new enough for the firmware. It then syncs your counters to the PC, downloads the firmware, verifies the release's signature and the file's SHA-256, checks that the image is for the ESP32-S3, and checks once more that no map started during the download. After flashing, it waits up to 45 seconds for the pad to come back on the new slot with the new version. <!-- src: /desktop/daemon/src/firmware_update.rs#install --> <!-- src: /desktop/crates/opad-update/src/firmware.rs#blockers --> <!-- src: /desktop/crates/opad-update/src/verify.rs#trusted_keys --> <!-- src: /desktop/daemon/src/firmware_update.rs#still_safe_to_flash --> <!-- src: /desktop/daemon/src/firmware_update.rs#RECONNECT_TIMEOUT -->

## Automatic rollback

New firmware starts on trial. It confirms itself only once your PC has set up its USB connection, which proves the pad works as a keyboard again. <!-- src: /firmware/main/app_main.c#confirm_image_once_usb_works -->

- **It works:** the new slot is kept.
- **The PC never sets it up:** after 20 seconds connected to a PC without that, the pad restarts and goes back to the previous firmware. <!-- src: /firmware/main/app_main.c#IMAGE_CONFIRM_TIMEOUT_US -->
- **It crashes before confirming:** the pad goes back to the previous firmware the same way.
- **On a charger, with no PC:** nothing is decided; the trial waits until you plug the pad into a computer.

## Check that it worked

```sh
opadctl status
```

Look for: <!-- src: /desktop/cli/src/main.rs#Commands::Status -->

- `ESP32 Device:     Connected`
- `Firmware Version:` the version you installed
- `Running Slot:` `ota_0` or `ota_1`

In the app, the **Device** page shows **Firmware** and **Running slot**. <!-- src: /desktop/gui/src/pages.rs#device -->

## If it did not work

- **The pad went back to the old firmware:** it keeps working as before. See [Recovery §7.6](../recovery.md#76-when-flashing-is-interrupted).
- **The pad does not show up as OPad (`303a:8415`) any more:** run `opadctl recover` and replug the pad, as in [Recovery §7.7](../recovery.md#77-a-pad-that-does-not-show-up-as-opad-opadctl-recover).
- **The serial port is busy, or permission is denied:** see [Troubleshooting §3](../troubleshooting.md#3-linux-usb-permissions-and-the-serial-port).
