---
title: Using the Touchscreen and Swipes
description: How to use the touchscreen third button for in-game Quick Retry and configure directional swipe actions for volume control and track skipping.
category: how-to
source_files:
  - /AGENTS.md
  - /firmware/main/input/touch_retry.c
  - /firmware/main/input/gesture.c
  - /desktop/crates/opad-model/src/swipe.rs
  - /desktop/daemon/src/telemetry.rs
---

The 2.0-inch capacitive touchscreen on OPad functions as **the third button** (Product Rule 4). It is context-aware: during active osu! gameplay, a resting finger triggers Quick Retry (`` ` ``) and swipes never trigger an unwanted retry; outside of beatmaps, directional swipes execute media or keyboard actions. <!-- src: /AGENTS.md -->

## In-Game Behavior: Quick Retry

When playing an osu! beatmap: <!-- src: /firmware/main/input/touch_retry.c -->
- **Resting Finger**: Place and hold your finger anywhere on the glass surface. The firmware observes finger movement for 60 ms (`PLAY_RETRY_STILL_US`). If movement remains within a 12-pixel stillness radius (`PLAY_RETRY_STILL_PX`), it asserts the Quick Retry keycode (`` ` `` / `~`). <!-- src: /firmware/main/input/touch_retry.c#PLAY_RETRY_STILL_US -->
- **Hold to Retry**: Like holding `` ` `` on a conventional keyboard, holding your finger down restarts the current beatmap instantly.
- **Swipe Protection**: Quick Retry is never asserted during a swipe. An accidental grazing touch or finger flick will not restart your map.
- **Display Cleanliness**: During active gameplay, no swipe feedback badges or overlays are drawn on the LCD, keeping the display free of visual distractions.

## Out-of-Game Behavior: Directional Swipes

When browsing beatmap selection screens or using desktop applications, touching the screen will not send Quick Retry. Instead, directional swipes run configurable actions: <!-- src: /firmware/main/input/gesture.c -->

| Swipe Direction | Default Action | Behavior Details |
|---|---|---|
| **Swipe Up** | Volume Up | Increases osu! in-game volume when osu! is focused; steps system master volume otherwise. <!-- src: /desktop/crates/opad-model/src/swipe.rs --> |
| **Swipe Down** | Volume Down | Decreases osu! in-game volume when osu! is focused; steps system master volume otherwise. <!-- src: /desktop/crates/opad-model/src/swipe.rs --> |
| **Swipe Left** | Previous Track | Sends standard USB HID consumer Media Previous Track report. <!-- src: /desktop/crates/opad-model/src/swipe.rs --> |
| **Swipe Right** | Next Track | Sends standard USB HID consumer Media Next Track report. <!-- src: /desktop/crates/opad-model/src/swipe.rs --> |

### Swipe Mechanics
- **Minimum Distance**: Swipes must travel at least 24 pixels (`GESTURE_SWIPE_PX = 24`) from the contact point. <!-- src: /firmware/main/input/gesture.c -->
- **Axis Locking**: To avoid false triggers from diagonal finger rolls, vertical movement requires $2 \cdot |dy| \ge 3 \cdot |dx|$, and horizontal movement requires $2 \cdot |dx| \ge 3 \cdot |dy|$. Ambiguous movements wait for further displacement. <!-- src: /firmware/main/input/gesture.c#gesture_feed -->

## Configuring Custom Swipe Actions

You can customize each swipe direction in `opad-gui`: <!-- src: /desktop/crates/opad-model/src/swipe.rs -->
1. Open `opad-gui` and navigate to **Settings > Keypad**.
2. Locate the **Swipe Actions** configuration section.
3. Select an action from the dropdown for each direction:
   - **System Volume / osu! Volume**: Default dynamic volume control.
   - **Media Playback**: Next Track, Previous Track, Play/Pause.
   - **osu! Shortcuts**: Open Chat (`F9`), Toggle Leaderboard (`Tab`), Toggle Overlay (`Ctrl+O`).
   - **Custom Key**: Any standard alphanumeric key or function key (`F1`–`F12`) with optional modifier flags (`Ctrl`, `Shift`, `Alt`).
4. Click **Apply Changes**. The daemon transmits the updated configuration (`SetConfig`) to the pad. <!-- src: /protocol/opad.proto#SetConfig -->

### Configuring via opadctl
```bash
# Configure swipe up to step volume
opadctl config set swipe-up volume-up

# Configure swipe left to trigger custom key binding
opadctl config set swipe-left media-prev
```
