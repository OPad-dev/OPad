---
title: Firmware USB Subsystem
description: Implementation reference for TinyUSB composite descriptors, split 1000 Hz HID endpoints, CDC-ACM framing, and ROM download reboot handling.
sidebar:
  order: 3
---

The USB subsystem implements a USB 2.1 Full-Speed composite device (`VID 0x303A`, `PID 0x4001`, `bcdDevice 0x0102`) using TinyUSB (`espressif/esp_tinyusb` 2.3.0 / `tinyusb` 0.21.0~2) pinned to Core 0 at priority `configMAX_PRIORITIES - 2` ([/firmware/main/app_main.c](/firmware/main/app_main.c), [/firmware/main/usb/usb_descriptors.c](/firmware/main/usb/usb_descriptors.c)).

## Composite Descriptors & Interface Topology

Declared in [/firmware/main/usb/usb_descriptors.c](/firmware/main/usb/usb_descriptors.c):

| Interface | Class / Protocol | Endpoints | Polling | Purpose |
|---|---|---|---|---|
| **Interface 0 (`ITF_NUM_HID`)** | HID Keyboard (`HID_ITF_PROTOCOL_KEYBOARD`) | `EP 0x81` (IN, 8 B) | `1 ms` (1000 Hz) | Key 1 and touchscreen Quick Retry (`LINK_KEY1_TOUCH`, report ID `0`) |
| **Interfaces 1 & 2 (`ITF_NUM_CDC`)** | CDC-ACM | `EP 0x82` (Notif IN, 8 B), `EP 0x03` (OUT, 64 B), `EP 0x83` (IN, 64 B) | Bulk / Interrupt | Framed nanopb serial protocol (`usb_cdc.c`) |
| **Interface 3 (`ITF_NUM_MEDIA`)** | HID Media & Key 2 (`HID_ITF_PROTOCOL_NONE`) | `EP 0x84` (IN, 16 B) | `1 ms` (1000 Hz) | Key 2 keyboard report (`MEDIA_REPORT_ID_KEYBOARD`), consumer media keys (`MEDIA_REPORT_ID_CONSUMER`), and mouse wheel (`MEDIA_REPORT_ID_MOUSE`) |

Also exposed:
- **BOS + MS OS 2.0 descriptors** (`opad_usb_bos_desc`, `opad_usb_ms_os_20_desc`): Assigns `FriendlyName = "OPad"` to Interface 1 so Windows Device Manager shows `OPad (COMx)` without a custom INF.
- **Build-time `_Static_assert` guards**: Fail the build if `CFG_TUD_HID < HID_INSTANCE_MEDIA + 1` or `CFG_TUD_CDC < 1` (catches stale `firmware/sdkconfig`).

## Split Endpoint HID Submission (`usb_hid.c`)

Implemented in [/firmware/main/usb/usb_hid.c](/firmware/main/usb/usb_hid.c):

- **Two independent keyboard links (`hid_link_t`)**: `LINK_KEY1_TOUCH` (`HID_INSTANCE_KEYBOARD`, `EP 0x81`) and `LINK_KEY2` (`HID_INSTANCE_MEDIA`, `EP 0x84`). Because ESP32-S3 has at most 5 IN endpoints including `EP0`, Key 2 shares Interface 3's endpoint rather than adding a third HID interface.
- **Dynamic Key 3 routing (`usb_hid_set_key3`)**: Routes Key 3 onto whichever link is not carrying the other active game key so two active game keys never contend for the same 1 ms USB poll.
- **Lockless sequence tracking**: Writers bump `change_seq` before `try_submit`; successful `tud_hid_n_keyboard_report` advances `sent_seq`. If an endpoint is busy, `pending_edge_us` stores the oldest undelivered edge timestamp and `tud_hid_report_complete_cb` immediately resends and records deferred latency.

## CDC-ACM Framing & ROM Download Reboot (`usb_cdc.c`)

Implemented in [/firmware/main/usb/usb_cdc.c](/firmware/main/usb/usb_cdc.c):

- **Receive task (`cdc_proto`)**: Runs on Core 1 (`tskIDLE_PRIORITY + 3`, 6144 B stack) feeding received bytes into `protocol_feed_cdc_bytes` and handling `tud_cdc_line_state_cb` DTR drops (which reset framing and cancel app-initiated HE calibration).
- **1200-baud touch fallback**: Setting line coding to 1200 baud with DTR low arms a fallback reboot into ROM download mode.
- **Hands-free ROM download reboot (`reboot_to_rom_download`)**:
  1. Calls `tud_disconnect()` first (never `tinyusb_driver_uninstall()`, which frees mutexes other tasks use).
  2. Forces USB `D+` pull-up low (`SE0`) for ~300 ms so the host registers a real physical disconnect (avoiding Linux `error -71`).
  3. Restores `dp_pullup = 1` *before* clearing `pad_pull_override` so the chip does not vanish from the bus.
  4. Sets `RTC_CNTL_FORCE_DOWNLOAD_BOOT` in `RTC_CNTL_OPTION1_REG` and calls `esp_restart()` to enter the ESP32-S3 ROM bootloader (`303a:1001`). (Exiting ROM download mode afterward is performed by the host flasher via an RTC watchdog `WRITE_REG`.)
