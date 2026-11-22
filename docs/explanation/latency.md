---
title: Latency Engineering
description: How OPad minimizes keypress latency and jitter using dual USB HID endpoints, eager debouncing, Core 0 isolation, and on-device latency telemetry.
---

In rhythm games like *osu!*, timing consistency is measured in single-digit milliseconds, and jitter directly increases Unstable Rate (UR). OPad's primary engineering rule is **latency always wins** ([/AGENTS.md](/AGENTS.md) §1.1): the screen, storage, serial telemetry, tray, and `tosu` never join the keypress path.

## Dual USB HID Endpoints

Full-speed USB HID endpoints are polled by the host at `1 ms` intervals (`1000 Hz`, `bInterval = 1`). On a conventional single-endpoint keyboard:

- When Key 1 changes state, a HID report is queued on the endpoint.
- If Key 2 changes state `200 µs` later (normal during alternating streams), its report cannot be submitted until the host collects the first report—adding up to `1 ms` of variable jitter.

OPad avoids endpoint contention between the two primary game keys by splitting keyboard reports across two independent 1000 Hz IN endpoints ([/firmware/main/usb/usb_descriptors.c](/firmware/main/usb/usb_descriptors.c), [/firmware/main/usb/usb_hid.c](/firmware/main/usb/usb_hid.c)):

