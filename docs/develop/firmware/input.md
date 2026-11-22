---
title: Firmware Input Subsystem
description: Implementation reference for the firmware input pipeline, mechanical switch debouncing, Hall Effect ADC sampling, and unified key event dispatching.
category: reference
source_files:
  - /firmware/main/input/keypad.c
  - /firmware/main/input/debounce.c
  - /firmware/main/input/he_backend.c
  - /firmware/main/input/he_rt.c
  - /firmware/main/input/key_event.c
---

The input subsystem captures physical switch contacts and magnetic flux transitions, applies debouncing or rapid trigger algorithms, updates monotonic keypress counters, and submits HID reports to USB endpoints. All input code runs strictly on Core 0 with `configMAX_PRIORITIES - 1`. <!-- src: /firmware/main/input/keypad.c -->

## Mechanical Keypad Pipeline (`keypad.c` & `debounce.c`)

For mechanical switches, the pipeline consists of edge-triggered GPIO interrupts coupled with eager debouncing: <!-- src: /firmware/main/input/debounce.c -->

- **Interrupt Handling**: Pin state changes on Key 1 (GPIO 14) and Key 2 (GPIO 9) trigger an edge-sensitive GPIO interrupt (`gpio_isr_handler`). The ISR stamps transition time with `esp_timer_get_time()` and pushes the event to `s_input_task_handle` via direct task notifications (`vTaskNotifyGiveFromISR`). <!-- src: /firmware/main/input/keypad.c -->
- **Eager Debounce State Machine (`debounce_step`)**:
  - `DEBOUNCE_SOURCE_EDGE`: When an edge arrives, if the level differs from `accepted_pressed` and the current timestamp exceeds `lockout_end_us`, the new state is accepted immediately (`DEBOUNCE_ACTION_PRESS` or `RELEASE`). Lockout end is advanced: `lockout_end_us = now_us + lockout_us`. <!-- src: /firmware/main/input/debounce.c#debounce_step -->
  - `DEBOUNCE_SOURCE_SAMPLE`: At lockout expiration, the pin is resampled. If the physical level matches the accepted state, debouncing stabilizes; if contact bounced back, release is scheduled.

## Hall Effect Pipeline (`he_backend.c` & `he_rt.c`)

For magnetic switches, input is driven by continuous ADC sampling rather than GPIO edges: <!-- src: /firmware/main/input/he_backend.c -->

- **Continuous DMA Sampling**: Hardware ADC1 samples Key 1 (CH9) and Key 2 (CH6) in continuous DMA ping-pong buffers at `HE_SAMPLE_HZ` (250 µs per conversion frame). <!-- src: /firmware/main/input/he_backend.c -->
- **Median Filtering (`he_frame.h`)**: The IRAM frame interrupt calculates the median of all valid conversions in the 250 µs window to filter electromagnetic noise without adding group delay.
- **Rapid Trigger Step (`he_rt_step`)**:
  - Compares median flux against `p->actuation` and dynamic `p->rt_release` / `p->rt_press` thresholds. <!-- src: /firmware/main/input/he_rt.c#he_rt_step -->
  - Emits `HE_RT_DOWN` or `HE_RT_UP` directly within the interrupt.
  - If and only if a key state transition occurs, the interrupt notifies `he_task` to queue the USB HID report.

## Unified Key Event Dispatch (`key_event.c`)

Both mechanical and Hall Effect pipelines converge on the unified key event dispatcher (`firmware/main/input/key_event.c`): <!-- src: /firmware/main/input/key_event.c -->

- **Atomic Press Counters**: Increments `s_lifetime_presses[key_id]` and map-specific counter `s_map_presses[key_id]`.
- **Latency Telemetry**: Calls `latency_stats_record()` with the microsecond elapsed delta between physical contact detection and HID buffer enqueue. <!-- src: /firmware/main/input/key_event.c -->
- **USB Report Trigger**: Dispatches the state to `usb_hid_set_key1_pressed()` or `usb_hid_set_key2_pressed()`, signaling TinyUSB to transmit on Endpoint IN 1 or Endpoint IN 2. <!-- src: /firmware/main/usb/usb_hid.c -->
- **Diagnostic Logging**: Appends timestamped transitions to the circular debug log (`press_log_record`).
