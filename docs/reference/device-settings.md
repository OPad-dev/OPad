---
title: Device Settings Reference
description: Complete catalog of OPad configuration settings, data types, valid ranges, default values, and NVS storage keys.
sidebar:
  order: 3
---

This page documents every configuration setting stored by the OPad firmware: keycodes, GPIO pin assignments, eager debounce lockout, display preferences, swipe actions, and Hall Effect parameters.
<!-- src: /firmware/main/config/device_config.h, /protocol/opad.proto -->

Settings are applied to RAM immediately. To avoid flash stalls on the keypress path, NVS writes are blocked during `PLAYING` and `COOLDOWN`; changes made mid-map are flagged dirty and flushed to NVS once the pad returns to `IDLE`.
<!-- src: /firmware/main/config/device_config.c#write_to_nvs, /firmware/main/config/device_config.c#device_config_flush -->

---

## Core Key & Hardware Settings (`opad_cfg` / `"cfg"`)

Stored in the `opad_cfg` NVS namespace under key `"cfg"` as `device_config_data_t` (blob version `7`).
<!-- src: /firmware/main/config/device_config.c#NVS_KEY, /firmware/main/config/device_config.h#DEVICE_CONFIG_VERSION -->

| Setting | Protobuf Field | C Type | Valid Range | Default Value | Description |
|---|---|---|---|---|---|
| Key 1 HID Usage | `key1_hid_usage` | `uint32_t` | `0x04` – `0xE7` | `0x1D` (`Z`) | USB HID keyboard usage sent for Key 1. |
| Key 2 HID Usage | `key2_hid_usage` | `uint32_t` | `0x04` – `0xE7` | `0x1B` (`X`) | USB HID keyboard usage sent for Key 2. |
| Key 1 GPIO | `key1_gpio` | `uint32_t` | `KEY_GPIO_ALLOWED` | `14` (P2 pin 11) | Switch GPIO for Key 1 (hand-wired MX). |
| Key 2 GPIO | `key2_gpio` | `uint32_t` | `KEY_GPIO_ALLOWED` | `9` (P2 pin 12) | Switch GPIO for Key 2 (hand-wired MX). |
| Debounce Lockout | `debounce_us` | `uint32_t` | `500` – `20000` µs | `5000` µs (5 ms) | Eager debounce lockout duration after an accepted switch edge. |
| Display Brightness | `brightness` | `uint32_t` | `0` – `100` % | `100` % | LCD LEDC PWM backlight percentage (GUI slider offers `10` – `100` %). |
| Sleep Timeout | `display_sleep_seconds` | `uint32_t` | `0` or `10` – `86400` s | `600` s (10 min) | Inactivity timeout before display sleep (`0` disables sleep; GUI slider offers `60` – `3600` s). |
| Gameplay Refresh Cap | `gameplay_display_hz` | `uint32_t` | `0` or `1` – `60` Hz (host clamps `1` – `30`) | `10` Hz | Maximum telemetry/display refresh rate while playing. |
<!-- src: /firmware/main/config/device_config.h, /firmware/main/config/config_validate.c#device_config_validate -->

### Allowed Switch GPIO Pins

Hand-wired switches must use one of the 14 GPIOs in `KEY_GPIO_ALLOWED`:
`2`, `4`, `6`, `7`, `9`, `10`, `11`, `12`, `13`, `14`, `15`, `16`, `18`, `21`.
<!-- src: /firmware/main/config/config_validate.c#KEY_GPIO_ALLOWED -->

Key 1 and Key 2 cannot share a GPIO. USB (`19`, `20`), UART0 (`43`, `44`), touch/IMU I2C (`47`, `48`), `CAM_PWDN` (`17`), and module-ID ADC (`8`) are rejected.
<!-- src: /firmware/main/config/config_validate.c#device_config_validate -->

---

## Extended Settings (`opad_cfg` / `"cfg_ext"`)

Stored separately under `"cfg_ext"` (`device_config_ext_t`, version `1`) so downgrading to older firmware does not break loading the v7 `"cfg"` blob.
<!-- src: /firmware/main/config/device_config.c#NVS_EXT_KEY, /firmware/main/config/device_config.h#device_config_ext_t -->

| Setting | Protobuf Field | C Type | Valid Range | Default Value | Description |
|---|---|---|---|---|---|
| Key 3 HID Usage | `key3_hid_usage` | `uint8_t` | `0` or `0x04` – `0xE7` | `0x06` (`C`) | USB HID usage for Key 3 on 3-key V3 modules (`0` sends nothing). |
| Game Keys | `game_keys` | `uint8_t` | Bitmask `1` – `6` | `0x03` (Keys 1 & 2) | Bitmask of 1 or 2 game keys (bit 0 = Key 1, bit 1 = Key 2, bit 2 = Key 3) counted in totals, tap rate, and map stats. `0` and `0x07` (all three) are rejected. |
<!-- src: /firmware/main/config/device_config.h#device_config_ext_t, /firmware/main/config/config_validate.c#device_config_validate_ext -->

