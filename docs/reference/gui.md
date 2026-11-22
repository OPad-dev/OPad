---
title: GUI Reference
description: Reference guide to the OPad graphical desktop application (opad-gui), its pages, interactive designer, diagnostics, and Hall Effect tuning interface.
---

The OPad graphical user interface (`opad-gui`) is an iced-based desktop application providing complete device status monitoring, settings configuration, pixel-exact LCD screen designing, interactive switch diagnostics, and Hall Effect magnetic actuation tuning. It connects to the local `opad-daemon` over IPC. <!-- src: /desktop/gui/src/main.rs#parse_page_arg -->

## Navigation Pages

The GUI organizes functionality into dedicated primary pages selectable via the sidebar: <!-- src: /desktop/gui/src/main.rs#Page -->

- **Dashboard**: Live device overview, connection state, active gameplay status, lifetime keypress counters, and current firmware details. <!-- src: /desktop/gui/src/main.rs#Page::Dashboard -->
- **Keys**: Hall Effect actuation, reset sensitivity, deadzone configuration, and rapid trigger tuning with real-time switch cut-away visualization. Displayed dynamically when an HE-capable pad is connected. <!-- src: /desktop/gui/src/keys.rs --> <!-- src: /desktop/gui/src/main.rs#Page::Keys -->
- **Stats**: Tap rate analytics and KPM (keys per minute) histograms across playing attempts and historical periods. <!-- src: /desktop/gui/src/stats.rs --> <!-- src: /desktop/gui/src/main.rs#Page::Stats -->
- **Designer**: WYSIWYG visual layout editor for the pad's 320x240 LCD display. <!-- src: /desktop/gui/src/designer/mod.rs --> <!-- src: /desktop/gui/src/main.rs#Page::Designer -->
- **Settings**: Configuration panels for Keypad, Display, Updates, About tosu, and general daemon parameters. <!-- src: /desktop/gui/src/pages.rs#SettingsTab --> <!-- src: /desktop/gui/src/main.rs#Page::Settings -->
- **Device**: Physical pad hardware settings, GPIO pin assignments, touchscreen gesture bindings, and ownership controls. <!-- src: /desktop/gui/src/main.rs#Page::Device -->
- **Diagnostics**: Suite of hardware and communication tests (switch chatter tester, COM protocol latency benchmark, display test patterns, carrier PCB self-tests, and one-click diagnostic bundle export). <!-- src: /desktop/gui/src/diagnostics.rs --> <!-- src: /desktop/gui/src/main.rs#Page::Diagnostics -->
- **Logs**: Real-time log event viewer streaming diagnostic events from both `opad-daemon` and the pad ring buffer. <!-- src: /desktop/gui/src/main.rs#Page::Logs -->
- **Calibration**: Guided interactive calibration wizard for Hall Effect magnetic travel range extremes. <!-- src: /desktop/gui/src/main.rs#Page::Calibration -->
- **About**: Desktop application version, license notices, and third-party attribution details. <!-- src: /desktop/gui/src/main.rs#Page::About -->

## Settings Tabs

The Settings page organizes device and desktop behavior into five sub-tabs: <!-- src: /desktop/gui/src/pages.rs#SettingsTab -->

### Keypad Tab
- **Debounce Algorithm**: Switch between eager debounce (low-latency transmission with lockout period) and defer debounce (filtering noisy contact closures). <!-- src: /desktop/gui/src/pages.rs#SettingsTab::Keypad -->
- **Debounce Window**: Sliders to tune debounce duration from 1 ms to 20 ms.
- **Key Mappings**: Select HID keycodes sent to the host for Key 1, Key 2, and Touch actions.

### Display Tab
- **Screen Timeout**: Configure display sleep interval (seconds) before entering standby during inactivity. <!-- src: /desktop/gui/src/pages.rs#SettingsTab::Display -->
- **Backlight Brightness**: Active screen brightness level (0% to 100%).
- **Result Animations**: Dropdown selection of celebration splash animations (`AnimChoice`) triggered at the end of beatmaps based on grade. <!-- src: /desktop/gui/src/pages.rs#AnimChoice -->

### Updates Tab
- **Update Channel**: Toggle between `stable` and `beta` release streams. <!-- src: /desktop/gui/src/pages.rs#SettingsTab::Updates -->
- **Automatic Checking**: Enable or disable periodic background update checks.
- **Component Status**: Live version numbers and upgrade buttons for desktop suite, tosu, and firmware binaries.

### About tosu Tab
- **Integration Status**: Connection indicator to the supervised `tosu` instance. <!-- src: /desktop/gui/src/pages.rs#SettingsTab::AboutTosu -->
- **tosu Process State**: Supervisor state, memory scanning health, and song metadata retrieval telemetry.

