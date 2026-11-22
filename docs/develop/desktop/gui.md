---
title: GUI Subsystem Architecture
description: Implementation reference for opad-gui, iced state architecture, asynchronous IPC event loops, system tray integration, and screen designer preview.
category: reference
source_files:
  - /desktop/gui/src/main.rs
  - /desktop/gui/src/pages.rs
  - /desktop/gui/src/theme.rs
  - /desktop/gui/src/tray.rs
  - /desktop/gui/src/designer/mod.rs
---

The `opad-gui` application is built with the [iced](https://github.com/iced-rs/iced) GUI library, following The Elm Architecture (Model-View-Update). It runs cross-platform on Linux and Windows, featuring an asynchronous Tokio runtime backend to communicate with `opad-daemon` over IPC. <!-- src: /desktop/gui/src/main.rs -->

## Application Architecture & Event Loops (`main.rs`)

The application state is encapsulated in `App`: <!-- src: /desktop/gui/src/main.rs -->

- **`App::update`**: Pure event handler responding to user interactions (`Message`), navigation changes, settings toggles, and async IPC task completions (`Task<Message>`).
- **`App::view`**: Pure UI layout generator producing reactive element trees for the active page.
- **`App::subscription`**: Event subscriptions managing periodic polling ticks (50 Hz during Hall Effect live view, 1 Hz for background daemon heartbeats, and window close/minimize interceptions). <!-- src: /desktop/gui/src/main.rs -->
- **Single-Instance Mutex (`single_instance.rs`)**:
  Ensures only one instance of `opad-gui` runs per logged-in desktop session. If launched again, the second process signals the active instance over a private local pipe/socket to unminimize and focus the existing window, then exits cleanly.

## Design Language & Theme (`theme.rs`)

The GUI visual appearance matches the physical keypad's 2.0-inch LCD display (`ui_defaults.c`): <!-- src: /desktop/gui/src/theme.rs -->
- **Palette**: Near-black void background (`BG = #0E0E16`), dark elevation surfaces (`SURFACE = #13131E`), subtle card borders (`CARD = #181826`), and osu! pink primary accents (`#FF66AA`). <!-- src: /desktop/gui/src/theme.rs#BG -->
- **Typography**: Clean Montserrat font weights across all headers, stats displays, and LCD widgets.

## Screen Designer Integration (`designer/mod.rs`)

The Screen Designer allows users to visually edit 320x240 LCD layouts with pixel precision: <!-- src: /desktop/gui/src/designer/mod.rs -->

- **Firmware Engine Recompilation (`opad-ui-preview`)**:
  Rather than writing a secondary mock renderer that diverges from the ESP32 firmware, the desktop suite compiles the firmware's actual C LVGL drawing routines into `opad-ui-preview`. The designer preview renders an exact pixel copy of what the ST7789 display will show. <!-- src: /desktop/gui/src/designer/mod.rs -->
- **Interactive Canvas Overlay**:
  Renders drag handles (`HANDLE = 5` pad pixels), selection bounding boxes, alignment snap lines, and coordinate labels scaled at 2.0x (`SCALE = 2.0`). <!-- src: /desktop/gui/src/designer/mod.rs#SCALE -->

## System Tray Integration (`tray.rs`)

`opad-gui` minimizes to the system tray rather than terminating when the window close button is pressed: <!-- src: /desktop/gui/src/tray.rs -->
- **Linux**: Uses `ksni` (KDE StatusNotifierItem protocol), integrating into GNOME (via AppIndicator extension), KDE Plasma, and Wayland desktop environments. <!-- src: /desktop/gui/src/tray.rs#TrayHandle -->
- **Windows**: Uses Windows Shell NotifyIcon APIs with custom Win32 message loops.
- **Menu Actions (`TrayAction`)**: Provides quick actions: Show Window, Open Live Monitor, Sync Now, Start Daemon, and Quit. <!-- src: /desktop/gui/src/tray.rs#TrayAction -->
