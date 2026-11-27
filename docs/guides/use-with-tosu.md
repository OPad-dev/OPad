---
title: Using OPad with tosu
description: How OPad uses tosu for live osu! data on the pad's screen, what to set up on Linux, how to use your own tosu, and how to check that it works.
sidebar:
  order: 6
---

[tosu](https://github.com/tosuapp/tosu) reads osu!'s memory and serves the current map and play over a local WebSocket. OPad bundles tosu v4.26.2 (LGPL-3.0) and uses it to: <!-- src: /Makefile --> <!-- src: /desktop/gui/src/settings.rs#about -->

- show map and play data on the pad's screen (title, stars, accuracy, PP, combo, UR, hit counts and more; see [Customizing the screen layout](customize-screen-layout.md));
- switch the pad to its **Playing** screen during a map; <!-- src: /firmware/main/ui/ui_port.c#pad_timer_cb -->
- tell the touchscreen when you are in a map, so a resting finger is Quick Retry there and swipes work outside it (see [Touchscreen and swipes](touchscreen-and-swipes.md)). <!-- src: /firmware/main/input/touch_retry.c#touch_mode_now -->

Your keys never depend on tosu: the pad types the same with or without it. <!-- src: /AGENTS.md#1. Product rules -->

## What you need

- The OPad desktop app installed, with the pad connected.
- osu! (stable or lazer) running.

There is nothing to start by hand: the daemon runs tosu for you.

## How it works

1. **The daemon starts tosu.** If nothing is listening on tosu's port (`127.0.0.1:24050`) yet, the daemon starts tosu, and restarts it if it exits (after 5 seconds, doubling up to 60). It turns off tosu's own self-updater and its habit of opening a browser dashboard. If you already run a tosu yourself, the daemon uses that one. <!-- src: /desktop/crates/opad-tosu/src/lib.rs#spawn_tosu_supervisor --> <!-- src: /desktop/crates/opad-tosu/src/lib.rs#INITIAL_BACKOFF -->
2. **The daemon reads tosu's data** from `ws://127.0.0.1:24050/websocket/v2`, and osu!'s own key counts from `/websocket/v2/precise`. <!-- src: /desktop/crates/opad-tosu/src/lib.rs#DEFAULT_TOSU_ENDPOINT --> <!-- src: /desktop/crates/opad-tosu/src/lib.rs#precise_endpoint -->
3. **The daemon sends the pad only what changed.** During a map it sends at the rate set in **Screen** → **SCREEN UPDATES DURING A MAP** (1–30 Hz, default 10 Hz), and outside a map twice a second. <!-- src: /desktop/daemon/src/telemetry.rs#flush_interval_playing --> <!-- src: /desktop/daemon/src/telemetry.rs#FLUSH_INTERVAL_IDLE --> <!-- src: /desktop/gui/src/screen.rs#display -->

The daemon looks for tosu in this order, and uses the first one it finds: <!-- src: /desktop/crates/opad-tosu/src/lib.rs#find_tosu_binary -->

1. the file in the `OPAD_TOSU_PATH` environment variable;
2. `~/.local/opt/tosu/tosu` (`tosu.exe` on Windows);
3. `tosu` on your `PATH`;
4. the bundled copy.

So a tosu you installed yourself always wins over the bundled one.

## Linux: let tosu read osu!

On Linux, the Yama setting `ptrace_scope` stops one program reading another's memory whenever it is above 0, so tosu needs permission. <!-- src: /desktop/crates/opad-tosu/src/lib.rs#ptrace_access -->

- **`.deb`, `.rpm` and the Arch release package** give the bundled tosu the `cap_sys_ptrace` capability when they install. Nothing to do. <!-- src: /packaging/linux/deb/postinst --> <!-- src: /packaging/linux/arch/opad-bin.install -->
- **The AppImage and the Arch package built from source** cannot carry that capability. There you lower `ptrace_scope` to 0 instead. <!-- src: /packaging/linux/arch/PKGBUILD -->

To see which case applies to you, run:

```bash
opadctl doctor
```

It prints your `ptrace_scope` and the exact command that fixes it. [Troubleshooting §2](../troubleshooting.md#2-linux-tosu-cannot-read-osu-yama-ptrace_scope) explains both fixes. <!-- src: /desktop/cli/src/main.rs#run_doctor -->

## Windows

tosu works with no setup. If live data never shows up, check that your antivirus has not quarantined `%LOCALAPPDATA%\Programs\opad\tosu\tosu.exe`; see [Troubleshooting §4](../troubleshooting.md#4-windows-smartscreen-and-antivirus). <!-- src: /packaging/windows/installer.iss -->

## Use your own tosu

tosu is LGPL-3.0, so you may replace the bundled copy with your own build. Pick the way that suits you: <!-- src: /desktop/crates/opad-tosu/src/lib.rs#find_tosu_binary -->

- **Run your own tosu first.** When something already listens on port 24050, the daemon starts none and reads from yours. <!-- src: /desktop/crates/opad-tosu/src/lib.rs#spawn_tosu_supervisor -->
- **Install it in `~/.local/opt/tosu/` or put it on your `PATH`.** The daemon prefers it over the bundled copy.
- **Set `OPAD_TOSU_PATH`** to your tosu executable in the daemon's environment.
- **In the app:** **Settings** → **osu! connection** → **Choose another tosu…** (and **Use the one that comes with OPad** to go back). The app remembers the file and uses it when it starts tosu or the daemon itself. A daemon that starts at login (systemd, or the Windows Run entry) does not see this choice; use one of the ways above for it. <!-- src: /desktop/gui/src/settings.rs#osu_connection --> <!-- src: /desktop/gui/src/main.rs#App --> <!-- src: /desktop/gui/src/platform_linux.rs#start_daemon -->

## Check that it works

1. Run `opadctl status` and look for `tosu (osu!):      Active`. <!-- src: /desktop/cli/src/main.rs#run -->
2. Or open **Settings** → **osu! connection** in the app: it should say `Connected`, and **In use** says which tosu is running. <!-- src: /desktop/gui/src/settings.rs#osu_connection -->
3. Start a map in osu!. The pad switches to its Playing screen with live data.

## If it did not work

- **Linux: tosu is connected, but the pad shows no map data:** tosu cannot read osu!'s memory. Run `opadctl doctor` and follow [Troubleshooting §2](../troubleshooting.md#2-linux-tosu-cannot-read-osu-yama-ptrace_scope).
- **tosu does not start, or keeps restarting:** read its log with `opadctl logs --source tosu`, or open `tosu.log` (in `~/.local/state/opad/` on Linux, `%APPDATA%\opad\` on Windows). <!-- src: /desktop/cli/src/main.rs#LogsArgs --> <!-- src: /desktop/crates/opad-model/src/paths.rs#tosu_log_path -->
