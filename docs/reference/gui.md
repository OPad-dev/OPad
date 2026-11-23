---
title: GUI Reference
description: Reference guide to opad-gui pages, sidebar navigation, Settings tabs, Hall Effect Keys page, Screen Designer, Diagnostics suite, and Stats page.
sidebar:
  order: 2
---

`opad-gui` is an `iced`-based desktop application with a system tray icon (`--tray` starts hidden in the tray, and `--page <name>` opens a specific page). It communicates with `opad-daemon` over local IPC. <!-- src: /desktop/gui/src/main.rs#parse_page_arg -->

## Navigation Pages

The left sidebar shows `Dashboard`, `Stats`, `Designer`, `Settings`, `Device`, `Logs`, and `About`, plus `Keys` (shown automatically when a Hall Effect pad is connected) and `Diagnostics` (when enabled under **Settings** -> **🛠 Diagnostics**). `Calibration` is opened from **Keys** -> **Recalibrate**, **Settings** -> **⌨ Keypad** -> **Recalibrate Keys**, or `--page calibrate`. <!-- src: /desktop/gui/src/main.rs#Page -->

| Page | `--page` tokens | Purpose |
|---|---|---|
| **Dashboard** | `dashboard` | Connection and mode status, lifetime key counts, osu! profile / live map summary, and on-device key latency (`p50`, `p99`, `p99.9`, `max`, `deferred`, `presses`, **Reset**). <!-- src: /desktop/gui/src/pages.rs#dashboard --> |
| **Keys** (HE only) | `keys`, `rapid-trigger` | Hall Effect actuation, rapid trigger, advanced thresholds, and live cut-away switch visualizer. <!-- src: /desktop/gui/src/keys.rs#view --> |
| **Stats** | `stats`, `kpm`, `ppm` | Current/last attempt and historical tap rate in KPM (keys per minute). <!-- src: /desktop/gui/src/stats.rs --> |
| **Designer** | `designer` | WYSIWYG 320x240 LCD layout editor for `Idle` and `Playing` screens. <!-- src: /desktop/gui/src/designer/mod.rs --> |
| **Settings** | `settings` | Five sub-tabs covering keypad, display, updates/firmware, `tosu`/about, and diagnostics toggle. <!-- src: /desktop/gui/src/pages.rs#settings --> |
| **Device** | `device` | Hardware info (`Device ID`, `Board`, `Firmware`, `Running slot`, `Counter generation`, `Last sync`), **Counter Reconciliation State** table, **Sync now**, **Update firmware**, **Reset lifetime counters**, and **Backup & Restore** (**Export backup** / **Import backup**). <!-- src: /desktop/gui/src/pages.rs#device --> |
| **Logs** | `logs`, `monitor` | Live diagnostic log stream from daemon, pad, program, and `tosu`. <!-- src: /desktop/gui/src/main.rs#Page --> |
| **Diagnostics** | `diagnostics`, `test`, `testing` | Five-tab hardware and protocol test suite (opt-in from Settings). <!-- src: /desktop/gui/src/diagnostics.rs#DiagnosticsTab --> |
| **Calibration** | `calibrate`, `calibration` | Guided 5-press-per-key Hall Effect calibration wizard. <!-- src: /desktop/gui/src/pages.rs#calibration --> |
| **About** | `about`, `license`, `licenses` | Application version, open-source licenses, and third-party notices. <!-- src: /desktop/gui/src/main.rs#Page --> |

## Settings Tabs (`Settings`)

The **Settings** page has five tabs: <!-- src: /desktop/gui/src/pages.rs#settings -->

### `⌨ Keypad`
- **Keys**: **KEY 1** and **KEY 2** (plus **KEY 3** and **GAME KEYS** checkboxes on 3-key V3 pads), **KEY 1 PIN** / **KEY 2 PIN** dropdowns with **Auto-Detect** (shown only for hand-wired builds), **DEBOUNCE LOCKOUT** slider (`0.5 ms`–`20.0 ms`, step `0.5 ms`), and **HALL EFFECT CALIBRATION** (**Recalibrate Keys** button on HE pads). <!-- src: /desktop/gui/src/pages.rs#settings -->
- **Touchscreen swipes**: Directional pickers (`↑ SWIPE UP`, `↓ SWIPE DOWN`, `← SWIPE LEFT`, `SWIPE RIGHT →`) with `Nothing`, `Volume up`, `Volume down`, `Previous track`, `Next track`, `Play / pause`, `Mute`, osu! shortcuts, or `Other key (F13-F24)…`. <!-- src: /desktop/gui/src/pages.rs#settings --> <!-- src: /desktop/crates/opad-model/src/swipe.rs -->
- **Save settings** commits changes to the pad.

### `🖥 Display & Game`
- **Display**: **BRIGHTNESS** slider (`10%`–`100%`) and **SLEEP AFTER** slider (`1 min`–`60 min`, `60..=3600` s in 60 s steps). <!-- src: /desktop/gui/src/pages.rs#settings -->
- **Result animations**: Per-grade (`SS`, `S`, `A`, `B`, `C`, `D`) animation dropdowns and **Play** preview buttons. <!-- src: /desktop/gui/src/pages.rs#settings -->
- **Advanced**: **GAMEPLAY REFRESH RATE** slider (`1–30 Hz`, default `10 Hz`) and **Start in tray at login** checkbox. <!-- src: /desktop/gui/src/pages.rs#settings -->

