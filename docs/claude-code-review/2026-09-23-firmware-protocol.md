# Code review: `firmware/main/protocol`

- Date: 2026-09-23
- Command: `/code-review high firmware/main/protocol`
- Base: `main` @ `b0e0a37`
- Verification: none of these findings were re-verified after the review; treat each as *plausible* until checked against the code.
- Status legend: `open` / `fixed` / `wontfix` / `invalid`

Findings are ranked most severe first.

---

## 1. Legacy headers accepted after the host has locked to marked framing — `fixed`

**File:** `firmware/main/protocol/frame_parser.c:143`
**Category:** correctness

The parser keeps accepting legacy headers even after the host is known to speak marked framing, so any desync lets a false legacy header (any `xx xx 00 00`, which protobuf payloads produce routinely) capture the parser, undoing the byte-wise-resync guarantee the `AA 55` marker was introduced for.

**Failure scenario:** A marked host streams `DataUpdate` every ~100 ms during a map; the CDC task falls behind (e.g. blocked in `detect_pin`, or the 512-byte TinyUSB RX FIFO overflows) and some bytes are lost. The old marked-only parser would slide to the next `AA 55`. The new parser slides one byte and hits e.g. `09 00 00 00 ...` (tag 2 fixed64 for `DataValue.number = 100.0` encodes as `11 00 00 00 00 00 00 59 40`) → `b[2] == b[3] == 0` and `0x0009` is in `2..8188` → believed as a legacy header; it swallows 13 bytes of a real frame, hands garbage to nanopb, lands mid-payload again and repeats. The 500 ms stale timer (`protocol.c:580`) never fires because bytes keep arriving, so the pad stays desynced for the rest of the map.

**Suggested fix:** The host side already locks via `accept: Option<Framing>` in `opad-protocol`; the firmware should pass `s_host_framing_known ? s_host_framing : any` into `frame_parser_feed` and reject the other framing's headers once locked.

## 2. `s_host_framing` is re-latched on every accepted frame — `fixed`

**File:** `firmware/main/protocol/protocol.c:571`
**Category:** correctness

`s_host_framing` is re-latched on every frame whose payload nanopb accepts, and nanopb accepts almost anything (e.g. `08 01` = sequence 1, no payload), so a single false-header "frame" in the wrong framing flips the pad's TX framing and the locked host discards the pad's replies.

**Failure scenario:** Marked host, parser hits a false legacy header (see finding 1) whose 9-byte "payload" happens to decode (varint-only garbage usually does) → `handle_host_message` takes the default branch, but `s_host_framing` is set to `LEGACY` first. The next real host frame is e.g. `SetLayout`; its `LayoutAck` goes out with a legacy header; the host, locked to Marked after `HelloAck` (`decode_device_message_framed(&mut buf, Some(Marked))`), slides past it byte by byte → host times out waiting for the ack. Conversely with an old legacy host, stray `AA 55 00 00` (zero-length marked payload is accepted, unlike legacy which needs ≥ 2) flips the pad to marked and the old firmware-era host parser reads `AA 55 xx xx` as a > 0x55AA length and drops its whole buffer.

**Suggested fix:** Latch framing once per connection (first valid frame, or `Hello` only) and clear it only in `protocol_reset_rx`.

## 3. `DetectPinRequest.timeout_ms` is unbounded and busy-loops the CDC task — `fixed`

**File:** `firmware/main/protocol/protocol.c:551`
**Category:** correctness

`DetectPinRequest.timeout_ms` is passed to `keypad_detect_pressed_pin` with no upper bound, and that call busy-loops on the CDC/protocol task for the whole timeout, stalling all RX parsing, DTR-drop reset handling and log draining.

**Failure scenario:** Host (or a fuzzed/foreign process on the port) sends `detect_pin` with `timeout_ms = 0xFFFFFFFF` → `(int64_t)timeout_ms * 1000` is ~49 days; `usb_cdc_task_poll` never returns, `tud_cdc_n_read` is never called so the 512-byte RX FIFO fills and every later host frame is lost/truncated (guaranteed desync for finding 1), `s_rx_reset_requested` is never serviced so a port close/reopen does not reset the parser, and no acks are sent. Even the default 10 s blocks all protocol traffic.

**Suggested fix:** Clamp `timeout_ms` (e.g. ≤ 30000) and/or keep servicing RX while scanning.

## 4. Byte-at-a-time resync memmoves the whole buffer: O(n²) — `fixed`

**File:** `firmware/main/protocol/frame_parser.c:140`
**Category:** efficiency

