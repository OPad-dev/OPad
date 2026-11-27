---
title: GUI Reference
description: The opad-gui pages (Dashboard, Stats, Keys, Screen, Designer, Pad, Settings, Troubleshooting), how each one saves, the tray menu, and their labels and ranges.
sidebar:
  order: 2
---

`opad-gui` is an `iced` desktop app with a tray icon. It talks to `opad-daemon` over local IPC (see [IPC Protocol Reference](ipc.md)) and holds no pad connection of its own.
<!-- src: /desktop/gui/src/main.rs#main -->

| Argument | Effect |
|---|---|
| `--tray` | Start hidden in the tray. |
| `--page <name>`, `-p <name>` | Open on a page or one of its tabs (tokens below, case-insensitive). A second launch hands the page to the running instance instead of starting another. |
<!-- src: /desktop/gui/src/main.rs#parse_page_arg, /desktop/gui/src/main.rs#main, /desktop/gui/src/single_instance.rs -->

## Pages

The sidebar has eight pages in three groups, always the same ones. A page that needs a pad says so instead of disappearing.
<!-- src: /desktop/gui/src/main.rs#Page::GROUPS, /desktop/gui/src/widgets.rs#pad_section -->

| Group | Page | `--page` tokens | Content |
|---|---|---|---|
| Play | **Dashboard** | `dashboard` | Total presses, one card per key, osu! profile, the map now selected, and **Key speed**. |
| Play | **Stats** | `stats`, `kpm`, `ppm` | **Key speed** in detail and the tap rate in KPM. |
| Your pad | **Keys** | `keys`, `rapid-trigger`; `calibrate`, `calibration` open the calibration | What each key sends and how it presses. |
| Your pad | **Screen** | `screen`, `display` | Brightness, sleep, touchscreen swipes, result animations. |
| Your pad | **Designer** | `designer` | Screen layout editor. |
| Your pad | **Pad** | `pad`, `device` | Pad details, counters, backup. |
| App | **Settings** | `settings`; tabs `general`, `updates`, `osu` / `tosu`, `about` / `license` / `licenses` | The app on this computer. |
| App | **Troubleshooting** | `troubleshooting`, `diagnostics`; tabs `logs` / `monitor`, `key-test` / `test` / `testing`, `board-test`, `connection`, `report` | Logs, tests and a report to send. |
<!-- src: /desktop/gui/src/main.rs#Target::TOKENS, /desktop/gui/src/main.rs#Page::GROUPS -->

Below the pages, the sidebar shows `Pad connected` or `Pad not connected` (opens **Pad**) and `osu! connected` or `osu! not connected` (opens **Settings**, **osu! connection**).
<!-- src: /desktop/gui/src/main.rs#view -->

## How Pages Save

| Pages | How |
|---|---|
| **Keys**, **Screen** | One bar at the bottom of both pages: `Unsaved changes`, **Revert**, **Save to pad**. It reads `Everything is saved on the pad.` when nothing is pending and `Connect your pad to change its settings.` without a pad. Leaving these two pages with unsaved changes asks **Stay**, **Discard** or **Save to pad**. |
| **Settings** | Each control applies at once; there is no save button. |
| **Designer** | **Apply to pad**, per screen. |
<!-- src: /desktop/gui/src/widgets.rs#save_bar, /desktop/gui/src/main.rs#go, /desktop/gui/src/main.rs#pad_dirty -->

The result of an action shows as a notice in a corner of the window. Up to three show at once; a notice stays 4 s, a failure 8 s, and longer while the pointer is over it. The full text of a failure also goes to **Troubleshooting**, **Logs**.
<!-- src: /desktop/gui/src/main.rs#Tone::stays, /desktop/gui/src/main.rs#MAX_NOTICES, /desktop/gui/src/main.rs#fail -->

A banner stays at the top of the window only while something needs an answer: the OPad service is not running (**Start it**), storage failed, the pad and the app need different versions, a new pad wants the counts of the old one, the pad was set up on another computer (**Use it here**, **Not now**), or an update waits for a restart.
<!-- src: /desktop/gui/src/main.rs#view -->

## Dashboard

