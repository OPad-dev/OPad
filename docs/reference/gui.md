---
title: GUI Reference
description: The opad-gui pages, Settings tabs, Keys page, Screen Designer, Diagnostics tabs and Stats page, with their labels and ranges.
sidebar:
  order: 2
---

`opad-gui` is an `iced` desktop app with a tray icon. It talks to `opad-daemon` over local IPC (see [IPC Protocol Reference](ipc.md)) and holds no pad connection of its own.
<!-- src: /desktop/gui/src/main.rs#main -->

| Argument | Effect |
|---|---|
| `--tray` | Start hidden in the tray. |
| `--page <name>`, `-p <name>` | Open on a page (tokens below, case-insensitive). A second launch hands the page to the running instance instead of starting another. |
<!-- src: /desktop/gui/src/main.rs#parse_page_arg, /desktop/gui/src/main.rs#main, /desktop/gui/src/single_instance.rs -->

## Pages

The sidebar lists `Dashboard`, `Stats`, `Designer`, `Settings`, `Device`, `Logs` and `About`. `Keys` is added after `Device` while a pad with an active Hall Effect backend is connected, `Diagnostics` before `About` once it is enabled in **Settings → 🛠 Diagnostics**, and `Calibration` at the end while it is open.
<!-- src: /desktop/gui/src/main.rs#Page::ALL, /desktop/gui/src/main.rs#view -->

| Page | `--page` tokens | Content |
|---|---|---|
| **Dashboard** | `dashboard` | Connection and mode, total presses, osu! profile and the map now selected, and **Key latency** (`p50`, `p99`, `p99.9`, `max`, `deferred`, `presses`, **Reset**). |
| **Stats** | `stats`, `kpm`, `ppm` | Tap rate in KPM for the current or last attempt and the history. |
| **Designer** | `designer` | Screen layout editor. |
| **Settings** | `settings`, `updates` | Five tabs; `updates` opens the **🔄 Updates & Firmware** tab. |
| **Device** | `device` | Pad details, counter reconciliation, actions, backup and restore. |
| **Keys** | `keys`, `rapid-trigger` | Hall Effect actuation and rapid trigger. |
| **Logs** | `logs`, `monitor` | Log stream with **LEVEL:** and **SOURCE:** filters, **Clear**, **Copy** and **Save log**. |
| **Diagnostics** | `diagnostics`, `test`, `testing` | Five test tabs. |
| **Calibration** | `calibrate`, `calibration` | Guided Hall Effect calibration. |
| **About** | `about`, `license`, `licenses` | Version, system and hardware facts, open source licences. |
<!-- src: /desktop/gui/src/main.rs#Page::TOKENS, /desktop/gui/src/main.rs#wants_updates_tab, /desktop/gui/src/pages.rs -->

## Settings

Tabs, in order: **⌨ Keypad**, **🖥 Display & Game**, **🔄 Updates & Firmware**, **ℹ About & tosu**, **🛠 Diagnostics**. The first two end with **Save settings**, which sends the settings to the pad.
<!-- src: /desktop/gui/src/pages.rs#settings -->

### ⌨ Keypad

| Control | Shown | Range / options |
|---|---|---|
| **KEY 1**, **KEY 2** | Always | The key each switch sends |
| **KEY 3**, **GAME KEYS** | Three-key pads | Key 3's key (empty sends nothing); one or two of Key 1–3 as game keys |
| **KEY 1 PIN**, **KEY 2 PIN** with **Auto-Detect** | Hand-wired pads (no module), or firmware that does not report the module | The allowed header pins, minus the other key's pin |
| **DEBOUNCE LOCKOUT** | Always | `0.5`–`20.0` ms in `0.5` ms steps |
| **HALL EFFECT CALIBRATION**: **Recalibrate Keys** | Hall Effect pads | Opens **Calibration** |
| **↑ SWIPE UP**, **↓ SWIPE DOWN**, **← SWIPE LEFT**, **SWIPE RIGHT →** | Always | See below |
<!-- src: /desktop/gui/src/pages.rs#settings, /desktop/crates/opad-model/src/lib.rs#key_pins_configurable -->