Resync slides one byte at a time with `drop_front(parser, 1)`, which memmoves the entire remaining buffer per byte: O(n²) on the protocol task, whereas the replaced code jumped with `memchr` to the next `0xAA`.

**Failure scenario:** 8 KB of ROM-bootloader chatter or a swallowed oversized junk run sits in `rx_buf`: ~8192 iterations each moving up to 8 KB ≈ 32 MB of memmove on the CDC task in one poll, delaying every ack and log drain and keeping the RX FIFO undrained.

**Suggested fix:** Scan with a head index (or `memchr` for `0xAA` / for `00 00` pairs) and compact once after the `while` loop; the removed `memchr` fast-path for `FRAME_MAGIC_0` was dropped without replacement.

## 5. Legacy-branch resync bytes produce no diag event; stale-frame log is dead — `fixed`

**File:** `firmware/main/protocol/protocol.c:588`
**Category:** diagnostics

Only marked oversized headers raise `DIAG_EVENT_FRAME_TOO_LARGE`; bytes skipped via the new legacy-branch rejection (`resync_bytes`) produce no diag event or log, so the most common resync path after this change is invisible in diagnostics.

**Failure scenario:** The pad resyncs through 3 KB of junk via the legacy branch (`frame_parser.c:148`) → `resync_bytes` grows, nothing is recorded, `osupadctl` logs show nothing while the user reports missed telemetry. Additionally the "Dropping stale partial frame" warning at line 581 can effectively never fire because `usb_cdc_task_poll` calls `protocol_rx_idle()` (line 603) first, which silently resets the same stale frame.

**Suggested fix:** Record `resync_bytes` deltas (e.g. `DIAG_EVENT_FRAME_TOO_LARGE` `arg1` or a new event) and put the stale log in the shared path.

## 6. Stale-partial-frame expiry is implemented twice — `fixed`

**File:** `firmware/main/protocol/protocol.c:603`
**Category:** simplification

The stale-partial-frame expiry is implemented twice (`protocol_feed_cdc_bytes` lines 579-583 and `protocol_rx_idle` lines 603-605) with different side effects, and `protocol_rx_idle`, a query by name, now mutates parser state and is called twice per CDC read from `usb_cdc.c:187/192`.

**Failure scenario:** Maintenance cost: a future change to `RX_STALE_US` handling (e.g. adding the diag event from finding 5) must be made in both places or they diverge; the one with the log is dead in practice.

**Suggested fix:** A single static `expire_stale_partial(now)` helper containing the check, reset, log and diag, called from both entry points; or have `protocol_feed_cdc_bytes` call `protocol_rx_idle()`.

## 7. `layout_from_proto` clamps out-of-range flags to a valid value — `fixed`

**File:** `firmware/main/protocol/protocol.c:362`
**Category:** correctness

`layout_from_proto` clamps out-of-range `flags` to 0 (a valid value) while the comment and every neighbouring field clamp to a value the validator rejects, so a widget with `flags > 255` is silently accepted with its flags stripped instead of rejected.

**Failure scenario:** Host sends `UiWidget.flags = 0x100` (e.g. a future flag bit an older pad does not know) → `w->flags = 0`, `ui_set_layout` succeeds, `LayoutAck` says success, and the layout is persisted to flash without the requested flags; the host never learns the pad rejected part of the layout. Same pattern for `w`/`h` > `INT16_MAX` → 0.

**Suggested fix:** Clamp to `UINT8_MAX` / a rejected sentinel like the other fields, or reject the frame explicitly.

**Resolution:** `flags` is not checked by `ui_layout_validate`, so an out-of-range value now rejects the frame explicitly. The `w`/`h` part does not hold: they clamp to 0, which the validator rejects (`w <= 0 || h <= 0`).

## 8. `s_host_framing` declared `volatile` but only touched by one task — `fixed`

**File:** `firmware/main/protocol/protocol.c:36`
**Category:** simplification

`s_host_framing_known` / `s_host_framing` are declared `volatile` as if shared across tasks, but every writer (`on_frame_received`, `protocol_reset_rx`) and every reader (`send_envelope` via `protocol_send_*`, `protocol_drain_diag_logs`) runs on the single `cdc_proto` task; no `protocol_send_*` has callers outside `protocol.c`.

**Failure scenario:** Misleading: `volatile` suggests cross-core publication without providing ordering, so a future reader on core 0 would assume it is safe when it is not (the paired `framing_known`/`framing` writes at lines 571-572 have no barrier).

**Suggested fix:** Drop `volatile` and add a comment that the protocol state is owned by the CDC task, matching the "protocol task only" note at line 478.
