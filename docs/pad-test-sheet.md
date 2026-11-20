# OPad pad test sheet

The hands-on part of `docs/launch-checklist.md` §3, short enough to print and
tick by hand. One sheet per pad and module. The full criteria and the reasons
are in the checklist and in `docs/testing-checklist.md` (IDs in brackets).

Pad serial: ______________ Module: MX V3 / HE V3 Date: __________

Firmware: __________ App: __________ Host: Windows 10 / 11 / Linux ______

Flash from the **published release**, never a build tree. A box is ticked only
if it was done now, on this pad.

## 1. No software at all

- [ ] Unplug, uninstall nothing, use a PC with no OPad software: Z, X, C type (Notepad or `evtest`).
- [ ] Keys work in the BIOS / boot menu.
- [ ] Through a hub, and through a USB-C to A adapter.
- [ ] Sleep and wake the PC: keys still work, none stuck.
- [ ] Replug 20 times quickly: it comes back every time.

## 2. Keys

- [ ] Each key, 20 clean taps: exactly 20 presses, counter +20 in `opadctl status` (HW-01, 02, 08).
- [ ] Two keys at once, every pair: both register (HW-03).
- [ ] Stream for 60 s: no missed press, no double (HW-04).
- [ ] Hold a key 30 s: only the OS's own repeat, releases cleanly.
- [ ] Map with `opadctl latency`: p99.9 ______ µs, max ______ µs, deferred ______ (MX ≤ 50 µs, HE ≤ 40 µs).
- [ ] Screen asleep, press a key: the key arrives at once (HW-05).
- [ ] HE only: 1 hour idle, keys untouched, screen animating: no press at all.

## 3. Touchscreen

- [ ] Idle screen: swipe up / down / left / right: arrows and actions match (SWP-01).
- [ ] Text editor open: tap and swipe: nothing typed (SWP-02).
- [ ] Volume swipes with osu! closed, in osu!'s menus, and mid-map (SWP-03, 04, 05).
- [ ] Mid-map, swipe fast and slow in every direction: the map never restarts (SWP-06).
- [ ] Mid-map, rest a finger: Quick Retry within ~60 ms (SWP-07).
- [ ] Daemon stopped: every touch is Quick Retry (SWP-08).
- [ ] Custom swipe actions from Settings each do what they say (SWP-10).

## 4. Hall Effect calibration (HE only)

- [ ] New pad, no app: red lock screen with the download link; keys and swipes send nothing (HE-CAL-01, 02).
- [ ] Calibrate on the pad: all three keys; unlocks without a reboot (HE-CAL-03, 04, 08).
- [ ] Calibrate from the app; the app opened by itself (HE-CAL-05).
- [ ] Cancel mid-calibration, and unplug mid-calibration: old calibration kept (HE-CAL-06).
- [ ] Key held at boot: nothing sent until it is released (HE-CAL-09).
- [ ] Key 3 uncalibrated: pad locked (HW-10).
- [ ] Magnet test passes on each key, and only on that key (HW-07).

## 5. Board test and modules

- [ ] `opadctl board-test` passes (HE: `--magnet`), and keys work right after (HW-06 / 07 / 13).
- [ ] Swap MX ↔ HE with USB out, five times: the pad finds the right one each time.
- [ ] Unplug the module cable while running: no crash, no stuck key.

## 6. Screen and storage

- [ ] Every screen shows correctly: idle, playing, results (each grade), calibration, lock.
- [ ] Brightness and sleep settings survive a replug.
- [ ] 500 presses, wait 10 s, pull the cable, plug it back: counters exact (FAIL-03).
- [ ] Export a backup, change something, import it: restored (BAK-01).

## 7. Updates and recovery

- [ ] Update the firmware from the app: counters and calibration unchanged; asks every time; refused mid-map (UPD-06, 07, 08).
- [ ] After the update, on a PC without OPad: still types (UPD-09).
- [ ] A broken image rolls back by itself (FAIL-06).
- [ ] `opadctl recover` and a replug bring a pad back; no button touched (FAIL-07).
- [ ] Unplug mid-update, then recover (UPD-11).

## 8. Long run

- [ ] 2 hours of maps with tosu and the screen on: no freeze, no reset (STR-01).
- [ ] 8 hours of maps with `opadctl monitor` open: note any dropped message ______.
- [ ] 48 hours plugged in idle: answers at once.

Result: PASS / FAIL Notes: ________________________________________________