### Diagnostics Tab
- Direct shortcut into hardware self-test routines and diagnostic bundle generation. <!-- src: /desktop/gui/src/pages.rs#SettingsTab::Diagnostics -->

## Keys Tab (Hall Effect Tuning)

For pads with Hall Effect input modules, the Keys tab provides deep switch travel customization: <!-- src: /desktop/gui/src/keys.rs -->

- **Unit of Measurement**: In accordance with hardware decision record D3, all travel distances are expressed as a percentage of the calibrated magnetic span (in 0.1% increments), never fixed millimetres. <!-- src: /desktop/gui/src/keys.rs -->
- **Actuation Point**: The downward travel percentage required to register a keypress. <!-- src: /desktop/gui/src/keys.rs -->
- **Rapid Trigger (RT)**:
  - **Press Sensitivity**: Distance key must travel downward to trigger an immediate repress while already held down.
  - **Release Sensitivity**: Distance key must travel upward to register an immediate release.
  - **Deadzone**: Filtering band at resting point and bottom-out travel to eliminate noise and phantom presses.
- **Live Switch Cut-Away Canvas**: Real-time graphical rendering of the physical stem position, active actuation thresholds, and instantaneous magnetic flux readings polled from `HeLiveSample`. <!-- src: /desktop/gui/src/keys.rs#HeLiveSample -->

## Screen Designer

The Screen Designer (`desktop/gui/src/designer/mod.rs`) allows users to visually arrange widgets on the pad's 320x240 LCD display: <!-- src: /desktop/gui/src/designer/mod.rs -->

- **WYSIWYG Pixel Preview**: The canvas renders widgets using the firmware's actual display engine (`opad-ui-preview`), scaled by 2.0x for desktop editing. <!-- src: /desktop/gui/src/designer/mod.rs#SCALE -->
- **Screens Supported**: Independent layout profiles for `Idle` (menu/desktop) and `Playing` (mid-beatmap). <!-- src: /desktop/gui/src/designer/mod.rs -->
- **Widget Placement**: Up to 32 widgets per screen (`MAX_WIDGETS`). Handles allow dragging and resizing (`HANDLE = 5` pad pixels). <!-- src: /desktop/gui/src/designer/mod.rs#HANDLE -->
- **Widget Properties**:
  - **Source Binding**: Data sources from tosu (UR, combo, accuracy, PPM, song progress, artist/title). <!-- src: /desktop/gui/src/designer/mod.rs -->
  - **Typography**: Font family, size, decimal precision, label prefixes, and units.
  - **Styling**: Alignment (left, center, right), borders, background fill, and conditional hiding when data is absent (`FLAG_HIDE_WHEN_EMPTY`). <!-- src: /desktop/gui/src/designer/mod.rs -->

## Diagnostics Suite

The Diagnostics page provides hardware verification tools: <!-- src: /desktop/gui/src/diagnostics.rs -->

1. **Switch Chatter & Input Tester**: High-resolution latency and debounce tester detecting switch bounce closures under 15 ms, audio feedback beeps, and rollover tests. <!-- src: /desktop/gui/src/diagnostics.rs -->
2. **Hardware COM & Protocol Test**: Measures round-trip ping latency percentiles and validates packet parser resilience against dropped bytes. <!-- src: /desktop/gui/src/diagnostics.rs -->
3. **Display Self-Test**: Tests LCD brightness ramps, RGB color gradients, and screen test patterns. <!-- src: /desktop/gui/src/diagnostics.rs -->
4. **Board Self-Test**: Executes on-device electrical tests verifying I2C buses, CST816S touch controller communication, ST7789 display communication, and Hall Effect ADC sampling. <!-- src: /desktop/gui/src/diagnostics.rs -->
5. **Diagnostic Bundle Export**: Generates a zipped report containing system hardware details, daemon logs, pad ring buffer logs, USB statistics, and calibration status for troubleshooting. <!-- src: /desktop/gui/src/diagnostics.rs -->

## Tap Stats & KPM Analytics

The Stats page displays player tap performance metrics collected during gameplay attempts: <!-- src: /desktop/gui/src/stats.rs -->

- **Keys Per Minute (KPM)**: Measures player tapping frequency, distinct from beatmap musical BPM. <!-- src: /desktop/gui/src/stats.rs -->
- **Attempt Stats**: Real-time tap frequency graphs, peak bursts, and stamina curves for the current or most recent song attempt (`AttemptStats`). <!-- src: /desktop/gui/src/stats.rs#AttemptStats -->
- **History Aggregation**: Historical tap rate histograms aggregated over configurable retention windows (7, 14, 30, or 90 days). <!-- src: /desktop/gui/src/stats.rs#TapHistory -->
