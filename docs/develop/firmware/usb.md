---
title: Firmware USB Subsystem
description: Implementation reference for TinyUSB composite descriptors, split 1000 Hz HID endpoints, CDC-ACM serial framing, and ROM download reboot handling.
---

The USB subsystem implements a composite USB 2.0 Full-Speed device on the ESP32-S3 using TinyUSB. It exposes two independent USB HID endpoints and one USB CDC-ACM virtual serial port. <!-- src: /firmware/main/usb/usb_descriptors.c -->

## Composite Descriptors & Interface Topology

The USB configuration descriptor defines three distinct interfaces: <!-- src: /firmware/main/usb/usb_descriptors.c -->

| Interface Index | Subclass / Protocol | Endpoint | Direction | Polling Interval | Purpose |
|---|---|---|---|---|---|
| **Interface 0 (HID Boot)** | Boot Keyboard | Endpoint 1 | IN | 1 ms (1000 Hz) | Key 1 and touchscreen Quick Retry reports. <!-- src: /firmware/main/usb/usb_hid.c#LINK_KEY1_TOUCH --> |
| **Interface 1 (HID Media)** | Custom Keyboard & Consumer | Endpoint 2 | IN | 1 ms (1000 Hz) | Key 2 reports and media playback gesture keys. <!-- src: /firmware/main/usb/usb_hid.c#LINK_KEY2 --> |
| **Interface 2 & 3 (CDC)** | CDC-ACM Serial Control | Endpoint 3 & 4 | IN / OUT | Bulk | Bidirectional Protobuf wire communication. <!-- src: /firmware/main/usb/usb_cdc.c --> |

A compile-time assertion enforces that TinyUSB is built with sufficient HID instances matching `usb_descriptors.c`: <!-- src: /firmware/main/usb/usb_descriptors.c -->
```c
_Static_assert(CFG_TUD_HID >= HID_INSTANCE_MEDIA + 1,
               "CONFIG_TINYUSB_HID_COUNT must match the descriptors");
```

## Split Endpoint Architecture (`usb_hid.c`)

Standard 1000 Hz USB HID keyboards share a single endpoint. If alternating keys actuate within 1 ms, the second key must wait for the host to complete the prior transfer. OPad completely removes this serialization jitter: <!-- src: /firmware/main/usb/usb_hid.c -->

- **Endpoint 1 (Keyboard Interface)**: Transmits 8-byte boot keyboard reports carrying Key 1 and touch Quick Retry (`0x35`).
- **Endpoint 2 (Media Interface)**: Transmits independent keyboard reports carrying Key 2.
- **Dynamic Key 3 Pairing (`usb_hid_set_key3_link`)**: On 3-key modules, Key 3 dynamically routes to whichever endpoint the secondary game key is not occupying. Key 1 and Key 2 never contend for the same USB endpoint buffer. <!-- src: /firmware/main/usb/usb_hid.c -->
- **Atomic Link Structures (`hid_link_t`)**: Core 0 (keypad) and Core 1 (touch) update atomic `change_seq` and `sent_seq` counters without taking mutex locks on the input path. <!-- src: /firmware/main/usb/usb_hid.c#hid_link_t -->

## CDC-ACM Communication & ROM Reboot (`usb_cdc.c`)

The CDC virtual serial port handles configuration updates, firmware flashing, and telemetry streaming: <!-- src: /firmware/main/usb/usb_cdc.c -->

- **Frame Feeding**: Received bytes are fed directly into the frame parser (`protocol_feed_cdc_bytes`).
- **DTR Drop Reset**: If the host drops DTR (e.g., closing `opadctl` or disconnecting the daemon), the receiver resets its framing buffer to prevent byte misalignment. <!-- src: /firmware/main/usb/usb_cdc.c -->
- **ROM Download Mode Reboot (`reboot_to_rom_download`)**:
  When commanded to enter bootloader flashing mode:
  1. Pulls USB D+ down (`SE0` bus state) for ~300 ms so the host operating system registers a real physical detachment.
  2. Sets `dp_pullup` back to 1 before dropping the GPIO pull override.
  3. Calls `tud_disconnect()` (never `tinyusb_driver_uninstall()`, which frees active mutexes).
  4. Triggers a hardware RTC watchdog reset (`WRITE_PERI_REG(RTC_CNTL_RESET_STATE_REG, ...)`).
  5. The ESP32-S3 boots directly into its internal ROM download mode (`303a:1001`), ready for `espflash` or `opadctl flash`. <!-- src: /firmware/main/usb/usb_cdc.c#reboot_to_rom_download -->
