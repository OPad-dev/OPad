---
title: GUI Subsystem Architecture
description: Implementation reference for opad-gui, its iced update loop and polling, daemon start and version matching, single-instance handoff, theme tokens, the Screen Designer preview, and the system tray.
sidebar:
  order: 3
---

`opad-gui` is an `iced` 0.14 app built with `iced::daemon`, so closing the window leaves it in the system tray. It holds no pad state of its own: everything comes from `opad-daemon` over IPC ([/desktop/gui/src/main.rs](https://github.com/OPad-dev/OPad/blob/main/desktop/gui/src/main.rs), [/desktop/gui/src/ipc.rs](https://github.com/OPad-dev/OPad/blob/main/desktop/gui/src/ipc.rs)). What each page does for a user: [GUI Reference](../../reference/gui.md).

## Modules

| Module | Role |
|---|---|
| [`main.rs`](https://github.com/OPad-dev/OPad/blob/main/desktop/gui/src/main.rs) | `App` (`new`, `update`, `view`, `subscription`); `Page`, `Section` and `Target` with the `--page` tokens; the one save model of the pad's settings (`form_config`, `pad_dirty`); notices; daemon start and version matching |
| [`pages.rs`](https://github.com/OPad-dev/OPad/blob/main/desktop/gui/src/pages.rs) | Dashboard, Pad, the Logs tab and the calibration view |
| [`keys.rs`](https://github.com/OPad-dev/OPad/blob/main/desktop/gui/src/keys.rs) | Keys page: bindings, wiring, and HE actuation and Rapid Trigger with a live cut-away of each switch |
| [`screen.rs`](https://github.com/OPad-dev/OPad/blob/main/desktop/gui/src/screen.rs) | Screen page: display, touchscreen swipes, result animations |
| [`settings.rs`](https://github.com/OPad-dev/OPad/blob/main/desktop/gui/src/settings.rs) | Settings page and its tabs: General, Updates, osu! connection, About |
| [`stats.rs`](https://github.com/OPad-dev/OPad/blob/main/desktop/gui/src/stats.rs) | Stats page: key speed, and tap rate in KPM for the attempt and the history period |
| [`troubleshooting.rs`](https://github.com/OPad-dev/OPad/blob/main/desktop/gui/src/troubleshooting.rs), [`he_monitor.rs`](https://github.com/OPad-dev/OPad/blob/main/desktop/gui/src/he_monitor.rs) | Troubleshooting page: key test, board test (graded by `opad_model::board_test`), connection, report, and the HE phantom / double-press monitor that follows `he-debug.log` |
| [`widgets.rs`](https://github.com/OPad-dev/OPad/blob/main/desktop/gui/src/widgets.rs) | What the pages share: cards, sections, the tab bar and the save bar |
| [`designer/`](https://github.com/OPad-dev/OPad/tree/main/desktop/gui/src/designer) | Screen Designer (`mod.rs`, `overlay.rs`) |
| [`chrome.rs`](https://github.com/OPad-dev/OPad/blob/main/desktop/gui/src/chrome.rs) | Custom title bar and resize edges replacing system decorations |
| [`theme.rs`](https://github.com/OPad-dev/OPad/blob/main/desktop/gui/src/theme.rs) | Colours and fonts |
| [`tray.rs`](https://github.com/OPad-dev/OPad/blob/main/desktop/gui/src/tray.rs) | Tray icon and menu |
| [`single_instance.rs`](https://github.com/OPad-dev/OPad/blob/main/desktop/gui/src/single_instance.rs) | One GUI at a time |
| [`platform_linux.rs`](https://github.com/OPad-dev/OPad/blob/main/desktop/gui/src/platform_linux.rs), [`platform_windows.rs`](https://github.com/OPad-dev/OPad/blob/main/desktop/gui/src/platform_windows.rs) | Autostart (systemd user units on Linux, the per-user mechanism on Windows) and other OS glue |

## Update Loop and Polling

`App::subscription` drives all traffic to the daemon:

| Poll | Interval |
|---|---|
| Status (`Message::Poll`) | 2 s while the daemon is offline, 1 s with the window open, 3 s hidden in the tray |
| HE live samples (Keys page) | 20 ms, plus animation frames |
| Calibration progress (the calibration inside Keys) | 100 ms |
| Update status | 60 s, 2 s while an update is in progress |

If the daemon is offline the GUI starts it once. If the daemon answers with a different version (one of the two was updated), whichever is older restarts as the new version, once per run.

## CLI Flags and Single Instance

- `--tray` starts hidden in the tray; `--page <token>` / `-p <token>` opens a page or one of its tabs. The tokens are `Target::TOKENS`; the names pages had before still open where their content went (`device` opens Pad, `monitor` the Logs tab, `diagnostics` Troubleshooting). The full list: [GUI Reference](../../reference/gui.md#pages).
- A second launch asks the running instance to show its window (and open its `--page`), then exits. On Unix the instance binds `gui.sock` next to the IPC socket; of two launches racing, exactly one wins the bind and the other connects. A leftover socket is removed only when connecting to it fails with `ECONNREFUSED`. On Windows a named mutex decides and the request goes over the pipe `\\.\pipe\opad-gui`.

## Theme and Typography (`theme.rs`)

The palette mirrors the pad's ([/firmware/main/ui/core/ui_defaults.c](https://github.com/OPad-dev/OPad/blob/main/firmware/main/ui/core/ui_defaults.c)): `BG = #0E0E16`, `SURFACE = #13131E`, `CARD = #1B1B28`, `CARD_HOVER = #24243A`, `BORDER = #2A2A3C`, `PINK = #FF66AA`, `WHITE = #FFFFFF`, `MUTED = #8C8CA6`, `GREEN = #44DD88`, `RED = #FF5566`, `CYAN = #66CCFF`, `YELLOW = #FFCC33`, plus `PINK_HOVER` and `OFF`. Embeds `Montserrat-Medium.ttf` and `Montserrat-Bold.ttf` (SIL OFL 1.1).

## Screen Designer (`designer/`)

[/desktop/gui/src/designer/mod.rs](https://github.com/OPad-dev/OPad/blob/main/desktop/gui/src/designer/mod.rs) edits the Idle and Playing layouts (up to 32 widgets, validated by `opad-layout`) on a canvas at `SCALE = 2.0` with `HANDLE = 5` pad-pixel resize handles (`overlay.rs`). The picture underneath is rendered by `opad-ui-preview`, the firmware's own `ui/core` code compiled for the host ([crates](crates.md#7-opad-ui-preview)). User guide: [Customizing the Screen Layout](../../guides/customize-screen-layout.md).

## System Tray (`tray.rs`)

A backend-agnostic view model (`IconState`: `Warning`, `Connected`, `Disconnected`; `TrayAction`: `ShowWindow`, `Open(Page)`, `SyncNow`, `StartDaemon`, `Quit`) backed by `ksni` (StatusNotifierItem) on Linux and `tray-icon` on Windows.