Each swipe picker lists `Nothing`, `Volume up`, `Volume down`, `Previous track`, `Next track`, `Play / pause`, `Mute`, the osu! shortcuts (for example `Random beatmap`, `Mod select`, `Settings`), and `Other key (F13-F24)…`. The free keys offered are F13–F23 on Windows and F14–F19 on Linux. Ranges and wire values: [Device Settings Reference](device-settings.md#touchscreen-swipes-cfg).
<!-- src: /desktop/crates/opad-model/src/lib.rs#SwipeAction, /desktop/crates/opad-model/src/swipe.rs#swipe_choices, /desktop/crates/opad-model/src/swipe.rs#free_keys -->

### 🖥 Display & Game

| Section | Control | Range |
|---|---|---|
| Display | **BRIGHTNESS** | `10`–`100` % |
| Display | **SLEEP AFTER** | `1`–`60` min (`60`–`3600` s, 60 s steps) |
| Result animations | One animation picker and **Play** button per grade (`SS`, `S`, `A`, `B`, `C`, `D`) | |
| Advanced | **GAMEPLAY REFRESH RATE** | `1`–`30` Hz (default `10`) |
| Advanced | **Start in tray at login** | Checkbox |
<!-- src: /desktop/gui/src/pages.rs#settings -->

### 🔄 Updates & Firmware

- **SOFTWARE UPDATES**: one row each for OPad and tosu (installed and available version, **What's new**, **Install now**), **Install OPad updates automatically**, **Install tosu updates automatically**, the **Channel** picker (`stable`: final releases only; `beta`: release candidates too), and **Check now**.
- **FIRMWARE UPDATE**: installed firmware and running slot, blockers, when an automatic update will install, release notes, **Install firmware updates automatically**, and **Update now**, which asks for explicit consent before flashing.
<!-- src: /desktop/gui/src/pages.rs#updates, /desktop/gui/src/pages.rs#firmware_updates -->

All three automatic-update switches are on by default.
<!-- src: /desktop/daemon/src/updater.rs#read_flag -->

### ℹ About & tosu

- **TOSU INTEGRATION**: **Binary source:**, **Status:**, **Custom path:**, **Browse external tosu...**, and **Use bundled** while a custom path is set.
- **ABOUT & THIRD-PARTY SOFTWARE**: OPad's licence (GPL-3.0-or-later) and the bundled tosu component.
<!-- src: /desktop/gui/src/pages.rs#tosu_settings, /desktop/gui/src/pages.rs#about_section -->

### 🛠 Diagnostics

**Enable Diagnostics Menu in Navigation** and **Open Diagnostics Menu →**.
<!-- src: /desktop/gui/src/pages.rs#settings -->

## Device

| Section | Content |
|---|---|
| Pad | **Device ID**, **Board**, **Firmware**, **Running slot**, **Mode**, **Counters source**, **Counter generation**, **Last sync** |
| Counter Reconciliation State | **PC Database** versus **PAD Hardware** for generation, Key 1, Key 2 and total, with `✓ SYNCHRONIZED` or `⚠ MISMATCH DETECTED`; on a mismatch **Restore PAD from PC** and **Import PC from PAD** |
| Actions | **Sync now**, **Update firmware** (opens Settings), **Reset lifetime counters** |
| Backup & Restore | **Export backup**, **Import backup**; disabled during a map and its cooldown |
<!-- src: /desktop/gui/src/pages.rs#device -->

## Keys

Shown only while a pad with an active Hall Effect backend is connected. Every setting is a percentage of the key's calibrated magnetic span, in `0.5` % slider steps.
<!-- src: /desktop/gui/src/keys.rs#view, /desktop/gui/src/keys.rs#setting -->

| Control | Shown | Range |
|---|---|---|
| **Set keys separately** | Always | One column per key, or one control for all |
| **Advanced** / **Hide advanced**, **Recalibrate** | Always | |
| **ACTUATION POINT** | Always | From the lowest point the key can press at to `95.0` % |
| **Rapid trigger** | Always | Toggle |
| **SENSITIVITY** | Rapid trigger on, basic view | `1.0`–`50.0` % |
| **Continuous** | Rapid trigger on | Toggle |
| **PRESS SENSITIVITY**, **RELEASE SENSITIVITY** | Advanced, rapid trigger on | `1.0`–`50.0` % |
| **RELEASE POINT** | Advanced | `1.0`–`50.0` % |
| **TOP RESET** | Advanced, Continuous on | `1.0`–`50.0` % |
| **BOTTOM DEAD ZONE** | Advanced | `0.0`–`30.0` %, `0` = off |
| **Allow below the noise floor** | Advanced, firmware that reports its noise floors | Toggle |
| **Revert**, **Save to pad** | Always | |
<!-- src: /desktop/gui/src/keys.rs#key_controls, /desktop/gui/src/keys.rs#min_actuation -->

The live switch view polls `GetHeLive` every 20 ms, smooths the drawing with a One Euro filter and snaps it to the real depth when the pad presses; the pad itself decides on raw readings. It shows `Waiting for the pad (paused during a map)` during a map.
<!-- src: /desktop/gui/src/main.rs#subscription, /desktop/gui/src/keys.rs -->

## Calibration

A guided flow: measure the rest position, then press Key 1, Key 2 (and Key 3 on three-key pads) to the bottom 5 times each. It ends with `✓ Calibration Complete` or `⚠ Calibration Failed` and **Retry Calibration**. See [Calibrating Hall Effect](../guides/calibrate-hall-effect.md).
<!-- src: /desktop/gui/src/pages.rs#calibration -->

## Designer

Edits the `Idle` and `Playing` layouts of the 320 × 240 screen, drawn at 2× (`SCALE = 2.0`) by the firmware's own LVGL UI compiled into `opad-ui-preview`.
<!-- src: /desktop/gui/src/designer/mod.rs#SCALE, /firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h#BOARD_LCD_H_RES -->

- Up to 32 widgets per screen (`MAX_WIDGETS`), with drag and resize handles (`HANDLE = 5` pad pixels). Arrow keys nudge the selection (Shift = 10 px); Delete removes it.
- Widget list with **Raise**, **Lower**, **Duplicate** and **Delete**; the top of the list is drawn on top.
- Per widget: kind, data source, font, alignment, position and size, radius, decimals, label, suffix, colours, and **Fill background (text)**, **Border**, **Hide when there is no data**, **Show tap rate (key's live KPM or KPS)**.
- Toolbar: **Apply to pad**, **Revert**, **Reset to default**, **Export**, **Import** (JSON, `opad-<screen>.json`), **Live data from osu!**, **Show keys pressed**.
<!-- src: /desktop/gui/src/designer/mod.rs, /desktop/crates/opad-layout/src/lib.rs#MAX_WIDGETS, /desktop/crates/opad-layout/src/lib.rs#FLAG_BG_FILL -->

The layout format and data sources are in [Customizing the Screen Layout](../guides/customize-screen-layout.md).

## Diagnostics

| Tab | Content |
|---|---|
| **⌨ Switch Chatter & Input** | Live key cards; presses under 15 ms counted as chatter (with an optional beep), 15–35 ms as **RAPID FLUTTER**, presses within 10 ms of the other key as **SIMULTANEOUS PRESSES (NKRO)**; the Hall Effect or MX press monitor. |
| **🔌 Hardware COM & Protocol** | Daemon ping latency, serial connection and protocol health, latency percentiles. |
| **🖥 Screen & Backlight** | Backlight levels, test patterns, layout preview. |
| **🔧 PCB Test** | **Run PCB Test** (`RunBoardTest`); a **Set Key 1 = GPIO10, Key 2 = GPIO7** fix when the pins are wrong for the carrier; the Hall Effect V3 magnet test; **Restart Pad**. |
| **📦 Export Diagnostic Bundle** | A JSON bundle with app, OS, daemon, pad, board test and config details. |
<!-- src: /desktop/gui/src/diagnostics.rs#DiagnosticsTab, /desktop/gui/src/diagnostics.rs#view_board_test, /desktop/gui/src/diagnostics.rs#view_restart, /desktop/gui/src/diagnostics.rs#generate_diagnostic_bundle -->

## Stats

Tap rate in **KPM** (keys per minute, not the map's BPM).
<!-- src: /desktop/gui/src/stats.rs -->

- **Current attempt** (`LIVE`) or **Last attempt**: the map, **AVG KPM**, **PEAK KPM** (fastest 7 presses in a row), **BEST 10 S**, and a per-channel table (K1, K2, Combined) of current, tapping average, peak and presses. A replay is marked `REPLAY · NOT SAVED`.
- **History**: period buttons `7 days`, `30 days`, `90 days`, `All time`; **ATTEMPTS**, **AVG KPM**, **PEAK KPM**, **BEST 10 S**, **TAPPING AVG**, **PRESSES**, and per channel tapping average, average peak, best peak and presses.
<!-- src: /desktop/gui/src/stats.rs, /desktop/crates/opad-model/src/tap_rate.rs#HISTORY_PERIOD_CHOICES -->
