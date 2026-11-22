---
title: Using with tosu
description: How to configure and run the bundled tosu memory scanner for live osu! beatmap telemetry, UR streaming, and permission requirements.
---

OPad bundles `tosu` (v4.26.2), a high-performance open-source memory reader that inspects the running `osu!.exe` process to stream live gameplay statistics to the keypad's LCD display. <!-- src: /desktop/crates/opad-tosu/src/lib.rs -->

## How tosu Integration Works

1. `opad-daemon` launches and supervises the bundled `tosu` binary in the background (`bundled_tosu_binary`). <!-- src: /desktop/crates/opad-tosu/src/lib.rs -->
2. `tosu` exposes a local WebSocket server at `ws://127.0.0.1:24050/websocket/v2`. <!-- src: /desktop/crates/opad-tosu/src/lib.rs#DEFAULT_TOSU_ENDPOINT -->
3. The daemon connects to the WebSocket stream and receives real-time JSON packets detailing:
   - Song artist, title, difficulty name, and mapper.
   - Live accuracy, combo, miss count, current grade (SS, S, A, B, C, D).
   - Unstable Rate (UR) and live Keys Per Minute (KPM).
4. The daemon transforms telemetry diffs into `GameplayDisplayState` and `DataUpdate` envelopes, streaming them over USB CDC to the pad display at a rate clamped between 1 Hz and 30 Hz (`flush_interval_playing`). <!-- src: /desktop/daemon/src/telemetry.rs#flush_interval_playing -->

## Linux Permission Setup (ptrace)

On modern Linux distributions, the Yama security module restricts process memory inspection (`ptrace`) so that unprivileged processes cannot read the memory of unrelated programs. <!-- src: /packaging/linux/deb/postinst -->

If you installed OPad via `.deb`, `.rpm`, or AUR packages, file capabilities are configured automatically: <!-- src: /packaging/linux/deb/postinst -->
```bash
sudo setcap cap_sys_ptrace=eip /usr/lib/opad/tosu/tosu
```
This capability allows `tosu` to attach to `osu!.exe` under Wine or native Proton without requiring root privileges or disabling system-wide security controls.

### Troubleshooting ptrace on Custom Installs
If tosu shows `Waiting for osu!` even while the game is running, verify the capability:
```bash
getcap /usr/lib/opad/tosu/tosu
```
If empty, reapply:
```bash
sudo setcap cap_sys_ptrace=eip $(which tosu || echo /usr/lib/opad/tosu/tosu)
```

## Windows Setup

On Windows 10 and 11, `tosu` operates out of the box without special privileges.
- Launch `osu!`.
- Launch `opad-gui` or ensure `opad-daemon` is running.
- When you enter song select or begin playing a map, the pad's LCD immediately transitions from the `Idle` screen to the `Playing` screen, displaying live song metadata and gameplay meters.

## Checking tosu Status

You can inspect the live connection status in two ways:
1. **Via GUI**: In `opad-gui`, go to **Settings > About tosu**. The panel displays tosu process status, memory reading health, and active osu! client detection.
2. **Via CLI**:
   ```bash
   opadctl status
   ```
   The output reports whether `tosu` is running, connected to the WebSocket endpoint, and actively tracking gameplay.
