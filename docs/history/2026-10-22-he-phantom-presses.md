# HE phantom key presses: investigation, 2026-10-21/22

**Status: unresolved.** The cause is not found yet. This records what was
measured, what was ruled out (with the evidence), the mistakes made on the
way, the tools built, and where to pick up. Branch `feat/he-night`.

## Symptom

After playing a map in osu! on the owner's HE pad (V1.1 HE module, keys on
GPIO10 / GPIO7), the pad started typing Z and X by itself, also in menus and
after a replug. The daemon then also failed counter syncs ("non-monotonic":
the pad's counters kept changing during the sync).

Settings at the time, unchanged throughout: both keys continuous rapid
trigger, actuation 20.0 % / 19.8 % (key 1 rest+92, key 2 rest+104 counts),
release distance 10 %, top reset 10 %, sensitivity 50 %, brightness 100 %.
Calibration: key 1 rest 2081 full 2539, key 2 rest 2087 full 2611.

## What the phantom presses are

Established from the HE debug firmware's log (below), ~2900 phantoms over
7 boots:

- **One reading jumps** 61 to 379 counts above rest, then drops back. A finger
  moves a key about 10 counts per frame at most, so these are not mechanical.
  `scripts/he_debug_log.py` calls a press a phantom when the reading jumped more
  than 60 counts in the single frame before it.
- **Short:** 370 of 492 measured lasted 1 frame (250 us); the rest 2 to 11
  frames (up to 2.75 ms). Counted over the log up to 01:13 (boots 0 and 1),
  from the frames before each release.
- **Often both keys in the same frame** (149 of the frames in boot 6), always
  upwards. Key 2 jumps more than key 1 in those frames (median +162 vs +121).
- **In bursts** of seconds to minutes, then quiet for minutes. Bursts happen in
  idle, in maps and in cooldown.
- **On a grid:** within a burst the phantoms line up on 10 ms (one burst:
  alignment 0.95) or 5 ms (others: 0.94, 0.65), measured on the pad's own
  clock: 100.0008 Hz over a 28 s burst, i.e. no drift from the pad's crystal.
  Two boots also line up on **154.08 ms** (0.70 in boot 4, 0.60 in boot 6).
  Not followed up yet.
- **The ADC reports itself healthy** during bursts: no late frames, no stray
  (foreign) or railed conversions, no key faults or stalls, ~4000 frames/s.
- **Temperature steady**, 37 to 40 C (chip sensor).

Per boot (`scripts/he_debug_log.py summary` on the saved log):

| Boot | Started | Real presses | Phantoms (idle / playing / cooldown) |
| --- | --- | --- | --- |
| 1 | 01:01:15 | 1758 | 620 (424 / 149 / 47) |
| 2 | 01:15:57 | 342 | 858 (741 / 78 / 39) |
| 3 | 01:22:16 | 38 | 339 (339 / 0 / 0) |
| 4 | 01:30:07 | 29 | 242 (242 / 0 / 0) |
| 5 | 01:34:42 | 338 | 207 (13 / 181 / 13) |
| 6 | 01:43:33 | 1445 | 630 (426 / 111 / 93) |

## Ruled out

| Hypothesis | Test | Result |
| --- | --- | --- |
| A configuration problem: actuation too low | Settings sweep, and spike sizes | Spikes reach 379 counts (~72 % of travel); no press point short of bottoming out avoids them. A sweep at 25 to 45 % found 0 phantoms, but so did 20 % right after, so the sweep only caught a quiet period. |
| The result animation / screen redraws | Animations played while counting, debug firmware with animation state | With the screen working, a 2.6 s animation showed no spikes. Earlier "phantoms during animations" were coincidence (see Mistakes). |
| LCD clock 80 MHz | Same tests at 40 MHz | Fewer phantoms in one session, then 0 at 80 MHz later: not repeatable. The owner keeps 80 MHz. |
| The GUI / Keys tab per-frame redraw | 15 s and 60 s windows, GUI closed / Dashboard / Keys tab | GUI closed: 197 phantoms in 60 s; Dashboard: 0 in 60 s, and the owner saw them on the Dashboard too. Not the GUI. |
| The live view (`he_live_view`) stream | Live readings requested with the GUI closed, 15 s | 0, and phantoms happen without it. |
| Front-panel USB | Pad moved to a rear motherboard port | 45 phantoms in the first 40 s. Not the port. |
| Touchscreen / touch controller I2C | Debug log of finger state and controller reads per second | Bursts with no finger and 1 controller read per second (the idle check). |
| Backlight PWM (5 kHz on GPIO1, an ADC1 pin) | 100 % made solid-on (duty 256/256, no edges) | 95 phantoms within ~6 minutes. Not the backlight. The solid-on change is kept (correct anyway). |
| Module monitor | Code read (`firmware/main/input/module_monitor.c`) | With HE running it does no ADC work. |
| Heat | Chip temperature per second | Steady 37 to 40 C. |

## Open leads and next steps

1. **Measure the 3V3 / ADC reference together with the keys.** Upward,
   simultaneous jumps on both keys fit a supply or ADC reference disturbance
   (the DRV5055 output is ratiometric to its supply). Add the module ID channel
   (a fixed divider, ~1.06 V, ADC1_CH7, GPIO8) to the debug build's ADC
   pattern, as `firmware/main/diag/hall_bench.c` did: if ID jumps in the same
   frames, it is electrical and common-mode.
2. **Find the 154.08 ms and 5 / 10 ms sources.** Both are on the pad's clock.
   Candidates not yet checked: TinyUSB / CDC activity, the LVGL task, flash or
   PSRAM access, the HE task's own 2 ms health check.
3. **Hardware:** RC filtering on each sensor output and decoupling at each
   sensor's supply pin on the HE board, once the source is known.
4. **Firmware mitigation, owner's call:** ignoring single-frame jumps would
   remove ~75 % (1-frame phantoms) at 0.25 ms latency and goes against D2 (no
   filtering on the key path); catching all needs ~3 ms.

## Mistakes made during the investigation

Recorded so they are not repeated:

- Concluded twice from too little data: "the result animation causes it"
  (the screen was not even working in one of those runs) and "the Keys tab
  causes it" (15 s windows on a phenomenon that comes in bursts of minutes).
  Use windows of minutes and compare like with like.
- The first debug build stamped presses with the log task's 20 ms tick, which
  made every gap look like a multiple of 20 ms. Presses now carry their own
  frame time.
- The first debug build started its task before the display and took the DMA
  RAM the LCD draw buffers need: the screen failed to start (`LCD
  initialization failed`), so "animation" tests in that boot played nothing.
  Fixed: the debug task starts after the display.

## Tools built

- **HE debug firmware** (`CONFIG_OSUPAD_HE_DEBUG`, `firmware/main/input/he_debug.c`,
  `firmware/sdkconfig.hedebug`, build dir `build-hed`). Logs every press with the
  12 frames before it, per-second ranges and ADC health, state, animation,
  touch (finger and controller reads), chip temperature, thresholds. Build and
  line formats: `docs/specs/v2-hall-effect-decisions.md`, "HE debug firmware".
- **Daemon log file:** `he.*` lines appended to `~/.local/share/opad/he-debug.log`
  (`desktop/daemon/src/he_debug_log.rs`, rotated at 50 MB).
- **Analysis:** `scripts/he_debug_log.py summary|table|rhythm|spikes`.

## Logs

- This investigation's full log: `he-debug.log.gz` in
  `docs/history/2026-10-22-he-phantom-presses/` (36 599 lines, 7 boots,
  00:56 to 01:50 on 2026-10-22), also as
  `~/.local/share/opad/he-debug-2026-10-22.log` on the owner's PC.
- Earlier evidence from before the debug firmware (daemon journal):
  counters rising while idle and "Device rejected CounterSync: non-monotonic"
  at 00:24 to 00:26 on 2026-10-22 (`journalctl --user -u opad-daemon`).

## State left behind

- The pad runs the HE debug firmware (`1.0.0-rc+hedebug`) with 100 %
  backlight solid-on. Settings as above. To go back to the normal HE build:
  `idf.py -B build-he -D SDKCONFIG=build-he/sdkconfig build`, then
  `opadctl flash firmware/build-he/opad-firmware.bin`.
- The installed app is this branch's build (Keys tab, live view, debug log file).
