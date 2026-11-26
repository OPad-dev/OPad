---
title: Firmware Config and NVS Subsystem
description: Implementation reference for ESP32-S3 NVS namespaces, legacy osupad->opad migration, config versioning, and A/B CRC32 calibration records.
sidebar:
  order: 6
---

Persistent pad state is stored in the ESP32-S3 NVS partition (`nvs` at `0x9000`, 24 KB, in [/firmware/partitions.csv](https://github.com/OPad-dev/OPad/blob/main/firmware/partitions.csv)). All NVS commits are forbidden during `PLAYING` and `COOLDOWN` and deferred to `IDLE` ([runtime state machine](overview.md#runtime-state-machine-runtimec)). What each setting means to a user is in [Device Settings](../../reference/device-settings.md); how settings survive an app or firmware rollback is in [App and Pad Versions](../../explanation/app-and-pad-versions.md).

## NVS Namespaces & Migration (`nvs_ns.c`)

Defined in [/firmware/main/config/nvs_ns.c](https://github.com/OPad-dev/OPad/blob/main/firmware/main/config/nvs_ns.c):

| Enum (`nvs_ns_id_t`) | Active Name | Legacy Name | Stored Data |
|---|---|---|---|
| `NVS_NS_COUNTERS` | `"opad"` | `"osupad"` | `generation`, `key1_cnt`, `key2_cnt`, `key3_cnt` ([/firmware/main/counters/counters.c](https://github.com/OPad-dev/OPad/blob/main/firmware/main/counters/counters.c)) |
| `NVS_NS_CONFIG` | `"opad_cfg"` | `"osupad_cfg"` | `"cfg"` (`device_config_data_t` v7 including `owner_id[16]`), `"cfg_ext"` (`device_config_ext_t` v1), `"he_cfg"`, `"he_cfg3"` ([/firmware/main/config/device_config.c](https://github.com/OPad-dev/OPad/blob/main/firmware/main/config/device_config.c)) |
| `NVS_NS_CAL` | `"opad_cal"` | `"osupad_cal"` | V1 Hall Effect A/B calibration (`cal_a`, `cal_b`) ([/firmware/main/input/he_cal.c](https://github.com/OPad-dev/OPad/blob/main/firmware/main/input/he_cal.c)) |
| `NVS_NS_CAL3` | `"opad_cal3"` | `"osupad_cal3"` | V3 Hall Effect A/B calibration (`cal_a`/`cal_b` for Keys 1–2, `cal3_a`/`cal3_b` for Key 3) ([/firmware/main/input/he_cal.c](https://github.com/OPad-dev/OPad/blob/main/firmware/main/input/he_cal.c)) |
| `NVS_NS_UI` | `"opad_ui"` | `"osupad_ui"` | Custom screen layouts (`layout0`, `layout1`) ([/firmware/main/ui/ui_store.c](https://github.com/OPad-dev/OPad/blob/main/firmware/main/ui/ui_store.c)) |

At boot, `nvs_ns_migrate()` copies every entry from each legacy `osupad*` namespace into its `opad*` namespace, commits all entries, and writes the `NVS_NS_MIGRATED_KEY` marker (`"_migrated"` = `1`) last. If any step fails or power drops mid-copy, the partial new namespace is erased, `nvs_ns_name()` keeps returning the legacy name, and the next boot starts the copy again from scratch. The legacy namespace is read only and never written again, so firmware from before the rename still finds its data after a rollback. Host-tested in [/firmware/test/host/test_nvs_ns.c](https://github.com/OPad-dev/OPad/blob/main/firmware/test/host/test_nvs_ns.c).

## Config Blob Compatibility (`device_config.h`, `config_validate.c`)

- `device_config_data_t` (version `7`) supports loading shorter legacy blobs v1–v6 (`device_config_blob_layout_ok` in [/firmware/main/config/config_validate.c](https://github.com/OPad-dev/OPad/blob/main/firmware/main/config/config_validate.c)) and filling newer fields with defaults.
- Settings added after v7 (`key3_usage`, `game_keys`) live in a separate `"cfg_ext"` key (`device_config_ext_t`) so rolling back to older firmware never invalidates `"cfg"`.
- Hall Effect settings live in `"he_cfg"` (Keys 1–2) and `"he_cfg3"` (Key 3) as `he_config_record_t` (`HE_CONFIG_RECORD_MAGIC = 0x48454346` `"HECF"`, version `1`, `flags` for `no_floor`, IEEE 802.3 CRC-32) ([/firmware/main/config/he_config_record.h](https://github.com/OPad-dev/OPad/blob/main/firmware/main/config/he_config_record.h)).

## Hall Effect Calibration Records

Atomic A/B records (`he_cal_record.h`, `he_cal.c`).

Implemented in [/firmware/main/input/he_cal_record.h](https://github.com/OPad-dev/OPad/blob/main/firmware/main/input/he_cal_record.h) and [/firmware/main/input/he_cal.c](https://github.com/OPad-dev/OPad/blob/main/firmware/main/input/he_cal.c):

- **Struct (`he_cal_record_t`)**: `magic = 0x4F504341` (`"OPCA"`), `version = 1`, `seq` (monotonic `uint32_t` with wrap-safe comparison), `keys[2]` (`rest`, `full`, `valid`), and IEEE 802.3 `crc32`.
- **Factory defaults**: `rest = 2086`, `full = 2536`, used with `valid = false` until a calibration is saved; a pad with any key not valid is locked ([input](input.md#calibration-and-the-lock-decision-d7)).
- **Ping-pong A/B writes**: Each store alternates between slot A and slot B (`cal_a`/`cal_b`, and `cal3_a`/`cal3_b` for Key 3 on V3). At boot, both slots are read and CRC-validated, selecting the valid slot with the newer `seq`. Missing or corrupt records fall back to provisional factory defaults (`valid = false`) without erasing NVS.

## Deferred Flushing

When `SetConfig`, `SetLayout`, or calibration updates arrive during `PLAYING` or `COOLDOWN`, firmware updates RAM immediately, sets dirty flags, and waits for `runtime_supervisor_task` ([/firmware/main/runtime/runtime.c](https://github.com/OPad-dev/OPad/blob/main/firmware/main/runtime/runtime.c)) (the task named `runtime_task`) to flush them once `OPAD_STATE_IDLE` is reached: `device_config_flush()`, `ui_store_flush_dirty()` ([UI](ui.md#layouts-and-deferred-persistence-ui_storec)) and `he_cal_flush()`. Counters follow their own checkpoint rules ([counters](counters.md)).