- **TOTAL PRESSES** of the game keys, and one card per key (`Key 1 · Z`) with its lifetime count and its count in this map.
- What the pad is doing: `Ready`, `In a map`, `Map just ended` or `Syncing`.
- **OSU! PROFILE** and **NOW PLAYING**.
- **Key speed**: the typical time from the switch to the computer, with **Details** opening **Stats**.
- Without a pad: `No pad found` and `Plug it in with its USB cable.`
<!-- src: /desktop/gui/src/pages.rs#dashboard, /desktop/gui/src/pages.rs#mode_words, /desktop/gui/src/pages.rs#osu_cards -->

## Stats

**Key speed** is the time from the switch closing to the pad handing the press to USB, measured on the pad: **TYPICAL** (median), **99% UNDER**, **99.9% UNDER**, **SLOWEST**, **PRESSES MEASURED**, **WAITED ONE USB POLL**, and **Reset**.
<!-- src: /desktop/gui/src/stats.rs#key_speed_card -->

Tap rate is in **KPM** (keys per minute, not the map's BPM).

- **Tap rate (KPM) · current attempt** (`LIVE`) or **· last attempt**: the map, **AVG KPM**, **PEAK KPM** (fastest 7 presses in a row), **BEST 10 S**, and a table of current, tapping average, peak and presses. Its rows are named by the key that counts them (`Key 1 · Z`, `Key 2 · X`, `Both keys`). A replay is marked `REPLAY · NOT SAVED`.
- **History**: period buttons `7 days`, `30 days`, `90 days`, `All time`; **ATTEMPTS**, **AVG KPM**, **PEAK KPM**, **BEST 10 S**, **TAPPING AVG**, **PRESSES**, and per row tapping average, average peak, best peak and presses.
<!-- src: /desktop/gui/src/stats.rs#view, /desktop/gui/src/stats.rs#channel_names, /desktop/crates/opad-model/src/tap_rate.rs#HISTORY_PERIOD_CHOICES -->

## Keys

| Section | Shown | Content |
|---|---|---|
| **Bindings** | Always | One tile per key. Click a tile, then press the letter (A to Z) it should send; Esc keeps the one it has. On three-key pads Key 3 can also **Send nothing**, and **GAME KEYS** picks one or two of Key 1–3. |
| **Key feel** | Hall Effect pads | The live switch view and the settings below. Says `The keys are not calibrated` with **Calibrate** until they are. |
| **Wiring** | Hand-wired pads (no module), or firmware that does not report the module | **KEY 1 PIN**, **KEY 2 PIN** with **Auto-detect**: the allowed header pins, minus the other key's pin. |
| **Advanced** | Always, folded (**Show** / **Hide**) | Hall Effect pads: the fine settings below. Other pads: **SWITCH BOUNCE FILTER**, `0.5`–`20.0` ms in `0.5` ms steps. |
<!-- src: /desktop/gui/src/keys.rs#view, /desktop/gui/src/keys.rs#bindings, /desktop/gui/src/keys.rs#wiring, /desktop/gui/src/keys.rs#advanced, /desktop/gui/src/main.rs#captured_letter, /desktop/crates/opad-model/src/lib.rs#key_pins_configurable -->

Every Hall Effect setting is a percentage of the key's calibrated magnetic span, in `0.5` % slider steps.
<!-- src: /desktop/gui/src/keys.rs#setting -->

| Control | Shown | Range |
|---|---|---|
| **Set keys separately** | Key feel | One column per key, or one control for all |
| **ACTUATION POINT** | Key feel | From the lowest point the key can press at to `95.0` % |
| **Rapid trigger** | Key feel | Toggle |
| **SENSITIVITY** | Key feel, rapid trigger on | `1.0`–`50.0` % |
| **Continuous** | Key feel, rapid trigger on | Toggle |
| **Recalibrate** | Key feel | Opens the calibration |
| **PRESS SENSITIVITY**, **RELEASE SENSITIVITY** | Advanced, rapid trigger on | `1.0`–`50.0` % |
| **RELEASE POINT** | Advanced | `1.0`–`50.0` % |
| **TOP RESET** | Advanced, Continuous on | `1.0`–`50.0` % |
| **BOTTOM DEAD ZONE** | Advanced | `0.0`–`30.0` %, `0` = off |
| **Allow below the noise floor** | Advanced, firmware that reports its noise floors | Toggle |
<!-- src: /desktop/gui/src/keys.rs#key_controls, /desktop/gui/src/keys.rs#key_advanced, /desktop/gui/src/keys.rs#min_actuation -->

The live switch view polls `GetHeLive` every 20 ms, smooths the drawing with a One Euro filter and snaps it to the real depth when the pad presses; the pad itself decides on raw readings. It shows `Waiting for the pad (paused during a map)` during a map.
<!-- src: /desktop/gui/src/main.rs#subscription, /desktop/gui/src/keys.rs -->

### Calibration

Calibration opens inside **Keys** (**Calibrate**, **Recalibrate**, or `--page calibrate`) and **Back to Keys** leaves it. It measures the keys at rest, then asks for Key 1, Key 2 (and Key 3 on three-key pads) to be pressed all the way down 5 times each. It ends with `Calibration done` or `Calibration did not finish`, a sentence saying what went wrong, and **Try again**. See [Calibrating Hall Effect](../guides/calibrate-hall-effect.md).
<!-- src: /desktop/gui/src/pages.rs#calibration, /desktop/gui/src/main.rs#Target::CALIBRATION -->

## Screen

| Section | Control | Range |
|---|---|---|
| **Display** | **BRIGHTNESS** | `10`–`100` % |
| **Display** | **SLEEP AFTER** | `1`–`60` min (`60`–`3600` s, 60 s steps) |
| **Display** | **SCREEN UPDATES DURING A MAP** | `1`–`30` Hz (default `10`) |
| **Touchscreen** | **SWIPE UP**, **SWIPE DOWN**, **SWIPE LEFT**, **SWIPE RIGHT** | See below |
| **Result animations** | One animation picker and **Play** button per grade (`SS`, `S`, `A`, `B`, `C`, `D`) | |
| **Layout** | **Edit the layout** | Opens **Designer** |
<!-- src: /desktop/gui/src/screen.rs#display, /desktop/gui/src/screen.rs#touchscreen, /desktop/gui/src/screen.rs#result_animations, /desktop/gui/src/screen.rs#view -->

Each swipe picker lists `Nothing`, `Volume up`, `Volume down`, `Previous track`, `Next track`, `Play / pause`, `Mute`, the osu! shortcuts (for example `Random beatmap`, `Mod select`, `Settings`), and `Other key (F13-F24)…`. The free keys offered are F13–F23 on Windows and F14–F19 on Linux. Ranges and wire values: [Device Settings Reference](device-settings.md#touchscreen-swipes-cfg).
<!-- src: /desktop/crates/opad-model/src/lib.rs#SwipeAction, /desktop/crates/opad-model/src/swipe.rs#swipe_choices, /desktop/crates/opad-model/src/swipe.rs#free_keys -->

## Designer

Edits the `Idle` and `Playing` layouts of the 320 × 240 screen, drawn at 2× (`SCALE = 2.0`) by the firmware's own LVGL UI compiled into `opad-ui-preview`.
<!-- src: /desktop/gui/src/designer/mod.rs#SCALE, /firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h#BOARD_LCD_H_RES -->

- **Idle screen** and **Playing screen** buttons. A `•` on a button means that screen has edits the pad does not show yet.
- Up to 32 widgets per screen (`MAX_WIDGETS`), with drag and resize handles (`HANDLE = 5` pad pixels). Arrow keys move the selection (Shift = 10 px); Delete removes it.
- Widget list with **Raise**, **Lower**, **Duplicate** and **Delete**; the top of the list is drawn on top.
- Per widget: kind, data source, font, alignment, position and size, radius, decimals, label, suffix, colours, and **Fill background (text)**, **Border**, **Hide when there is no data**, **Show tap rate (key's live KPM or KPS)**.
- **Apply to pad** sends the open screen; **Apply both** sends every screen that has edits, one after the other.
- **Undo** (or Ctrl+Z) steps back through the open screen's edits, up to 100 of them. One drag or one typed word is one step. It also brings back what **Revert**, **Import**, **Reload** or **Reset to default** replaced.
- **Revert** goes back to what the pad shows. **Reset to default** makes the pad forget the custom layout of the open screen and draw its built-in one again.
- **Import**, **Export** (JSON, `opad-<screen>.json`), **Reload**, **Live data from osu!**, **Show keys pressed**.
<!-- src: /desktop/gui/src/designer/mod.rs#update, /desktop/gui/src/designer/mod.rs#handle, /desktop/gui/src/designer/mod.rs#UNDO_DEPTH, /desktop/gui/src/designer/mod.rs#view, /desktop/crates/opad-layout/src/lib.rs#MAX_WIDGETS, /desktop/crates/opad-layout/src/lib.rs#FLAG_BG_FILL -->

The layout format and data sources are in [Customizing the Screen Layout](../guides/customize-screen-layout.md).

## Pad

| Section | Content |
|---|---|
| **This pad** | **Pad ID**, **Board**, **Firmware** (with **Update to …** opening **Settings**, **Updates** when one is available), what the pad is doing, **Last sync**. **Details** adds **Running slot**, **Counters source** and **Counter generation**. |
| **Counters** | **On this PC** beside **On the pad** for each key (Key 3 on three-key pads), with `The PC and the pad agree` or `The PC and the pad differ`. **Sync now** and **Reset lifetime counters** are disabled without a pad and during a map. **Restore pad from PC** and **Import PC from pad** appear only when the two differ. |
| **Backup** | **Export backup**, **Import backup**, and **Last automatic backup**; disabled during a map. |
<!-- src: /desktop/gui/src/pages.rs#pad -->

## Settings

Tabs, in order: **General**, **Updates**, **osu! connection**, **About**.
<!-- src: /desktop/gui/src/settings.rs#Tab, /desktop/gui/src/settings.rs#view -->

| Tab | Content |
|---|---|
| **General** | **Start OPad at login**: it starts in the tray, without opening the window. |
| **Updates** | One row each for **OPad**, **tosu** and **Pad firmware**, all with the same shape: installed and available version, **What's new**, **Update automatically**, and **Install now** (**Update now** for the firmware, which asks for explicit consent before flashing and says for how long the pad stops typing). Then **Channel** (`stable`: final releases only; `beta`: release candidates too) and **Check now**. |
| **osu! connection** | `Connected` or `Not connected` with **Start**, and **tosu program**: the one **In use**, **Choose another tosu…**, and **Use the one that comes with OPad** while another is set. |
| **About** | OPad's version, licence and source code; what tosu is, its installed version and source code; **Open source licences**. |
<!-- src: /desktop/gui/src/settings.rs#general, /desktop/gui/src/settings.rs#updates, /desktop/gui/src/settings.rs#firmware_row, /desktop/gui/src/settings.rs#osu_connection, /desktop/gui/src/settings.rs#about -->

All three automatic-update switches are on by default.
<!-- src: /desktop/daemon/src/updater.rs#read_flag -->

## Troubleshooting

| Tab | Content |
|---|---|
| **Logs** | The log of the pad, the OPad service, the app and tosu, with **LEVEL** and **SOURCE** filters, **Clear**, **Copy**, **Save log**, and **Newest first** / **Oldest first**. |
| **Key test** | Live key cards; presses under 15 ms counted as chatter (with an optional beep), 15–35 ms as **RAPID FLUTTER**, presses within 10 ms of the other key as **SIMULTANEOUS PRESSES (NKRO)**; the Hall Effect or MX press monitor. |
| **Board test** | **Run the board test** (`RunBoardTest`); a **Set Key 1 = GPIO10, Key 2 = GPIO7** fix when the pins are wrong for the carrier; the Hall Effect V3 magnet test; **Restart pad**. |
| **Connection** | Measured values only: the OPad service, the pad, osu! (tosu), **Pad ID**, **Board**, **Firmware**, and **App to service** with **Measure**. On Linux, a warning when tosu may not be allowed to read osu!'s memory. |
| **Report** | **Save report…** and **Copy**: one JSON file with the app, OS, service, pad, board test and settings details, and a preview of what is in it. |
<!-- src: /desktop/gui/src/troubleshooting.rs#Tab, /desktop/gui/src/pages.rs#logs, /desktop/gui/src/troubleshooting.rs#view_input_tester, /desktop/gui/src/troubleshooting.rs#view_board_test, /desktop/gui/src/troubleshooting.rs#view_restart, /desktop/gui/src/troubleshooting.rs#view_connection, /desktop/gui/src/troubleshooting.rs#view_export_bundle, /desktop/gui/src/troubleshooting.rs#generate_diagnostic_bundle -->

## Tray Menu

A status line (for example `Pad connected · Ready`, `Pad not connected` or `Pad needs an update`), `Total presses: N`, then **Open OPad**, **Keys**, **Troubleshooting**, **Sync pad now** (**Start OPad service** while the service is not running) and **Quit OPad**.
<!-- src: /desktop/gui/src/tray.rs#TrayViewModel, /desktop/gui/src/tray.rs#TrayAction -->

Where the desktop has no system tray, the sidebar says so: closing the window then quits OPad, and the pad keeps working.
<!-- src: /desktop/gui/src/main.rs#view -->
