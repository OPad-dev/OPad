# OPad USB CDC Framing & Protocol

## 1. Framing Specification (§23)
USB CDC is a streaming byte-oriented transport. Each protobuf-encoded envelope
is prefixed with a 4-byte header, in one of two framings:

```text
marked (current):  | 0xAA | 0x55 | length (uint16-LE) | protobuf envelope |
legacy:            |     length (uint32-LE)            | protobuf envelope |
```

### Mixed versions

The app and the pad firmware are updated independently, so both sides speak
both framings:

- **The pad** parses either framing and answers in the framing of the last
  valid frame the host sent. It sends nothing, not even log batches, until the
  host has sent a frame on the current connection, and it forgets the host's
  framing when the port closes (DTR drops). An app from before the marker
  therefore only ever sees legacy frames.
- **The app** sends Hello alternately in the marked and legacy framing, every
  400 ms, until a HelloAck arrives. From then on it sends and parses only the
  framing that HelloAck came in. Firmware from before the marker reads a marked
  Hello as an impossible length and discards it, then answers the legacy one.
  After 10 unanswered Hellos the app closes and reopens the port, which resets
  the pad's parser.

The framings cannot be confused: a legacy header beginning `AA 55` would claim
at least 0x55AA bytes, more than any frame may hold.

### Protocol Constraints:
- Maximum frame size: **8192 bytes** (`PROTOCOL_MAX_FRAME_SIZE`), so N ≤ 8188.
- **Resynchronisation.** Both parsers (`firmware/main/protocol/frame_parser.c`,
  `opad-protocol`) slide forward one byte at a time until they find a
  plausible header: `AA 55` with a length ≤ 8188, or a legacy length of 2..8188
  (its top two bytes zero). Stray bytes (ROM bootloader chatter, a plain-text
  command, a dropped byte) cost at most the frame they land in. Junk that
  happens to look like a legacy header can hold a parser until the stale-frame
  timeout below; once the app knows the pad's framing it no longer considers
  the other one at all.
- The host skips a frame whose payload fails to decode by one byte and rescans,
  in case the header was a false start inside noise.
- **Stale partial frames.** Frames are written whole on both sides, so a partial
  frame that sits for 500 ms is dropped: the pad simply discards it; the host
  discards it and re-sends `Hello`.
- The pad queues a device frame only when the whole of it fits the CDC TX FIFO;
  otherwise it drops the frame and records `DIAG_EVENT_CDC_WRITE_DROPPED`.
- Disconnections or serial port resets discard any partial buffer.

---

## 2. Protobuf Message Envelopes

All messages are defined in [`protocol/osupad.proto`](../protocol/osupad.proto) using NanoPB-compatible schemas.

Every message transmitted in either direction is wrapped in a top-level envelope containing a 32-bit sequence number:

```protobuf
message HostToDevice {
    uint32 sequence_number = 1;
    oneof payload { ... }
}

message DeviceToHost {
    uint32 sequence_number = 1;
    oneof payload { ... }
}
```

---

## 3. HostToDevice Payloads

