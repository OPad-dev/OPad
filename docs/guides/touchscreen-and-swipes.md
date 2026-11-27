---
title: Using the Touchscreen and Swipes
description: The touchscreen is OPad's fourth button, after its three keys. A resting finger is Quick Retry during a map, and swipes run actions you choose, such as volume and media keys.
sidebar:
  order: 2
---

The touchscreen is OPad's fourth button, after its three keys. During an osu! map, a finger resting on it holds Quick Retry (`` ` ``) for as long as it stays down. Swipes run actions you choose, by default volume and track controls. <!-- src: /AGENTS.md#1. Product rules --> <!-- src: /firmware/main/input/touch_retry.c#touch_retry_task -->

To tell a map from a menu, the pad needs the desktop app with tosu connected. Without them, every touch is Quick Retry and there are no swipes. <!-- src: /firmware/main/input/touch_retry.c#touch_mode_now --> <!-- src: /firmware/main/input/touch_retry.c#touch_retry_set_host_status -->

## How a touch behaves

| Situation | A finger that rests | A swipe |
|---|---|---|
| **No app, or tosu not connected** | Quick Retry, at once, held until you lift | Nothing: still Quick Retry |
| **During a map** | Quick Retry, held until you lift | Its volume or media action. Never Quick Retry, never a keyboard key |
| **Outside a map** | Nothing | Its action |
<!-- src: /firmware/main/input/touch_retry.c#touch_mode_now --> <!-- src: /firmware/main/input/touch_retry.c#touch_retry_task --> <!-- src: /firmware/main/input/touch_retry.c#run_swipe_action -->

### During a map

- **Quick Retry waits for your finger to settle.** The pad presses Quick Retry once your finger has stayed within 12 pixels for 60 ms, or has been down for 250 ms without becoming a swipe. A tap shorter than that still sends one Quick Retry press and release. osu!'s retry is a hold anyway, so the wait costs nothing noticeable. <!-- src: /firmware/main/input/touch_retry.c#PLAY_RETRY_STILL_US --> <!-- src: /firmware/main/input/touch_retry.c#PLAY_RETRY_STILL_PX --> <!-- src: /firmware/main/input/touch_retry.c#PLAY_SWIPE_WINDOW_US --> <!-- src: /firmware/main/input/touch_retry.c#tap_retry -->
- **A swipe never restarts the map.** As soon as a touch becomes a swipe, it can no longer send Quick Retry. <!-- src: /firmware/main/input/touch_retry.c#touch_retry_task -->
- **Only volume and media actions run.** osu! shortcuts and other keyboard keys are skipped, volume swipes send a plain mouse wheel without Alt, and nothing is drawn on the pad's screen. <!-- src: /firmware/main/input/touch_retry.c#step_volume --> <!-- src: /firmware/main/input/touch_retry.c#handle_swipe -->

### What counts as a swipe

A swipe moves at least 30 pixels, mostly along one axis: one direction has to lead the other by at least 1.5 to 1. A vertical swipe keeps stepping every further 30 pixels while you drag, so one long drag changes the volume several steps. Every other action fires once per touch. <!-- src: /firmware/main/input/gesture.h#GESTURE_SWIPE_PX --> <!-- src: /firmware/main/input/gesture.c#gesture_feed --> <!-- src: /firmware/main/input/touch_retry.c#handle_swipe -->

## Default swipes

| Swipe | Default action | What it does |
|---|---|---|
| Up | `Volume up` | osu!'s volume when osu! is active, otherwise the system volume. Repeats along a long drag. |
| Down | `Volume down` | The same, downwards. |
| Left | `Previous track` | The media key Previous Track. |
| Right | `Next track` | The media key Next Track. |
<!-- src: /desktop/crates/opad-model/src/lib.rs#DEFAULT_SWIPE_UP --> <!-- src: /firmware/main/input/touch_retry.c#s_swipe_actions --> <!-- src: /firmware/main/input/touch_retry.c#run_swipe_action -->

"osu! is active" means: <!-- src: /desktop/daemon/src/osu_window.rs -->

- **Windows:** osu! (stable or lazer) is the window in front.
- **Linux:** osu!lazer is running. Linux has no portable way to ask which window has focus, so a volume swipe sends osu!'s volume control (Alt+wheel) to whatever window is in front while lazer runs.

## Choose your swipe actions

1. Connect the pad and open the OPad app.
2. Open **Screen** and find **Touchscreen**.
3. Each direction has a list: **SWIPE UP**, **SWIPE DOWN**, **SWIPE LEFT** and **SWIPE RIGHT**. Pick an action: <!-- src: /desktop/gui/src/screen.rs#touchscreen --> <!-- src: /desktop/crates/opad-model/src/swipe.rs#swipe_choices -->
   - **Media and volume:** `Nothing`, `Volume up`, `Volume down`, `Previous track`, `Next track`, `Play / pause`, `Mute`.
   - **An osu! shortcut**, such as `osu!: Random beatmap (F2)`, `osu!: Mod select (F1)`, `osu!: Chat (F8)`, `osu!: Settings (Ctrl+O)` or `osu!: Screenshot (F12)`. Shortcuts only in osu!lazer start with `osu!lazer:`; on Windows the list also has osu! stable's `Boss key (hide osu!) (Insert)` and `Reload skin (Ctrl+Alt+Shift+S)`. <!-- src: /desktop/crates/opad-model/src/swipe.rs#OSU_SHORTCUTS -->
   - **`Other key (F13-F24)…`:** a second list, **Choose a key**, offers function keys that nothing else uses, for binding in OBS, Discord or osu! itself. On Linux that is F14–F19 (the desktop gives F13 and F20–F23 their own meanings); on Windows F13–F23. F24 is never offered, because the pad uses it internally after an Alt+wheel volume step. <!-- src: /desktop/crates/opad-model/src/swipe.rs#free_keys -->
4. Click **Save to pad** in the bar at the bottom. The pad stores the actions, so they work the same on any PC with OPad. <!-- src: /desktop/gui/src/widgets.rs#save_bar --> <!-- src: /firmware/main/config/device_config.c -->

> [!NOTE]
> An uncalibrated Hall Effect pad is locked to its calibration screen. There, a touch only presses that screen's button: swipes are off and no key is sent. See [Calibrating Hall Effect switches](calibrate-hall-effect.md). <!-- src: /firmware/main/input/touch_retry.c#touch_retry_task --> <!-- src: /docs/specs/v2-hall-effect-decisions.md -->

## Check that it worked

1. Run `opadctl status`. The `Swipes:` line shows the action for each direction (`up`, `down`, `left`, `right`). <!-- src: /desktop/cli/src/main.rs#run -->
2. Outside a map, swipe on the pad. It shows a short badge in the direction of your swipe, and the action happens on your PC. <!-- src: /firmware/main/input/touch_retry.c#handle_swipe -->

## If it did not work

- **Every touch is Quick Retry and swipes do nothing:** the pad is not hearing from tosu. Check that `opadctl status` shows `tosu (osu!): Active`; see [Using OPad with tosu](use-with-tosu.md). <!-- src: /firmware/main/input/touch_retry.c#touch_mode_now -->
- **Volume swipes change the system volume instead of osu!'s:** on Windows, bring osu! to the front; on Linux, start osu!lazer. <!-- src: /desktop/daemon/src/osu_window.rs -->
- **An osu! shortcut swipe does nothing in a map:** that is by design. Keyboard keys are never sent during a map. <!-- src: /firmware/main/input/touch_retry.c#run_swipe_action -->
