---
title: Firmware Config and NVS Subsystem
description: Implementation reference for Non-Volatile Storage (NVS) management, namespace migration, atomic A/B records, and CRC32 verification.
---

The configuration and storage subsystem manages persistent pad state inside the ESP32-S3 Non-Volatile Storage (NVS) flash partition. It isolates flash erase and write operations from active gameplay, manages namespace migration, and ensures atomic record updates using checksummed A/B slots. <!-- src: /firmware/main/config/device_config.c -->

## NVS Namespaces & Migration (`nvs_ns.c`)

Storage is divided into five distinct logical namespaces (`nvs_ns.c`): <!-- src: /firmware/main/config/nvs_ns.c -->

| Namespace Constant | Active Name | Legacy Name | Contents |
|---|---|---|---|
| `NVS_NS_COUNTERS` | `"opad"` | `"osupad"` | Monotonic lifetime keypress counters, counter generation, owner identity. <!-- src: /firmware/main/config/nvs_ns.c --> |
| `NVS_NS_CONFIG` | `"opad_cfg"` | `"osupad_cfg"` | Debounce window, key pin assignments, backlight brightness, swipe bindings. <!-- src: /firmware/main/config/nvs_ns.c --> |
| `NVS_NS_CAL` | `"opad_cal"` | `"osupad_cal"` | Hall Effect calibration records for Key 1 and Key 2. <!-- src: /firmware/main/config/nvs_ns.c --> |
| `NVS_NS_CAL3` | `"opad_cal3"` | `"osupad_cal3"` | Hall Effect calibration records for Key 3 (V3 module). <!-- src: /firmware/main/config/nvs_ns.c --> |
| `NVS_NS_UI` | `"opad_ui"` | `"osupad_ui"` | Custom serialized widget screen layouts (`idle` and `playing`). <!-- src: /firmware/main/config/nvs_ns.c --> |

To guarantee backward compatibility (Product Rule 3), the namespace resolver checks for the existence of legacy namespace names before opening active ones; existing pads smoothly read their legacy records without losing data. <!-- src: /firmware/main/config/nvs_ns.c -->

## Atomic A/B Record Architecture (`he_cal_record.c`)

Flash writes risk corruption if power is interrupted mid-transaction. Critical records (such as Hall Effect calibration and configuration) employ an alternating A/B ping-pong storage scheme: <!-- src: /firmware/main/input/he_cal_record.c -->

- **Record Structure (`he_cal_record_t`)**:
  - `magic`: 32-bit validation magic (`HE_CAL_MAGIC = 0x4845434C`). <!-- src: /firmware/main/input/he_cal_record.c -->
  - `version`: Format revision (`HE_CAL_VERSION = 1`).
  - `seq`: Monotonically incrementing 32-bit sequence number.
  - `keys[2]`: Per-key calibration structures (rest, bottom, span).
  - `crc32`: Standard IEEE 802.3 CRC32 checksum covering the entire record. <!-- src: /firmware/main/input/he_cal_record.c#he_cal_crc32 -->
- **Two Slots (`cal_a` and `cal_b`)**:
  - At boot, both slots are read and validated against their CRC32 checksums.
  - If both are valid, the slot with the higher sequence number (`seq`) is loaded.
  - When writing updates, the inactive slot is written and committed first before invalidating or superseding the previous slot. An interrupted write leaves the previous valid record intact.

## Runtime Staging & Flash Invariant

Flash erase cycles take between 20 ms and 100 ms, stalling CPU cache execution across both Xtensa cores: <!-- src: /firmware/main/config/device_config.c -->
- When the host sends `SetConfig` or `SetLayout`, settings are applied to active RAM variables immediately (`s_config`).
- If the device state is `OPAD_STATE_PLAYING` or `OPAD_STATE_COOLDOWN`, flash commits are deferred (`s_dirty = true`).
- The runtime supervisor flushes pending dirty configuration records to NVS only after the pad returns to `OPAD_STATE_IDLE`.
