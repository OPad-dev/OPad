---
title: Latency Engineering
description: Where keypress latency and jitter come from on a USB keyboard, and how OPad removes each source with split HID endpoints, eager debouncing, Core 0 isolation and on-device measurement.
sidebar:
  order: 2
---

In osu!, what hurts is not the average delay but its *variation*: a key that sometimes arrives 1 ms late widens the hit-error spread (Unstable Rate) even if the average is low. OPad's rule is **latency always wins** ([AGENTS.md](https://github.com/OPad-dev/OPad/blob/main/AGENTS.md) §1.1): the screen, storage, serial telemetry, tray and tosu never join the keypress path.

A key press passes four stages before the game sees it. Each one has a design choice behind it:

```mermaid
flowchart LR
    A["Switch closes"] -->|"debounce: 0 µs on the first edge"| B["Input task (Core 0)"]
    B -->|"its own endpoint, no waiting on the other key"| C["HID report queued"]
    C -->|"host polls every 1 ms"| D["Host / osu!"]
```

The pad measures the part it controls, from the switch edge to the report being queued in TinyUSB, and reports it as a histogram (see [On-device measurement](#on-device-measurement)). The last hop, up to 1 ms of USB polling, belongs to the host.

## Debouncing without delay

Switch contacts bounce for a few milliseconds when they close or open. The usual fix waits until the signal has been stable for a while before reporting it, which adds that wait to *every* press.

OPad debounces **eagerly** ([debounce.c](https://github.com/OPad-dev/OPad/blob/main/firmware/main/input/debounce.c), `debounce_step`): the first edge is believed immediately, and the bounce that follows is ignored.

```text
Switch contact:   ___|‾|_|‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾|_|‾|________
Reported:         ___|‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾‾|_____________
                     ^ press, 0 µs added           ^ release, 0 µs added
                     |-- lockout (5 ms) --| resample
```

1. **The edge is accepted at once.** The GPIO interrupt (`gpio_isr_handler` in [keypad.c](https://github.com/OPad-dev/OPad/blob/main/firmware/main/input/keypad.c), `IRAM_ATTR`) timestamps the edge with `esp_timer_get_time()` and calls `debounce_step` with `DEBOUNCE_SOURCE_EDGE`. Outside a lockout the change is accepted and `keypad_task` is woken with `vTaskNotifyGiveFromISR`.
2. **A lockout follows.** Accepting a change starts a lockout (`DEBOUNCE_DEFAULT_US = 5000`, configurable from 500 to 20 000 µs, [debounce.h](https://github.com/OPad-dev/OPad/blob/main/firmware/main/input/debounce.h)). Edges inside it are bounce and are dropped.
3. **The pin is read again when it ends.** `keypad_task` reads the pin once (`DEBOUNCE_SOURCE_RESAMPLE`). If it disagrees with the accepted state (a release that happened during the lockout, or a glitch), the state is corrected and a new lockout starts. No change is lost, only delayed by at most one lockout.

The trade-off is that a noise spike long enough to trigger the interrupt registers as a press, and is corrected (released) one lockout later. Real switches on a short cable make that rare; the lockout is configurable for setups where it is not.

Hall Effect switches have no contacts and skip all of this; see [Hall Effect and Rapid Trigger](hall-effect.md).

## One endpoint per game key

A full-speed USB HID endpoint is polled by the host once per `bInterval`, here 1 ms (1000 Hz). The device can have only one report waiting on an endpoint: a second report waits until the host has collected the first.

In a stream the two keys change well within 1 ms of each other. On a keyboard with one endpoint, Key 2's change made 200 µs after Key 1's waits for the next poll to collect Key 1's report: up to 1 ms of delay that depends on timing, which is exactly the kind of jitter that hurts.

OPad gives each game key its own endpoint ([usb_descriptors.c](https://github.com/OPad-dev/OPad/blob/main/firmware/main/usb/usb_descriptors.c), [usb_hid.c](https://github.com/OPad-dev/OPad/blob/main/firmware/main/usb/usb_hid.c)):

| Key | Interface and endpoint | Report | Shares the endpoint with |
|---|---|---|---|
| **Key 1** | Interface 0, `EP 0x81`, 1 ms (`HID_INSTANCE_KEYBOARD`) | Boot keyboard report, no report ID | The touchscreen's Quick Retry (`` ` ``) and, outside a map, swipe keys (`LINK_KEY1_TOUCH`) |
| **Key 2** | Interface 3, `EP 0x84`, 1 ms (`HID_INSTANCE_MEDIA`) | Keyboard report, `MEDIA_REPORT_ID_KEYBOARD` | Swipe media keys and the wheel (`LINK_KEY2`) |
| **Key 3** (HE V3) | Whichever of the two the other game key is not on (`usb_hid_set_key3`) | Same as that link | Rides with Key 2, unless Key 2 is a game key and Key 1 is not |

Interfaces 1 and 2 are the CDC-ACM serial port (`EP 0x82`, `0x03`, `0x83`). The ESP32-S3 allows five IN endpoints including EP0, so there is no room for a third HID interface: Key 2 shares the media interface's endpoint, where only swipes (never sent during a map) compete with it.

The result is that a key's report only ever waits for *its own* previous report, which is at least one debounce lockout old and has long been collected.

### Lockless sequence tracking (`hid_link_t`)

Key events come from Core 0, but touchscreen events come from Core 1 (`touch_retry`) and land in Key 1's report. A mutex between them would let a Core 1 task delay a key. Instead each report has a `hid_link_t` with three atomics:

- `change_seq` is incremented by whoever changes a key in that report. A submit that loaded it before reading the keys is guaranteed to carry at least that change.
- `sent_seq` records the newest change a successful submit carried, raised with compare-and-swap by whichever submitter succeeds. A change is pending while `sent_seq` is behind `change_seq`. Unlike a "pending" flag, one submitter can never clear another's change.
- `pending_edge_us` holds the edge time of the oldest undelivered change, so its latency is measured from the switch edge even when it is sent late.

If the endpoint is busy (`tud_hid_n_ready` false), the change stays pending, and `tud_hid_report_complete_cb` sends it the moment the transfer in flight completes. A change can be late by one report, but never lost, and a key can never stick.

## Keeping Core 0 clear

The two mechanisms above only work if nothing else delays the input task. [System Architecture](architecture.md) covers the core split; the rules on the key path are:

- **Core and priority**: the input tasks run on Core 0 at `configMAX_PRIORITIES - 1`, TinyUSB one level below. LVGL, the serial protocol and touch run on Core 1.
- **IRAM**: `gpio_isr_handler`, `debounce_step`, `usb_hid_handle_key_event`, `latency_stats_record` and the V1 ADC frame callback carry `IRAM_ATTR`, and [linker.lf](https://github.com/OPad-dev/OPad/blob/main/firmware/main/linker.lf) places `keypad_task`, `usb_hid` and TinyUSB's HID path in IRAM, so a flash cache miss caused by Core 1 cannot stall them.
- **No allocation**: debounce state, HID links and the latency histogram are static.
- **No flash writes during a map**: NVS writes wait for `IDLE` ([runtime.c](https://github.com/OPad-dev/OPad/blob/main/firmware/main/runtime/runtime.c)).

## On-device measurement

[latency_stats.c](https://github.com/OPad-dev/OPad/blob/main/firmware/main/input/latency_stats.c) records, for every key change, the time from the edge timestamp to the moment TinyUSB accepts the report:

- **Histogram**: 500 atomic buckets of 10 µs (`BUCKET_US`, `BUCKET_COUNT`), covering 0 to 5 ms; anything slower lands in the last bucket. `p50`, `p99` and `p99.9` are therefore multiples of 10 µs (the upper edge of a bucket), while `max_us` is exact.
- **Deferred reports**: how many changes had to wait for a transfer in flight.
- **Outliers**: any sample above 1000 µs is counted and its maximum kept; the protocol task reports them to the host as `DIAG_EVENT_LATENCY_OUTLIER` diagnostics.

`opadctl latency` reads the numbers and `opadctl latency reset` clears them before a run. The measurement stops at TinyUSB: what the host does with the report afterwards is not in these numbers. [Latency Measurement](../develop/latency-testing.md) describes the benchmark stages and the release gate built on them.