| Tag | Message | Description |
|---|---|---|
| 1 | `Hello` | Initial handshake upon port discovery. Contains `protocol_version` (1) and `client_version`. |
| 2 | `TimeSync` | Real-time clock synchronization with year, month, day, hour, minute, second. |
| 3 | `SetConfig` | Applies device parameters: `key1_hid_usage`, `key2_hid_usage`, `debounce_us` (500–20,000 µs), `brightness` (0–100%), `display_sleep_seconds`, `gameplay_display_hz` (1–60), and `key1_gpio` / `key2_gpio` (switch pins; 0 keeps the current pin). Pins must be one of the supported header GPIOs 2, 4, 6–16, 18, 21 and differ; a pin move is applied by the keypad task once both keys are released. `swipe_up_action` / `swipe_down_action` / `swipe_left_action` / `swipe_right_action` (`SwipeAction`; 0 keeps the current action) and the matching `swipe_*_key` (HID usage for `SWIPE_ACTION_KEY`; 0 keeps the current key) and `swipe_*_modifiers` (HID modifier bits held with the key, e.g. Ctrl for osu!'s Ctrl+O; always applied, 0 = none) set what each touchscreen swipe does; firmware without swipes ignores them and reports 0, which the host reads as the defaults (up/down volume, left/right previous / next track). Field 14 is reserved (an unreleased `swipe_vertical`). *(Note: `press_color_rgb` is deprecated; highlight colors now come from layouts).* |
| 4 | `CounterSync` | Lifetime counter synchronization with `target_state` (`counter_generation`, `lifetime_key1`, `lifetime_key2`) and `force_restore` flag. |
| 5 | `HostStatus` | Sent every second and on every change: `tosu_connected`, `playing` (osu! is playing a map; holds the pad in PLAYING), `play_id` (changes on every attempt; the pad zeroes its map counters) and `osu_active` (volume swipes go to osu!: in front on Windows, osu!lazer running on Linux; false from hosts predating swipes). While `tosu_connected` is false, or no `HostStatus` came for 3 s, every touch is Quick Retry and swipes are off (`firmware/main/input/touch_retry.c`). |
| 6 | `DataUpdate` | Real-time telemetry batch containing up to 32 `DataValue` elements (`source` ID `0..95`, variant of `number`, `text` or `clear`). See [DataUpdate sources](#dataupdate-sources-and-widget-flags). |
| 7 | `SetLayout` | Uploads custom screen layout JSON / binary definition for `screen` ID `0..3`. |
| 8 | `ResetLayout` | Resets specified screen to firmware default layout. |
| 9 | `RequestLogs` | Requests a batch of buffered diagnostic logs from device RAM. |
| 10 | `RequestStatus` | Requests instantaneous device status and hardware statistics. |
| 11 | `ResetLatencyStats` | Clears cumulative latency statistics (min, max, average, and histogram buckets). |
| 12 | `EnterBootloader` | Instructs device to restart immediately into native USB ROM DFU bootloader for flashing. |
| 14 | `ClaimOwnership` | Records which host install owns this pad (§W3-1, §W3-2). Carries a 16-byte `owner_id`. It is an NVS write, so the firmware honours it **only in IDLE**, never during `PLAYING` or `COOLDOWN` (P1-3); the host only ever sends it at connect time, which is already an IDLE-only moment. An all-zero `owner_id` is **refused**, so there is no wire path to unpairing — that is a documented reflash (§W3-4, `docs/recovery.md` §7). Re-claiming by the current owner writes nothing. |
| 16 | `run_board_test` | Asks the pad to test the carrier and input module PCBs; answered with `BoardTestResult`. See [below](#boardtestresult-devicetohost-field-10-and-helloackboard_test-field-12). |

### DataUpdate sources and widget flags

Source ids are wire format, defined in `firmware/main/ui/core/ui_ids.h`
(`ui_source_t`) and mirrored by `desktop/crates/opad-model/src/ui_source.rs`;
they are only ever appended. A layout widget binds one source by id; the
designer shows the stable names below.

| Ids | Names | Sent by |
|---|---|---|
| 1–19 | `map.*` (title, stars, `map.bpm`, progress, ...) | daemon, from tosu |
| 20–37 | `play.*` (pp, accuracy, combo, grade, hits, ...) | daemon, from tosu |
| 40–50 | `profile.*`, `session.*`, `game.state` | daemon, from tosu |
| 51–59 | `play.ppm*`, `play.k1_ppm*`, `play.k2_ppm*` (tap rate) | daemon |
| 60–73 | `pad.*` (key counters, `pad.kps`, clock, key down) | pad itself |
| 74–79, 83 | `history.*` (tap rate history) | daemon |
| 80–82 | `status.pc`, `status.tosu`, `status.osu` | pad itself |

**Tap rate (PPM).** PPM is *presses per minute*: the player's physical key
press rate, worked out by the daemon from the key counts. It is not the
beatmap's musical BPM (`map.bpm`, id 12), even though players often say "BPM"
for it. All PPM values are integer numbers except `history.period`, which is
text.

| Id | Name | Meaning |
|---|---|---|
| 51 | `play.ppm` | Combined live rate (both keys): the rolling rate of the last presses, falling once the player clearly stops and 0 when not tapping. Always set during an attempt; cleared only without tosu. |
| 52 | `play.ppm_avg` | Combined average of the current attempt |
| 53 | `play.ppm_peak` | PEAK of the current attempt: the fastest combined rate over 7 presses in a row, which catches a short burst at its real speed |
| 54–56 | `play.k1_ppm`, `play.k1_ppm_avg`, `play.k1_ppm_peak` | Same for K1 alone |
| 57–59 | `play.k2_ppm`, `play.k2_ppm_avg`, `play.k2_ppm_peak` | Same for K2 alone |
| 74 | `history.ppm_avg` | Combined average over the history period |
| 75 | `history.ppm_peak` | Combined peak over the history period |
| 76–77 | `history.k1_ppm_avg`, `history.k1_ppm_peak` | K1 over the history period |
| 78–79 | `history.k2_ppm_avg`, `history.k2_ppm_peak` | K2 over the history period |
| 83 | `history.period` | The period as the pad shows it: `LAST 30 DAYS`, `ALL TIME` |
| 84 | `play.ppm_song` | Song rate of the current attempt: every press over the song time from the first note to the last, breaks included, on the song's clock (game paused does not count, speed mods converted to real time). 0 for the first 5 s of song. |
| 85 | `history.ppm_song` | Song rate over the history period, weighted by song time |
| 86 | `play.ppm_song_peak` | Song peak of the current attempt: the most presses in any 10 s of song, counted like `play.ppm_song`. Empty until 10 s of song have been played. (`play.ppm_peak` is PEAK, the fastest 7 presses in a row; this is the fastest 10 s.) |
| 87 | `history.ppm_song_peak` | Best song peak over the history period |

Averages and peaks of an attempt stay until the next attempt starts. The
`history.*` sources are sent whenever the daemon knows them, with or without
tosu. The pad computes none of this and stores none of it.

**Clearing.** A `clear` value empties a source; widgets with
`UI_FLAG_HIDE_WHEN_EMPTY` then disappear, other widgets show `-` for a number
and nothing for text. The pad clears sources itself when the host goes away:

- `HostStatus.tosu_connected = false` clears ids 1–59, 84 and 86 (tosu data and
  the current attempt's tap rate).
- The port closing (`status.pc` → 0) clears everything the daemon sends
  (1–59, 74–79, 83–87) and zeroes `status.tosu` / `status.osu`. The daemon
  resends every value after a reconnect.

**Widget flags** (`ui_widget_t.flags`):

| Bit | Name | Meaning |
|---|---|---|
| `0x01` | `UI_FLAG_BG_FILL` | Fill the widget with `bg` |
| `0x02` | `UI_FLAG_HIDE_WHEN_EMPTY` | Hidden while the bound source has no value |
| `0x04` | `UI_FLAG_BORDER` | 1 px border (RECT, KEYCARD) |
| `0x08` | `UI_FLAG_KEY_RATE` | KEYCARD: show that key's current PPM (`play.k1_ppm` for a K1 card, `play.k2_ppm` for K2) as `198 PPM` right of the title. Hidden, with the title centered as before, while the rate is empty. |

**Mixed versions.** Old firmware drops `DataValue`s with unknown source ids
and ignores unknown flag bits, so a new app can send the tap rate sources and
set `UI_FLAG_KEY_RATE` safely. It does, however, reject a whole *layout* whose
widgets bind an unknown source id (`ui_layout_validate`), so a layout that
uses ids 51–59, 74–79 or 83–87 only loads on firmware that knows them. An old app
never sends these sources: on new firmware they stay empty and every widget
bound to them hides.

---

## 4. DeviceToHost Payloads

| Tag | Message | Description |
|---|---|---|
| 1 | `HelloAck` | Handshake response. Returns `protocol_version`, `firmware_version`, `board_profile`, runtime MAC-derived `device_id`, `counter_generation`, current lifetime presses, `owner_id` (field 8) and `running_partition` (field 9). |
| 2 | `ConfigAck` | Acknowledges `SetConfig` with `success` boolean, descriptive status message, and current applied `ConfigPayload`. |
| 3 | `CounterSyncResp` | Acknowledges `CounterSync` with `success` boolean, error message, and confirmed synchronized `CounterState`. |
| 4 | `LayoutAck` | Acknowledges `SetLayout` or `ResetLayout` with `screen` ID, `success` boolean, and status message. |
| 5 | `Status` | Periodic or requested device health: `uptime_ms`, `runtime_state`, lifetime counters, and latency statistics (min, max, avg, samples, buckets). |
| 6 | `LogBatch` | Batch of diagnostic entries from the in-RAM ring buffer: `timestamp_ms`, severity `level`, `event_id`, and optional `arg0`/`arg1`. |
| 9 | `KeyPressBatch` | Key-downs timed on the pad, for the tap rate: see below. |
| 10 | `BoardTestResult` | Carrier and input module measurements, the answer to `run_board_test`: see below. |

### `HelloAck.owner_id` (field 8, §W3-1 / §W3-2)

16 bytes. Empty or all zero means **unclaimed**, which is also what firmware
predating W3-2 sends — the two are deliberately indistinguishable, so an older
pad is treated as unclaimed rather than as belonging to nobody in particular.

The host decides from this and its own install identity:

| `owner_id` | Host action |
|---|---|
| Absent / all zero | Claim it silently with `ClaimOwnership`. The common case, and it is not worth a prompt. |
| This install's | Nothing. Proceed normally. |
| A different install's | **Prompt** (§W3-3). Counter sync is blocked until the user answers. The pad keeps working as a keyboard throughout — that was never in question. |
| Any of the above, but this install has no identity | Claim nothing, prompt about nothing. A daemon with no storage has no identity (§W3-1). |

The prompt offers three answers: take the pad over keeping its counters, take it
over keeping this PC's, or leave it alone. "Leave it alone" suppresses config,
layouts, telemetry and all syncing, and the pad is still a keyboard.

This is an **ownership model, not DRM**. Nothing here is cryptographic and
nothing is enforced; it protects counter integrity from a pad silently changing
hands, and that is all it is for.

### `HelloAck.running_partition` (field 9, §U-3a)

The label of the app partition the running image booted from: `ota_0`, `ota_1`,
or `factory` for a pad still on the pre-OTA single-app table. An **empty string**
means firmware older than the field, which the host reads as unknown rather than
guessing a slot. Surfaced by `osupadctl status` as `Running Slot`.

### `KeyPressBatch` (DeviceToHost field 9) and `HelloAck.key_press_times` (field 11)

The tap rate (PPM, issue #2) is measured from the pad's own clock. Every
accepted key-down is logged with its `esp_timer` time in µs, where the press
is accepted: the switch edge for MX (a press confirmed by the debounce
resample is up to one lockout late), and the same point for any other input
module. Logging is one store into a lock-free ring, inside code the input path
already runs, so it adds nothing to the key-to-HID latency.

The protocol task, never the input path, sends the log as `KeyPressBatch`
messages: `presses` (`key` 1 = K1, 2 = K2, `t_us`), oldest first, at most 32
per message; `now_us`, the pad's clock when the batch was built; and
`dropped`, presses lost to a full log or a full CDC FIFO since the last batch.
Batches go out only while the host's `HostStatus.playing` holds the pad in
PLAYING; otherwise the log is emptied.

The host maps pad time onto its own clock with one offset per connection, the
smallest `received - now_us` seen, so the gaps between presses keep the pad's
µs precision whatever the USB and scheduling delays. tosu's key counters, the
only other source, are stamped when its message arrives, which bunches up
when tosu is busy; they are used only for a pad that sends no batches
(`key_press_times` false, i.e. firmware predating it), and then a peak must
span at least 0.4 s. With a pad that sends batches, only its presses are
saved: tosu's counters also run during a replay, from the replay's presses.
A play whose player is not the logged-in profile (a replay, spectating), or
one where tosu counts 8 presses before the pad sends any (your own replay,
another keyboard), is shown from tosu's counters, with the 0.4 s peak, and is
never saved.

Older hosts skip field 9 as an unknown oneof field; newer hosts read a missing
field 11 as false.

### `BoardTestResult` (DeviceToHost field 10) and `HelloAck.board_test` (field 12)

`run_board_test` (HostToDevice field 16) has the pad measure its carrier and
input module wiring (`firmware/main/diag/board_test.c`). The pad only reports
measurements. The host judges them (`desktop/crates/opad-model/src/board_test.rs`),
so the thresholds and advice can change without a firmware update.

| Field | Meaning |
|---|---|
| `ran`, `message` | `ran` is false when the pad refused: outside IDLE (`"not while a map is played"`), or the key line test did not run |
| `boot_module`, `module` | `InputModule` seen at boot (it sets the key pins and the Hall Effect lockout) and now |
| `id_mv`, `id_loaded_mv` | ID (GPIO8) voltage with the internal pull-down off and on, -1 = not measured |
| `reverse_probe_mv` | ID voltage with GPIO2 pulled up, only when no module answers: a pin 1 ↔ 8 reversed cable powers an MX module's IN2 pull-up from GPIO2, lifting ID to ~1.5 V. -1 = not run |
| `key1_gpio`, `key2_gpio`, `keys_enabled` | The key pins tested, and whether key input is on (off with a Hall Effect module on v1) |
| `tested_gpios`, `high_with_pullup`, `high_with_pulldown` | GPIO bitmasks (bit n = GPIOn). Key lines: low with the pull-up = held or shorted to GND; high with the pull-down = the MX module's 10 kΩ pull-up reaches the pin. Spare lines (GPIO6/4/2, connector pins 6–8, only with a module answering): each must follow its own pull |
| `bridged_gpios` | Spare-line neighbours that follow each other: one is driven low (only after both passed alone) and the other, pulled up, reads low |

The test takes about 30 ms on the protocol task. Key edges are ignored for
about 1 ms while the key pins are on pull-downs. The keypad task runs that part
itself and sends no report meanwhile, then restarts debouncing from the pins.
Nothing changes on the key path outside the test. The pad refuses the test
outside IDLE.

Compatibility: firmware predating the test reads field 16 as an unknown
payload and does not answer; it also leaves `HelloAck.board_test` false, so the
daemon refuses the request with "update the firmware" instead of waiting.
Older hosts never send field 16 and skip field 10.

---

## 5. Version Negotiation Rules

1. When the host opens the USB CDC port, it sends `Hello` with `protocol_version = 1`.
2. The device responds with `HelloAck` containing its own `protocol_version`.
3. If `device.protocol_version != host.protocol_version`:
   - The device is marked as `IncompatibleDevice` in daemon runtime state.
   - All telemetry and configuration synchronization is safely suspended.
   - **USB HID Keyboard functionality continues to operate without interruption**, ensuring player input is never broken by an outdated daemon or firmware.
   - The GUI surfaces an incompatibility warning informing the user to update.

---

## 6. Firmware Update & Bootloader Entry

The device supports two non-contact methods to enter the ESP32-S3 ROM bootloader for firmware flashing:

1. **Protocol Command (`EnterBootloader`)**:
   - `osupadctl flash <firmware.bin>` sends `EnterBootloader` over CDC.
   - The firmware calls `esp_restart()` with ROM download mode flags set, immediately re-enumerating as an Espressif USB JTAG/serial DFU device (`VID: 0x303A, PID: 0x1001`).
2. **1200-Baud Touch (Fallback)**:
   - Setting serial line baud rate to 1200 baud arms download mode; the firmware reboots when DTR drops, i.e. when the host closes the port.
The esptool DTR/RTS pattern (RTS falling while DTR stays high) is deliberately
**not** a trigger: ModemManager and other serial probes produce it when they open
any tty, and each one used to reboot the pad into download mode.

`osupadctl` tries both in that order, setting DTR and RTS explicitly rather
than relying on what the platform does at open — Linux asserts DTR when a tty is
opened and Windows does not, so an implicit sequence means different things on
the two platforms (§W1-3).

### Partition layout (§U-3a)

Since the two-slot table, the **app image is written at `0x20000`** (`ota_0`),
not at `0x10000`. `nvs` stays at `0x9000` at its original `0x6000` size, which
is why the lifetime counters and `owner_id` survive a firmware update.

A host-driven update writes the **app partition only** and never `erase-flash`
(§U-3b); `erase-flash` takes NVS with it and is the documented unbind path, not
an update mechanism (`docs/recovery.md` §7).