Every physical key still maintains its own lifetime counter regardless of `game_keys`.
<!-- src: /firmware/main/config/device_config.h#DEVICE_CONFIG_DEFAULT_GAME_KEYS -->

---

## Touchscreen Swipe Actions

Stored inside the v7 `"cfg"` blob (`swipe_*_action`, `swipe_*_key`, `swipe_mods[4]`).
<!-- src: /firmware/main/config/device_config.h, /protocol/opad.proto#SwipeAction -->

| Gesture | Default Action | Wire Values (`SwipeAction`) | Modifier Bits (`swipe_*_modifiers`) |
|---|---|---|---|
| Swipe Left | `SWIPE_ACTION_PREV_TRACK` (`2`) | `1` = None, `2` = Prev Track, `3` = Next Track, `4` = Play/Pause, `5` = Mute, `6` = Key, `7` = Volume Up, `8` = Volume Down | Standard HID modifier byte (`0x01` = Left Ctrl, `0x02` = Left Shift, `0x04` = Left Alt, `0x08` = Left GUI) |
| Swipe Right | `SWIPE_ACTION_NEXT_TRACK` (`3`) | Same as above | Same as above |
| Swipe Up | `SWIPE_ACTION_VOLUME_UP` (`7`) | Same as above | Same as above |
| Swipe Down | `SWIPE_ACTION_VOLUME_DOWN` (`8`) | Same as above | Same as above |
<!-- src: /firmware/main/config/device_config.h, /protocol/opad.proto#SwipeAction -->

When an action is `SWIPE_ACTION_KEY` (`6`), the corresponding `swipe_*_key` must be in `0x04`–`0xE7`. During gameplay (`PLAYING`), keyboard swipe actions are never sent so they cannot inject modifiers or keys into a map.
<!-- src: /firmware/main/config/config_validate.c#swipe_valid, /firmware/main/input/touch_retry.c -->

---

## Hall Effect & Rapid Trigger Settings (`opad_cfg` / `"he_cfg"`, `"he_cfg3"`)

Stored in `opad_cfg` under `"he_cfg"` (Keys 1 & 2) and `"he_cfg3"` (Key 3) as packed `he_config_record_t` structs with magic `0x48454346` (`"HECF"`), version `1`, `flags` (per-key `no_floor` bits), and IEEE 802.3 CRC-32.
<!-- src: /firmware/main/config/device_config.c#NVS_HE_KEY, /firmware/main/config/he_config_record.h -->

All depths and distances are in `0.1 %` increments of the key's calibrated magnetic span (`0`–`1000`, where `HE_PCT_FULL = 1000` is 100.0%).
<!-- src: /firmware/main/input/he_params.h#HE_PCT_FULL -->

| Setting | Protobuf Field | Unit | Valid Range | Default Value | Description |
|---|---|---|---|---|---|
| Actuation Depth | `actuation` | `0.1 %` | `0` – `1000` | `500` (`50.0 %`) | Depth from rest required for the initial press. |
| Release Hysteresis | `release_hyst` | `0.1 %` | `0` – `1000` | `100` (`10.0 %`) | Upward distance above actuation required to release when Rapid Trigger is off. |
| Top Reset Distance | `top_reset` | `0.1 %` | `0` – `1000` | `100` (`10.0 %`) | Depth above which Continuous Rapid Trigger ends and returns to `HE_RT_ARMED`. |
| Rapid Trigger Press | `rt_press` | `0.1 %` | `0` – `1000` | `150` (`15.0 %`) | Downward movement required to re-press in Rapid Trigger mode. |
| Rapid Trigger Release | `rt_release` | `0.1 %` | `0` – `1000` | `150` (`15.0 %`) | Upward movement required to release in Rapid Trigger mode. |
| Bottom Dead Zone | `bottom_dead_zone` | `0.1 %` | `0` – `1000` | `0` (Off) | Distance near full press where peak tracking stops (`0` disables it). |
| Rapid Trigger Mode | `rt_mode` | Enum | `1` – `3` | `1` (`OFF`) | `1` = Off (fixed actuation/hysteresis), `2` = Normal Rapid Trigger (active below actuation), `3` = Continuous Rapid Trigger (active down to `top_reset`). |
| Allow Below Noise Floor | `allow_below_noise_floor` | `bool` | `false` / `true` | `false` | Drops raw-count noise floors to `1` count for this key so settings apply as-is (sensor noise can then trigger presses). |
<!-- src: /firmware/main/config/he_config_record.h, /firmware/main/input/he_params.h, /protocol/opad.proto#HeKeySettings -->

### Noise Floor Enforcement

Unless `allow_below_noise_floor` (`no_floor`) is enabled, `he_params_build` clamps movement thresholds and top-reset clearance above raw-count noise floors (`min_movement`, `min_top_reset`) and returns explicit `HE_PARAMS_*_RAISED` status bits so the GUI can display the real effective press point.
<!-- src: /firmware/main/input/he_params.h#he_params_build -->
