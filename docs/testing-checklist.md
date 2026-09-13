# osu!pad Hardware & Release Verification Checklist

This checklist documents the manual hardware release verification procedure (§34, §35) required prior to tagged releases.

**Date of Verification:** 2026-09-13  
**Target Hardware:** Waveshare ESP32-S3-Touch-LCD-2.0  
**Firmware Version:** v1.0.0  
**Host Environment:** Linux x86_64, kernel 6.x  
**Tester:** GFerreiroS  

---

## 1. Physical Key Performance & Latency Invariants

| ID | Test Item | Procedure | Acceptance Criteria | Status |
|---|---|---|---|---|
| HW-01 | Key 1 Press & Release | Tap Key 1 cleanly 20 times. | Exact 1:1 keystroke emission in `evtest`, 0 sticky keys, lifetime counter increments by 20. | **PASS** |
| HW-02 | Key 2 Press & Release | Tap Key 2 cleanly 20 times. | Exact 1:1 keystroke emission in `evtest`, 0 sticky keys, lifetime counter increments by 20. | **PASS** |
| HW-03 | Simultaneous Key Press | Press Key 1 + Key 2 concurrently within 1 ms window. | Both keys reported down and up without key ghosting or lockups. | **PASS** |
| HW-04 | Rapid Alternating Stream | Stream alternating K1/K2 at > 20 presses/sec for 60 seconds. | 0 missed presses, 0 chatter/double-taps, p99.9 latency delta < 0.1 ms over baseline. | **PASS** |
| HW-05 | Display Asleep -> Wake on Press | Allow display to sleep (10 min idle), then press Key 1. | **HID-first invariant verified**: Key report sent immediately to host before display wake sequence begins. No input delay. | **PASS** |

---

## 2. Communication & Protocol Robustness

| ID | Test Item | Procedure | Acceptance Criteria | Status |
|---|---|---|---|---|
| COM-01 | CDC Host Absent | Boot pad with host daemon stopped (pure USB HID host). | Pad boots cleanly, keyboard functions normally, no FreeRTOS watchdog triggers or buffer starvation. | **PASS** |
| COM-02 | Malformed Protocol Frame | Inject random garbage bytes and invalid frame headers over `/dev/ttyACM0` using test script. | Firmware drops malformed buffer, records `DIAG_EVENT_FRAME_TOO_LARGE` / `DIAG_EVENT_DECODE_FAILED`, and recovers on next valid envelope without crash. | **PASS** |
| COM-03 | Rapid USB Reconnect | Unplug and replug USB cable 20 times rapidly (1s interval). | Host daemon reconnects cleanly each time; pad re-enumerates as HID keyboard + CDC without hanging. | **PASS** |
| COM-04 | Daemon Kill During Play | Terminate `osupad-daemon` (`kill -9`) in the middle of active gameplay. | Pad continues working as 1000 Hz HID keyboard with zero interruption to active keystrokes. | **PASS** |
| COM-05 | Tosu Kill During Play | Terminate tosu WebSocket server during active gameplay. | Pad safely transitions from PLAYING to COOLDOWN (5s window), then to IDLE; no stuck UI states. | **PASS** |

---

## 3. Storage & Failure Isolation (§P0-1, §P2-12)

| ID | Test Item | Procedure | Acceptance Criteria | Status |
|---|---|---|---|---|
| FAIL-01 | LCD Fail Fallback Build | Compile with `CONFIG_OSUPAD_TEST_FAIL_LCD=y` and flash to pad. | Non-fatal display init failure logged; pad falls back to headless keyboard operation; 0 HID latency impact. | **PASS** |
| FAIL-02 | NVS Fail Fallback Build | Compile with `CONFIG_OSUPAD_TEST_FAIL_NVS=y` and flash to pad. | Non-fatal NVS failure logged; pad falls back to RAM-only counters; keyboard and protocol continue operating. | **PASS** |
| FAIL-03 | Power-Cycle Persistence | Play map to register 500 presses, wait 10s for IDLE state sync, unplug power. Reconnect power. | Lifetime counters match pre-power-cycle count exactly; 0 loss of verified presses. | **PASS** |
| FAIL-04 | SQLite Degraded Mode | Revoke write permissions on SQLite database (`chmod 400 osupad.db`), run daemon. | Daemon starts in degraded mode, surfaces `storage_error` in GUI/IPC, blocks destructive writes, keeps layouts in memory. | **PASS** |

---

## 4. Endurance & Stress Testing

| ID | Test Item | Procedure | Acceptance Criteria | Status |
|---|---|---|---|---|
| STR-01 | Extended Play Session | Execute continuous gameplay session for ≥ 2 hours with live tosu streaming and display active. | 0 crashes, 0 memory leaks, 0 frame drops, p99 latency remains stable (< 60 µs), thermal stable. | **PASS** |
| STR-02 | Zero Storage Writes Invariant | Monitor disk I/O while playing a map and during the 5s cooldown window. | Zero SQLite transactions or disk writes occur until state reaches IDLE. Verified by `test_writes_blocked_guard`. | **PASS** |

---

## 5. Verification Sign-Off

All checklist items verified and passing for v1.0.0 release.
