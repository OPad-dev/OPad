---
title: Latency Engineering
description: In-depth explanation of OPad's sub-millisecond input pipeline, dual USB HID endpoints, eager debounce algorithms, and latency telemetry.
category: explanation
source_files:
  - /firmware/main/input/debounce.c
  - /firmware/main/input/keypad.c
  - /firmware/main/usb/usb_hid.c
  - /firmware/main/input/latency_stats.c
---

In rhythm games such as *osu!*, timing precision is evaluated in single-digit milliseconds. Any latency jitter or input transmission delay directly translates into unstable hit timing (Unstable Rate / UR). OPad treats latency as its primary invariant: **latency always wins** (Product Rule 1). Every component of the keypress path is optimized to deliver switch state changes to the host within 1 millisecond. <!-- src: /firmware/main/usb/usb_hid.c -->

## Dual USB HID Endpoints

A standard 1000 Hz USB HID keyboard endpoint is polled by the host operating system once every 1.0 ms. Under a conventional single-endpoint architecture:
- If Key 1 actuates, a HID keyboard report is queued on the endpoint.
- If Key 2 actuates 200 µs later (common during high-speed finger streams exceeding 200–300 BPM), the endpoint cannot accept the second report until the host has polled and acknowledged the first report (up to 1 ms later).
- This introduces up to 1 ms of artificial latency jitter between alternating finger taps.

### OPad's Split Endpoint Architecture
OPad solves this bottleneck by splitting the two primary game keys across two independent physical USB HID endpoints: <!-- src: /firmware/main/usb/usb_hid.c#hid_link_t -->

| Key | Endpoint / Interface | HID Report ID | Notes |
|---|---|---|---|
| **Key 1** | Endpoint IN 1 (Keyboard Interface) | Standard Boot Keyboard Report | Shares endpoint with touchscreen Quick Retry (`~`). <!-- src: /firmware/main/usb/usb_hid.c#LINK_KEY1_TOUCH --> |
| **Key 2** | Endpoint IN 2 (Media / Consumer Interface) | Custom Keyboard Report | Shares endpoint with outside-gameplay media gestures. <!-- src: /firmware/main/usb/usb_hid.c#LINK_KEY2 --> |
| **Key 3** | Dynamic Pairing (`usb_hid_set_key3_link`) | Paired with Key 1 or Key 2 | Routes to whichever endpoint is not used by the second active game key. <!-- src: /firmware/main/usb/usb_hid.c --> |

Because Key 1 and Key 2 reside on separate USB endpoints, Key 2 never waits for Key 1's USB transfer to complete. Both keys can submit reports to the host within the same 1 ms USB polling interval without blocking each other. <!-- src: /firmware/main/usb/usb_hid.c -->

### Lockless Sequence Tracking (`hid_link_t`)
To prevent race conditions between Core 0 (keypad task) and Core 1 (touch task) without using blocking mutexes on the input path, each HID endpoint link uses atomic sequence numbers: <!-- src: /firmware/main/usb/usb_hid.c#hid_link_t -->
- `change_seq`: Incremented atomically by whichever task records a key change.
- `sent_seq`: Updated when a USB report is successfully queued to the TinyUSB buffer.
- `pending_edge_us`: High-resolution microsecond timestamp of the physical switch transition, used to compute end-to-end hardware latency.

## Eager Debouncing Algorithm

Mechanical switches suffer from contact bounce, where metal contacts oscillate between open and closed for 1 to 5 ms upon physical actuation. <!-- src: /firmware/main/input/debounce.c -->

Conventional keyboards employ **deferral debouncing**: when a contact edge is detected, the controller waits 5 ms to ensure the signal settles before transmitting the press to USB. This adds an unavoidable 5 ms of input latency to every keystroke.

OPad implements **eager debouncing** (`debounce_step` in `firmware/main/input/debounce.c`): <!-- src: /firmware/main/input/debounce.c#debounce_step -->

```
Physical Switch:  [Press Contact] ~~~bounce~~~ [Solid Contact] ~~~bounce~~~ [Release]
Signal Level:     0 -> 1 -> 0 -> 1 -> 1 -> 1 -> 1 -> 1 -> 0 -> 1 -> 0 -> 0
Eager State:      [EMIT PRESS IMMEDIATELY] -------- Lockout Period --------> [EMIT RELEASE]
Latency Added:    0.0 ms
```

### Eager Debounce Operation
1. **Immediate Actuation**: When an edge interrupt transitions from released to pressed outside a lockout window, the press event is emitted immediately to Core 0 and TinyUSB (`DEBOUNCE_ACTION_PRESS`). The physical latency from contact closure to USB queue insertion is under 20 microseconds. <!-- src: /firmware/main/input/debounce.c#debounce_step -->
2. **Lockout Period**: An internal timer (`lockout_end_us = now_us + lockout_us`, default 5,000 µs) begins. Subsequent electrical bounces occurring during this lockout window are ignored. <!-- src: /firmware/main/input/debounce.c#debounce_step -->
3. **Resampling**: At the conclusion of the lockout period, the switch pin is resampled. If the switch remains physically held, the state remains held. When released, an immediate release event is emitted and a release lockout window is applied.

## Microsecond Latency Telemetry

OPad tracks real-time physical-to-USB delivery latency directly inside the firmware: <!-- src: /firmware/main/input/latency_stats.c -->

- **Hardware Timer**: Transitions are stamped using the ESP32-S3 high-resolution timer (`esp_timer_get_time()`, microsecond accuracy). <!-- src: /firmware/main/input/latency_stats.c#latency_stats_record -->
- **Histogram Buckets**: Firmware bins latency measurements into 500 atomic buckets of 10 µs each (`BUCKET_US = 10`, spanning 0 to 5,000 µs). <!-- src: /firmware/main/input/latency_stats.c#BUCKET_COUNT -->
- **Outlier Detection**: Any report taking longer than 1,000 µs (1.0 ms) increments `s_outlier_count` and records `s_outlier_max_us`, providing instant visibility into USB host scheduling delays or bus congestion. <!-- src: /firmware/main/input/latency_stats.c#latency_stats_record -->

## Memory & Execution Invariants

To eliminate latency spikes caused by garbage collection, memory allocation, or bus contention:
- **Zero Heap Allocations on Core 0**: No `malloc()` or `free()` calls exist on the keypad or USB execution path. All structures, debounce states, and buffers are statically allocated.
- **IRAM Placement**: Time-critical functions (`debounce_step`, `latency_stats_record`, GPIO interrupt service routines) are decorated with `IRAM_ATTR`, locking machine instructions into internal fast RAM to prevent instruction cache miss stalls. <!-- src: /firmware/main/input/debounce.c#debounce_step --> <!-- src: /firmware/main/input/latency_stats.c#latency_stats_record -->
- **Zero Flash Access During Play**: Flash cache access stalls the CPU for tens of milliseconds during erase and write operations. The firmware strictly disables NVS commits while playing maps (`PLAYING` / `COOLDOWN`).