| Key | USB Interface & Endpoint | HID Report | Notes |
|---|---|---|---|
| **Key 1** | Interface 0 — `EP 0x81` (`HID_INSTANCE_KEYBOARD`, 1 ms) | Boot keyboard report (report ID `0`) | Shares endpoint with touchscreen Quick Retry (`` ` ``, usage `0x35`) during a map (`LINK_KEY1_TOUCH`). |
| **Key 2** | Interface 3 — `EP 0x84` (`HID_INSTANCE_MEDIA`, 1 ms) | Keyboard report (`MEDIA_REPORT_ID_KEYBOARD`) | Shares endpoint with outside-gameplay media/wheel swipe reports (`LINK_KEY2`). |
| **Key 3** (V3) | Dynamically routed (`usb_hid_set_key3`) | Paired onto `LINK_KEY1_TOUCH` or `LINK_KEY2` | Rides on whichever endpoint the other active game key is not using, so two game keys never share an endpoint. |

(Interface 1 & 2 are CDC-ACM using `EP 0x82`, `0x03`, and `0x83`; because the ESP32-S3 allows at most five IN endpoints including `EP0`, Key 2 shares Interface 3's IN endpoint `0x84`.)

### Lockless Sequence Tracking (`hid_link_t`)

Key events come from Core 0 (`keypad_task` or the Hall Effect backend), while touchscreen events come from Core 1 (`touch_retry`). To coordinate submissions without blocking mutexes on the keypress path, each endpoint link (`hid_link_t` in [/firmware/main/usb/usb_hid.c](/firmware/main/usb/usb_hid.c)) maintains atomic counters:

- `change_seq`: incremented atomically after updating key state so any submitter that loaded it before reading keys delivers at least that change.
- `sent_seq`: advanced with compare-and-swap whenever `tud_hid_n_keyboard_report` succeeds. If an endpoint is busy (`!tud_hid_n_ready`), the change stays pending (`sent_seq < change_seq`) and `tud_hid_report_complete_cb` resends it the moment the in-flight transfer finishes, preventing dropped edges or stuck keys.
- `pending_edge_us`: stores the microsecond timestamp of the oldest undelivered key edge on that link so deferred delivery latency is accurately measured when the completion callback sends it.

## Mechanical Input: Eager Debouncing

Mechanical switch contacts bounce for a few milliseconds after closing or opening. Deferral debouncing waits for the signal to settle before reporting a press, adding several milliseconds of delay to every tap.

OPad uses **eager debouncing** in [/firmware/main/input/debounce.c](/firmware/main/input/debounce.c) (`debounce_step`) and [/firmware/main/input/keypad.c](/firmware/main/input/keypad.c):

```text
Physical Switch:  [Close] ~~~bounce~~~ [Held] ~~~bounce~~~ [Open]
Eager State:      [PRESS immediately] -- lockout (default 5000 us) --> [resample] ... [RELEASE immediately]
Debounce Delay:   0 us added to first edge
```

1. **Immediate edge acceptance**: The GPIO interrupt (`gpio_isr_handler`, in `IRAM_ATTR`) timestamps the edge with `esp_timer_get_time()` and runs `debounce_step(..., DEBOUNCE_SOURCE_EDGE, s_config.debounce_us)`. Outside an active lockout window, the press or release is accepted immediately and `keypad_task` (Core 0, priority `configMAX_PRIORITIES - 1`) is woken via `vTaskNotifyGiveFromISR`.
2. **Lockout window**: Accepting a transition sets `lockout_end_us = now_us + lockout_us` (`DEBOUNCE_DEFAULT_US = 5000`, configurable from `500` to `20000` µs in [/firmware/main/input/debounce.h](/firmware/main/input/debounce.h)). Further edges during the lockout are suppressed.
3. **End-of-lockout resample**: Once `lockout_end_us` expires, `keypad_task` reads the physical GPIO once (`DEBOUNCE_SOURCE_RESAMPLE`). If the pin ended up in the opposite state during the lockout (for example, a glitch or a release that happened inside the lockout), it corrects the state and starts a new lockout window.

## Hall Effect Input Latency

Hall Effect switches have no metallic contact bounce and bypass mechanical debouncing completely ([/firmware/main/input/he_backend.c](/firmware/main/input/he_backend.c), [/firmware/main/input/he_spi_source.c](/firmware/main/input/he_spi_source.c)):

- **V1 (internal ADC1 DMA)**: 250 µs frames (`4 kHz`), median-filtered and stepped inside the IRAM frame ISR on Core 0, waking `he_task` only on key transitions.
- **V3 (external ADS7028 over 40 MHz SPI3)**: 50 µs frames (`20 kHz`) driven by a hardware `gptimer` waking the Core 0 `he_spi` task (`configMAX_PRIORITIES - 1`), which reads all 3 keys directly via `GPSPI3` registers and submits HID reports directly on state change.

## On-Device Latency Telemetry

Firmware measures the time from the physical edge timestamp (`edge_us`) until the HID report is accepted by TinyUSB in [/firmware/main/input/latency_stats.c](/firmware/main/input/latency_stats.c):

- **Histogram**: 500 atomic `10 µs` buckets (`BUCKET_US = 10`, `BUCKET_COUNT = 500`, spanning `0` to `5000 µs`; slower samples land in the last bucket while `s_max_us` records the exact maximum).
- **Percentiles**: `latency_stats_get` computes `p50`, `p99`, `p99.9`, `max_us`, sample count, and `deferred_reports` (edges that had to wait for an in-flight USB transfer). Inspect them anytime with `opadctl latency` or reset before a map with `opadctl latency --reset`.
- **Outlier tracking**: Any sample above `1000 µs` increments `s_outlier_count` and updates `s_outlier_max_us`.

## Core, Memory, and Flash Invariants

- **Core isolation**: Input handling (`keypad_task`, `he_task`, `he_spi`) and TinyUSB run on Core 0 at top priority (`configMAX_PRIORITIES - 1`); LVGL UI rendering, touchscreen polling, and serial protocol framing run on Core 1 ([/firmware/main/app_main.c](/firmware/main/app_main.c)).
- **No dynamic allocation on the keypress path**: Debounce states, HID links, and latency histograms are statically allocated.
- **IRAM placement**: Critical routines (`gpio_isr_handler`, `debounce_step`, `usb_hid_handle_key_event`, `latency_stats_record`, and the V1 ADC `on_frame` ISR) carry `IRAM_ATTR` so flash cache misses do not stall them.
- **No flash writes during gameplay**: NVS commits (counters, configuration, calibration) are deferred while the runtime state is `PLAYING` or `COOLDOWN` and flushed only in `IDLE` ([/firmware/main/runtime/runtime.c](/firmware/main/runtime/runtime.c)).
