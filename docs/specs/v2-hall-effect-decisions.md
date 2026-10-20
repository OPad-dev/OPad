# Hall Effect firmware: decision record

**Started:** 2026-10-19. Supersedes the conflicting parts of
`docs/specs/v2-rapid-trigger.md` (written before the board existed) and the
"Decided so far" table of `docs/specs/v2-hall-effect-design.md`; see
[Superseded](#superseded) for which.

Statuses: **Confirmed** (owner decision, change only on new evidence),
**Provisional** (in use for testing, not a product default), **Open** (needs
measurement or an owner decision).

Bench numbers below are from `docs/specs/v2-hall-effect-design.md` §10b
(V1.1 board, Gateron Jade Silent, in the case): rest 2084 / 2088 counts,
normal full press +447 / +508 counts, 1 ms-average rest sd 2.8 / 2.6,
single-conversion rest sd 5.9 / 5.3, about 0.875 mV per count at 12 dB.

## Implementation status

| Part | Implemented | Host-tested | Hardware-tested | Performance-validated |
|---|---|---|---|---|
| Decision engine (`firmware/main/input/he_rt.c`) | yes | yes (`firmware/test/host/test_he_rt.c`) | no | no |
| Settings → thresholds (`firmware/main/input/he_params.c`) | yes | yes (`firmware/test/host/test_he_rt.c`) | no | no |
| Shared key-event path (`firmware/main/input/key_event.c`): counters, press log, HID report, latency stats for both backends | yes, MX moved onto it | no (needs ESP-IDF) | no: MX needs a recheck on the pad | no: MX latency to remeasure |
| Frame parser (`firmware/main/input/he_frame.c`) | yes | yes (`firmware/test/host/test_he_frame.c`) | no | no |
| ADC DMA backend + HID integration (`firmware/main/input/he_backend.c`, `CONFIG_OSUPAD_HE_BACKEND`, default off) | yes, loads calibration from NVS | no (needs ESP-IDF) | no | no |
| Health and back-off state machine (`firmware/main/input/he_health.c`) | yes | yes (`firmware/test/host/test_he_health.c`) | no | no |
| Calibration record & A/B NVS storage (`firmware/main/input/he_cal_record.c`, `firmware/main/input/he_cal.c`) | yes | yes (`firmware/test/host/test_he_cal.c`) | no | no |
| Calibration capture algorithm (`firmware/main/input/he_cal_capture.c`) | yes | yes (`firmware/test/host/test_he_cal_capture.c`) | no | no |
| Keypad keycode staging (`firmware/main/input/keypad.c`) | yes | yes (`firmware/test/host/test_keypad_staged.c`) | no | no |
| Per-key HE settings in dedicated NVS record (`firmware/main/config/he_config_record.c`, `device_config.c`) | yes, blob v7 retained + 'he_cfg' key | yes (`firmware/test/host/test_config.c`) | no | no |
| Protocol & host integration (`protocol/osupad.proto`, `firmware/main/protocol/protocol.c`, `desktop/`) | yes (HelloAck capabilities, ConfigPayload HE settings, DeviceStatus calibration status) | yes (Rust workspace tests + host test_config) | no | no |
| Raw trace stream + analysis (`CONFIG_OSUPAD_HE_TRACE`, `scripts/he_trace.py`, `firmware/tools/he_replay.c`) | yes | analysis checked on a synthetic trace only | no | no |
| Calibration wizard UI on pad / app | no entry point yet; rapid trigger cannot be enabled | no | no | no |

The engine and builder are compiled into the firmware but nothing calls them
yet. The key-event move keeps every MX key-path function in IRAM
(`gpio_isr_handler`, `keypad_task`, `key_event_press_accepted`,
`key_event_report`, `usb_hid_handle_key_event`, checked with `nm` on
`build/opad-firmware.elf`); static DIRAM use is 90 % (34 KB left), which
limits what the HE backend can add to internal RAM.

## Decisions

### D1 — ADC sampling — Confirmed

- **Decision:** ADC1 continuous (DMA), key 1 (GPIO10, CH9) and key 2 (GPIO7,
  CH6) alternating, 64 000 conversions/s, 16 conversions (64 bytes) per DMA
  frame = 250 µs, 12-bit, 12 dB. The decision uses the newest valid
  conversion per key in each frame. The ID line is not sampled during play.
- **Rationale:** ADC1 is the only S3 ADC with working DMA; 250 µs frames keep
  the wait for a sample well under the 1 ms USB poll.
- **Trade-offs:** newest-sample noise is single-conversion noise (sd ~5.5),
  about twice the 1 ms-average noise the bench measured.
- **Driver facts (ESP-IDF v5.5.2, `components/esp_adc/adc_continuous.c`):**
  `on_conv_done` runs in the ADC ISR and gets the finished DMA buffer
  directly; the driver also copies every frame into its ringbuffer from the
  ISR; 5 DMA buffers rotate (`INTERNAL_BUF_NUM`), so a buffer is reused 1.25
  ms after it completes; the S3 allows up to 83 333 conversions/s
  (`SOC_ADC_SAMPLE_FREQ_THRES_HIGH`). `CONFIG_ADC_CONTINUOUS_ISR_IRAM_SAFE` is
  off by default: without it the ISR cannot run while flash is being written
  (NVS, OTA), and frames are lost.
- **Verification:** not yet measured on the pad (frame rate, overflows, 30 min
  stress).

- **Backend as built (`he_backend.c`):** the engine runs in the frame
  interrupt (`on_frame`, core 0, IRAM); the interrupt wakes `he_task` (core 0,
  `configMAX_PRIORITIES - 1`, IRAM) only when a key changes, and the task calls
  `key_event_report` as the MX keypad task does. No task wakes per frame. The
  latency stats measure frame interrupt → USB submit; the newest conversion is
  at most one conversion time (~16 µs) older than the interrupt. The driver's
  ringbuffer is kept at 2 frames and never read: the key path reads the DMA
  buffer in the interrupt. `CONFIG_OSUPAD_HE_BACKEND` selects
  `ADC_CONTINUOUS_ISR_IRAM_SAFE`; everything the interrupt calls is in IRAM
  and none of it references flash (checked with `nm`/`objdump` on
  `build-he/opad-firmware.elf`). Static DIRAM with HE on: 91.5 % (29 KB left).
- **Health (provisional, OPEN G):** checked every 2 ms by `he_task`. A key
  with no valid conversion (readings at 0 or 4095 do not count) for 5 ms is
  released and ignored until readings return; no frames for 5 ms releases
  both and restarts the ADC with a 10 ms → 1 s back-off. The board test skips
  its ID reading while HE owns ADC1.
- **Module swaps while powered:** the key pins carry the internal pull-up
  (a working sensor reads ~2 counts higher, bench §10 of the design doc), so
  with nothing plugged in or an MX module every reading sits at a rail and
  both keys fault. After 100 ms of that, the module monitor asks `he_task`
  to pause the ADC for one module-ID reading (`he_backend_probe_module`):
  nothing → the LCD says so; MX → a red notice asks for a restart (the app's
  Restart, or a replug), since the key input is only chosen at boot
  (`firmware/main/input/module_monitor.c`). Only `he_task` ever stops and
  starts the ADC: the driver's ADC1 lock is a mutex, and releasing it from
  another task panics (FreeRTOS's mutex-holder assert).
- **Factory thresholds (provisional, OPEN C):** plain mode, actuation 50 %,
  hysteresis 10 % of a 450-count span from a 2086 rest, floors 32 counts. For
  testing only.

### D2 — Filtering — Confirmed

- **Decision:** no moving average, no time-based debounce, no ID-line
  correction on the key path. Noise is resisted by hysteresis (plain mode) and
  rapid-trigger sensitivity, each held above a raw-count noise floor (OPEN A).
- **Evidence:** the ID-line ratio made noise worse, subtraction helped only a
  little (owner bench, 2026-10-19).

### D3 — Travel representation — Confirmed

- **Decision:** settings are in 0.1 % steps of the per-key calibrated span
  (`HE_PCT_FULL` = 1000 in `he_params.h`), a share of the magnetic signal, never
  shown as millimetres. Thresholds are converted to raw counts off the key
  path; the key path is integer compare and subtract only (`he_rt_step`).
- **The reading is never clamped** to the calibrated full press. A bottom dead
  zone, when set, only stops the *peak* from following (`peak_cap`); with the
  default of 0 nothing is capped.

### D4 — Rapid trigger state machine — Confirmed semantics, thresholds Open

Implemented in `he_rt.c`, per key:

| State | Logically | Leaves on |
|---|---|---|
| ARMED | up | reading ≥ `actuation` → DOWN (only after the key has read ≤ `release` once since boot, a fault or a threshold change) |
| PRESSED | down | reading ≤ region exit → UP, ARMED; rapid trigger: reading ≤ peak − `rt_release` → UP, RT_RELEASED |
| RT_RELEASED | up | reading ≤ region exit → ARMED (no event); reading ≥ trough + `rt_press` → DOWN |

- Region exit is `release` (actuation minus hysteresis) for plain mode and
  normal rapid trigger, `top_reset` for continuous rapid trigger.
- Defaults: rapid trigger **off**, continuous **off** even with rapid trigger
  on, bottom dead zone 0. Actuation, sensitivities and top reset: OPEN A, C, D.
- Threshold changes never press a key: they apply while the key is up, or at
  its next release if held, and then the key must read ≤ `release` before it
  can press (`he_rt_set_params`).
- An ADC fault releases a held key and the key must read ≤ `release` before it
  can press again (`he_rt_fault`).
- **Confirmed (owner, 2026-10-20):** a key held while the pad boots is not
  reported until it has been released once, calibrated or not. At boot a held
  key and a stale calibration (switch swapped, reading drifted) give the same
  reading; reporting it would let a stale calibration bring a key up stuck
  down with nobody pressing it. MX does report a held key at boot
  (`restart_debounce_from_pins` in `keypad.c`): a contact has no stored
  reading to go stale.
- Rapid trigger is refused without a valid calibration
  (`HE_PARAMS_RT_NEEDS_CALIBRATION`); a setting that maps below the noise
  floor is raised to it and reported, never applied silently.

### D5 — Hardware — Confirmed

DRV5055A3, ADC1, V1.1 HE board, current case. No A2, no external ADC unless a
measured limit requires it. GPIO6/4/2 stay reserved.

### D6 — USB delivery — Confirmed

Latest state only, no transition FIFO, no minimum press length. The existing
HID layer already works this way (`firmware/main/usb/usb_hid.c`): per-report
`change_seq`/`sent_seq`, a submit carries the current key state, and a change
that finds the endpoint busy is resent by `tud_hid_report_complete_cb` with
whatever the state is then. Key 1 + touch and key 2 are on separate endpoints,
so neither waits on the other. The HE backend calls the same
`usb_hid_handle_key_event`.

### D7 — Calibration — Confirmed policy; algorithm & storage implemented, entry point not built

Explicit, per key, rest then 5 normal presses (median peak), reachable from
the pad itself. Required before rapid trigger. Never invalidated automatically
by unusual readings; sensor health is separate and never erases calibration.
No blind rest capture at boot.

**First use and entry point (owner, 2026-10-20):** HE pads ship uncalibrated
and every pad calibrates on first use. Calibration belongs in the app; the pad
does it itself only when no app is there.

- **Locked until calibrated.** An HE pad without a valid calibration sends no
  key at all (Z/X off, touch is never Quick Retry), swipes are off, and it
  never switches to the playing screen. MX pads are unaffected. Basic typing
  still needs no software (AGENTS.md §1.2): the pad calibrates itself.
- **App connected:** the pad shows "Calibrate in the OPad app" with no
  Calibrate button, and the daemon opens the GUI's calibration page directly.
  Calibration then happens only in the app. An app too old to calibrate
  counts as no app.
- **No daemon ~5 s after boot:** a red screen recommends installing the app
  first and shows the download URL as text (`https://opad.dev/download`; the
  path is not final, so keep it in one place). A Calibrate button starts the on-pad flow. There is no "Later": the
  screen stays until calibration succeeds. No QR code (the PC can't scan it),
  and the pad never opens a browser: typing the URL would be scripted input
  (§1.6), and WebUSB landing pages or a USB drive would add USB interfaces to
  the keyboard path for a Chrome-only or click-to-open result.
- **The button** is a rectangle on the touch coordinates: a finger that lands
  in it and lifts without moving is a press; anything that moves is ignored.
- **During calibration** nothing reaches the PC. Keys come back once it is
  saved (or after a failed attempt, which returns to the lock screen).
- **Recalibration, rapid trigger and every HE setting are app only.** The pad
  has no HE settings of its own.

- **Record & NVS storage (`firmware/main/input/he_cal_record.c`, `firmware/main/input/he_cal.c`):**
  Pure-C record `he_cal_record_t` with schema versioning (`HE_CAL_VERSION = 1`),
  per-key `rest`, `full`, `valid` flags, and a 32-bit CRC (`he_cal_crc32`).
  Stored in NVS namespace `"osupad_cal"` using an A/B slot ping-pong scheme with
  32-bit monotonic sequence numbers. An interrupted write leaves the other slot intact.
  Corrupt or missing records fall back to uncalibrated factory defaults (`valid = false`)
  without erasing NVS. Writes are gated by runtime mode: deferred during `PLAYING` or
  `COOLDOWN`, executed only when `IDLE` (matching `device_config.c` and `counters.c`).
  Tested in `firmware/test/host/test_he_cal.c`.
- **Capture algorithm (`firmware/main/input/he_cal_capture.c`):**
  Pure C state machine (`he_cal_capture_t`). Captures rest over a still window
  (default 100 samples / 25 ms, noise spread ≤ 32 counts, rejected if reading is out
  of physical rest bounds or moving). Detects 5 stroke peaks on the sample stream; selects
  the **median** peak (ignoring extreme bottom-out / overtravel presses). Validates that
  the signal rises on press (`peak > rest`), travel span exceeds `min_span` (≥ 100 counts),
  and peak-to-peak consistency is within `max_peak_spread` (≤ 80 counts). Tested in
  `firmware/test/host/test_he_cal_capture.c` with synthetic bench noise.
- **Boot and runtime integration:**
  Loaded at boot in `he_backend_start()` and fed into `he_params_build()`, replacing
  provisional factory thresholds when valid. Rapid trigger remains disabled
  (`HE_PARAMS_RT_NEEDS_CALIBRATION`) until valid calibration is saved.
- **Wizard UI & Entry Point:**
  Decided above (first use and entry point) but not built: no lock screen, no
  on-pad flow and no calibration messages on the wire yet,
  so HelloAck sets `he_calibration = false`. Because rapid trigger requires an explicit valid
  calibration per D7 and `he_params_build`, rapid trigger cannot be enabled on any pad yet.
  Protocol reporting of calibration values is supported in `DeviceStatus.he_calibration`.

## Open

| ID | Question | Evidence so far | Needed |
|---|---|---|---|
| A | Minimum rapid-trigger movement (noise floor) | 1 ms-average p-p 17–24 counts; single-conversion sd 5.3–5.9 | raw 250 µs-frame traces at rest, held mid-travel, 30 min; replay false events vs floor |
| B | 64 vs 80 kS/s | 80 kS/s is inside the S3 limit | only if A shows a benefit |
| C | Default actuation point | subjective half press ≈ 23 % of key 1's span; 50 % stays test-only (owner, 2026-10-20) | strokes with the raw build |
| D | Top reset region | as A | as A |
| E | Host-visible latency, missed sub-poll taps | MX ~70 µs edge → submit | USB capture on the host |
| F | USB-only recovery details | A/B OTA with rollback until USB configures (`app_main.c`), 16 MB flash with ~11.9 MiB past the last partition (`partitions.csv`) | separate proposal before any partition change |
| G | ADC stall watchdog timeout | proposal ~5 ms | stress with display, USB, NVS/OTA writes |

## Reproducing the measurements

### Raw noise and the rapid-trigger floor (OPEN A, D)

Build the trace firmware (`CONFIG_OSUPAD_HE_TRACE`, in `firmware/main/Kconfig.projbuild`):

```sh
cd firmware
printf 'CONFIG_OSUPAD_HE_BACKEND=y\nCONFIG_OSUPAD_HE_TRACE=y\n' > /tmp/sdkconfig.het
idf.py -B build-het -D SDKCONFIG=build-het/sdkconfig \
  -D SDKCONFIG_DEFAULTS="sdkconfig.defaults;/tmp/sdkconfig.het" build
opadctl flash build-het/opad-firmware.bin
```

Then record each condition on its own, nothing moving during the recording:

```sh
scripts/he_trace.py record /dev/ttyACM0 60 rest.bin        # both keys untouched
scripts/he_trace.py record /dev/ttyACM0 60 held1.bin       # key 1 held still part way down
scripts/he_trace.py record /dev/ttyACM0 1800 rest30.bin    # long rest run
scripts/he_trace.py analyse rest.bin                       # again, later
```

The stream (`he_trace_task` in `firmware/main/input/he_backend.c`) carries
every conversion with a frame sequence number, so lost frames show up; the
pad's own counters (frames, late frames, longest gap, frame interrupt cost)
come once a second. The analysis (`firmware/tools/he_replay.c`) reports
spread and retrace per key for three estimators (newest conversion, mean of
the newest two, mean of all eight) and replays the trace through the
firmware's engine at sensitivities from 4 to 64 counts, counting false
events with the key held PRESSED and RT_RELEASED. The floor for OPEN A is
the smallest sensitivity with no false events over the longest hold, plus a
margin we agree on. The trace build loads core 1 and USB, so its latency
figures do not count.

## Superseded

| Earlier text | Now |
|---|---|
| v2-rapid-trigger §3.1: position in 0.01 mm, clamped to `[0, travel]` | D3: magnetic share, unclamped |
| v2-rapid-trigger §3.2: `continuous_rt` default on, bottom dead zone 0.20 mm | D4: off, 0 |
| v2-rapid-trigger §3.4 / §4: filtering, per-model curve, auto rest tracking | D2, D3; no automatic tracking |
| v2-hall-effect-design decision 9: adaptive ID-line correction | D1/D2: no ID line during play |
| v2-hall-effect-design "Switch swap": a different key-up reading at boot drops to defaults | D7: never invalidated automatically; may suggest recalibrating |
| v2-hall-effect-design §4: rest averaged at every boot | D7: no blind rest capture at boot |
| v2-hall-effect-design decision 8: module checked while idle | boot only, as today (`board_detect_module` in `app_main.c`) |
