---
title: GUI Subsystem Architecture
description: Implementation reference for opad-gui, iced Elm architecture, single-instance handoff, theme tokens, Screen Designer preview, and system tray.
---

`opad-gui` is built with `iced` using `iced::daemon` (so closing the window minimizes to the system tray) and communicates asynchronously with `opad-daemon` over local IPC ([/desktop/gui/src/main.rs](/desktop/gui/src/main.rs)).

## Application Architecture (`main.rs`)

- **Model-View-Update**: `App::new`, `App::update` (returning `Task<Message>`), `App::view`, and `App::subscription` (background status polling, ~50 Hz `GetHeLive` polling on the **Keys** page, animation frames, and close-to-tray handling).
- **CLI flags**: `--tray` starts hidden in the system tray; `--page <token>` / `-p <token>` opens a specific page (e.g., `dashboard`, `stats`, `designer`, `settings`, `device`, `logs`, `diagnostics`, `about`, `calibrate`, `keys`).
- **Single-instance handoff** ([/desktop/gui/src/single_instance.rs](/desktop/gui/src/single_instance.rs)): A second launch forwards its `--page` token over a private local socket/pipe to focus and navigate the running instance, then exits.

## Theme & Typography (`theme.rs`)

[/desktop/gui/src/theme.rs](/desktop/gui/src/theme.rs) mirrors the pad display palette (`firmware/main/ui/core/ui_defaults.c`):
- `BG = #0E0E16`, `SURFACE = #13131E`, `CARD = #1B1B28`, `CARD_HOVER = #24243A`, `BORDER = #2A2A3C`, `PINK = #FF66AA`, `WHITE = #FFFFFF`, `MUTED = #8C8CA6`, `GREEN = #44DD88`, `RED = #FF5566`, `CYAN = #66CCFF`, `YELLOW = #FFCC33`.
- Embeds `Montserrat-Medium.ttf` and `Montserrat-Bold.ttf` (`SIL OFL 1.1`).

## Screen Designer (`designer/mod.rs`)

[/desktop/gui/src/designer/mod.rs](/desktop/gui/src/designer/mod.rs) edits `Idle` and `Playing` layouts (`MAX_WIDGETS = 32`) on a `2.0x` preview canvas (`SCALE = 2.0`, `HANDLE = 5` pad pixels) rendered by [/desktop/crates/opad-ui-preview/src/lib.rs](/desktop/crates/opad-ui-preview/src/lib.rs), which compiles the firmware's actual C LVGL rendering code for pixel-exact desktop previews.

## System Tray (`tray.rs`)

[/desktop/gui/src/tray.rs](/desktop/gui/src/tray.rs) implements a backend-agnostic tray view model (`IconState`: `Warning` / `Connected` / `Disconnected`) backed by `ksni` (StatusNotifierItem) on Linux and `tray-icon` on Windows, exposing `TrayAction` items: `ShowWindow`, `OpenMonitor`, `SyncNow`, `StartDaemon`, and `Quit`.