### `🔄 Updates & Firmware`
- **SOFTWARE UPDATES**: Installed vs available versions for **OPad** and **tosu**, **What's new**, **Install now**, the **Install OPad updates automatically** and **Install tosu updates automatically** switches (both on by default), **Channel** picker (`stable` / `beta`), and **Check now** button. <!-- src: /desktop/gui/src/pages.rs#updates -->
- **FIRMWARE UPDATE**: Installed firmware, running OTA slot (`ota_0` / `ota_1`), release notes, active blockers, when an automatic update will install, the **Install firmware updates automatically** switch, and **Update now** (opens an explicit consent dialog with **Confirm & Flash Pad**). <!-- src: /desktop/gui/src/pages.rs#firmware_updates, /desktop/gui/src/main.rs -->

### `ℹ About & tosu`
- **TOSU INTEGRATION**: Shows **Binary source:**, **Status:** (`Connected (streaming telemetry)` / `Not running / Disconnected`), **Custom path:**, **Browse external tosu...**, and **Use bundled** (when overridden). <!-- src: /desktop/gui/src/pages.rs#tosu_settings -->
- **ABOUT & THIRD-PARTY SOFTWARE**: Version, GPL-3.0-or-later license, and bundled `tosu` LGPL-3.0 attribution. <!-- src: /desktop/gui/src/pages.rs#about_section -->

### `🛠 Diagnostics`
- **Enable Diagnostics Menu in Navigation** checkbox and **Open Diagnostics Menu →** button. <!-- src: /desktop/gui/src/pages.rs#settings -->

## Keys Page (Hall Effect Tuning)

Shown only when a Hall Effect pad is connected. All travel settings are percentages of each key's calibrated magnetic span (`0.5 %` slider steps, never millimeters per D3): <!-- src: /desktop/gui/src/keys.rs#view -->
- Top bar: **Set keys separately** toggle, **Advanced** / **Hide advanced** button, **Recalibrate** button.
- Basic mode: **ACTUATION POINT**, **Rapid trigger** toggle, **SENSITIVITY** (`1.0 %`–`50.0 %`), **Continuous** toggle.
- Advanced mode adds: **PRESS SENSITIVITY**, **RELEASE SENSITIVITY**, **RELEASE POINT**, **TOP RESET** (with Continuous on), **BOTTOM DEAD ZONE** (`0.0 %`–`30.0 %`, `0 = off`), and **Allow below the noise floor**.
- Live cut-away switch canvas: polls `GetHeLive` at ~50 Hz with One Euro display smoothing and edge snapping (pad decides on raw readings); pauses during maps.
- Bottom bar: **Revert** and **Save to pad**.

## Screen Designer (`Designer`)

Visual editor for the `320x240` LCD (`Idle` and `Playing` layouts), rendered at `2.0x` scale (`SCALE = 2.0`) using the firmware's own LVGL UI compiled into `opad-ui-preview`: <!-- src: /desktop/gui/src/designer/mod.rs -->
- Up to `32` widgets (`MAX_WIDGETS`), drag/resize handles (`HANDLE = 5` pad pixels), arrow-key nudging, Z-order (**Raise** / **Lower**), and duplicate/delete.
- Widget properties: kind, data source, font, alignment, geometry (`X`, `Y`, `W`, `H`, radius), decimal places, tap rate unit, label/suffix, colors (foreground, background, accent, screen background), and flags (`FLAG_BG_FILL`, `FLAG_BORDER`, `FLAG_HIDE_WHEN_EMPTY`, `FLAG_KEY_RATE`).
- Live data preview toggle, simulated key press toggle, layout JSON export/import, **Apply**, **Revert**, and reset to built-in default.

## Diagnostics Suite (`Diagnostics`)

Five tabs (`DiagnosticsTab` in `desktop/gui/src/diagnostics.rs`): <!-- src: /desktop/gui/src/diagnostics.rs#DiagnosticsTab -->
1. **`⌨ Switch Chatter & Input`**: Live key press cards, `< 15 ms` definite chatter detection with optional audio beep, `15–35 ms` flutter tracking, simultaneous press count (`<= 10 ms`), and HE/MX debug log monitor.
2. **`🔌 Hardware COM & Protocol`**: Daemon ping latency, serial connection and protocol health, and pad restart control.
3. **`🖥 Screen & Backlight`**: Display test patterns and backlight brightness test controls.
4. **`🔧 PCB Test`**: Runs `RunBoardTest` on the carrier and input module PCBs (module ID, FFC orientation, key lines, shorts, ADS7028), **Use carrier key pins** helper, and live Hall Effect V3 magnet test.
5. **`📦 Export Diagnostic Bundle`**: Exports system info, USB/latency stats, counters, and recent logs for troubleshooting.

## Stats Page (`Stats`)

Displays live and historical tap rate in **KPM** (keys per minute, distinct from map BPM) for Key 1, Key 2, and Combined channels, including current/last attempt average, peak burst (7 consecutive presses), song rate, and history period buttons (`7` days, `30` days, `90` days, and `All time` (`0`)). <!-- src: /desktop/gui/src/stats.rs --> <!-- src: /desktop/crates/opad-model/src/tap_rate.rs#HISTORY_PERIOD_CHOICES -->
