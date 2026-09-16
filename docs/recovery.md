# osu!pad Counter Reconciliation, Recovery & Unbinding

## 1. Dual-Storage Model (§12, §13)

Both the ESP32-S3 and the host PC maintain lifetime press counters for both physical keys:
- **Device (ESP32-S3)**: Counters increment in RAM directly in the key processing critical path on Core 0. **Zero flash writes occur during keypresses.** Counters are checkpointed to NVS only during `IDLE` state after map cooldown has expired.
- **Host PC**: Stored in SQLite (`device_state` table). **Zero disk writes occur during `PLAYING` and `COOLDOWN`.** The daemon maintains the latest counts in memory and commits to SQLite upon transitioning to `SYNC/IDLE`.

---

## 2. Counter Generation & Monotonic Reconciliation

Each counter record contains:
- `device_id`: Hardware unique ID derived from the ESP32-S3 MAC address (`OSUPAD-<MAC>`).
- `counter_generation`: Monotonically incrementing integer. Bumped whenever counters are intentionally reset or restored from backup.
- `lifetime_key1`: Total lifetime presses for Key 1.
- `lifetime_key2`: Total lifetime presses for Key 2.

### Reconciliation Rules (§13):
1. **Generational Precedence**: If `generation_A > generation_B`, state A wins unconditionally. This prevents an old backup or stale database from overwriting an intentional reset.
2. **Monotonic Merging (Same Generation)**: When generations match, the maximum count for each key is retained:
   $$\text{Key1} = \max(\text{pc.k1}, \text{esp.k1}), \quad \text{Key2} = \max(\text{pc.k2}, \text{esp.k2})$$
3. **Power-loss Policy (§12.1)**: If power is abruptly disconnected during an active map before cooldown expires, uncommitted presses since the last IDLE checkpoint may be lost. This is an intentional architectural trade-off to ensure zero-latency key processing without blocking on flash writes.

---

## 3. Connect-Time Reconciliation Matrix

When the daemon opens the USB CDC port and receives `HelloAck`:

| Condition | Action Taken |
|---|---|
| **Known Pad, Same Generation** | Daemon merges highest counts ($\max(\text{pc}, \text{esp})$) and synchronizes both device RAM and SQLite. |
| **Known Pad, Device Gen > Host Gen** | Device was restored or reset on another machine. Host updates its SQLite database to match device generation and counts. |
| **Known Pad, Host Gen > Device Gen** | Device was reflashed or wiped. Host sends `CounterSync` to restore the recorded lifetime counts to the device. |
| **New Pad, Blank State (`gen == 0`, `k1 == 0`, `k2 == 0`)** | If exactly one previous pad exists in SQLite, trigger the **Device Replacement Flow**. Otherwise, initialize new device at generation 1. |

---

## 4. Device Replacement Flow (§14 / P1-5)

When a player replaces their osu!pad or swaps hardware:
1. The daemon detects a connection from an unknown `device_id`, but finds exactly one existing device record in SQLite.
2. The daemon creates a pending replacement recommendation (`suggested_replacement: Some(old_device_id)`).
3. The GUI displays a non-blocking replacement banner:
   > *"New pad detected (`OSUPAD-NEW`). Transfer lifetime presses from your previous pad (`OSUPAD-OLD`)?"*
4. **If User Accepts (`InheritDeviceCounters`)**:
   - Daemon increments `counter_generation` by 1.
   - Migrates old lifetime counts to the new `device_id` in SQLite.
   - Transmits `CounterSync(force_restore = true)` to the new pad, programming the previous lifetime stats into hardware.
5. **If User Rejects / Dismisses**:
   - New pad begins life cleanly with its own `device_id`, generation 1, and 0 presses.
   - Old device record remains preserved in SQLite as historical data.

---

## 5. Backup, Restore, and Reset Actions

- **Reset Counters**:
  - Resets Key 1 and Key 2 lifetime counts to 0.
  - Automatically increments `counter_generation` so all peers recognize the reset as the newest authority.
  - Synchronizes both host SQLite and device NVS.
- **Export Backup**:
  - Exports a portable JSON document conforming to the §21 specification containing:
    - Format version (`format_version: 1`).
    - Device hardware metadata (`device_id`, `board_profile`, `counter_generation`).
    - Statistics (`lifetime_key1`, `lifetime_key2`).
    - Configuration (`key1`, `key2`, `debounce_us`, `brightness`, `display_sleep_seconds`, `gameplay_display_hz`).
- **Import Backup (`PreviewImport` & `ImportBackup`)**:
  - Two-stage safety gate:
    1. `PreviewImport`: Validates JSON schema, key bindings, and counter values. Returns a side-by-side diff between current device state and backup state.
    2. `ImportBackup`: Requires explicit confirmation (`confirm: true`). Preserves local `tosu_endpoint`, increments generation, applies config to device, and commits new counter state.

---

## 6. Deferred Operations During Gameplay

During `PLAYING` and `COOLDOWN` states:
- All writes to SQLite and ESP32-S3 flash are strictly blocked.
- If a user changes configuration, resets layout, or triggers sync while playing, the daemon queues the action in RAM.
- When tosu reports `IDLE` (or after 5 seconds of cooldown post-map), the runtime supervisor flushes all pending operations safely to NVS and disk.

---

## 7. Reflashing & Unbinding a Pad (§W3-4)

This is the **only** way to unbind a pad. There is no unpair button in the app
and no factory-reset gesture on the pad itself — deliberately, because an
ownership claim that could be cleared over the wire would not protect counter
integrity at all (§W3-2 refuses an all-zero claim for the same reason).

If you arrived here from the takeover prompt: you do **not** need any of this to
use the pad. "Leave it alone" already keeps it working as a keyboard. This
section is for actually handing the pad over, or for putting a pad that will not
boot back into a known state.

### 7.1 What it costs

The owner record (`owner_id`) lives in the pad's NVS partition, alongside the
lifetime counters. Nothing can erase one and keep the other.

> **`espflash erase-flash` erases the ESP-side lifetime counters.**
> Key 1 and Key 2 both go back to zero on the pad. So do the device
> configuration and the stored layouts.

It erases the whole 16 MB — the app, the bootloader and the partition table as
well — so the pad stops being a keyboard until step 7.4 puts firmware back.

**It is not bricked, and it cannot be.** The ESP32-S3's first-stage bootloader
lives in mask ROM and no host command can erase it. A pad with completely blank
flash still enumerates as `303a:1001` and still accepts a flash.

### 7.2 Export first

Your host keeps its own copy of the counters in SQLite, but export the portable
JSON anyway — it is the only copy that survives reinstalling the app:

```bash
osupadctl export ~/osupad-backup.json     # or: the app → Device → Export backup
```

Restore it afterwards with `osupadctl import ~/osupad-backup.json`, which bumps
the counter generation and writes the counts back to the pad (§5).

**One thing to understand before you erase.** Reflashing makes the pad
unclaimed, and an unclaimed pad is claimed *silently* by the first host it
connects to (§W3-3). If that host is the one you just erased it from, it will
also restore the counters it remembers, because its generation is now higher
than the pad's blank one (§3). That is exactly right when you are fixing a
broken install, and exactly wrong when you are giving the pad away — in that
case plug it into its new owner's machine first, so their install is the one
that claims it.

### 7.3 Put the pad in download mode and free the port

```bash
osupadctl bootloader
```

That does both halves: it asks `osupad-daemon` to release the serial port, then
reboots the pad into the ROM download bootloader hands-free and prints the port
it came back on. It works with no daemon running at all.

**Windows: the port must be free, not merely idle.** Windows serial handles are
exclusive (`CreateFileW` with no sharing), so anything still holding the port
fails the flash outright rather than just slowing it down — unlike Linux, where
a second reader is only impolite. `osupadctl bootloader` handles the daemon;
close any serial monitor, PuTTY or Arduino IDE window yourself. If a flash stops
with a sharing violation or "access denied", that is what it means.

Find the port with `espflash list-ports`, or Device Manager → Ports (COM & LPT).
Ports above COM9 are fine; `osupadctl` and `espflash` both apply the `\\.\`
prefix that raw COM names need.

**The USB-download reboot quirk.** The pad reboots itself into download mode
over its USB-Serial-JTAG interface, and that interface keeps the USB address it
had at power-on. The host has to see a real detach before the re-attach, so the
firmware forces one; if it does not land you get `error -71` and the app simply
comes back. `osupadctl bootloader` tries three different triggers for this
reason. If all three fail:

1. Unplug the pad, wait two seconds, plug it back in, and try again.
2. Failing that, do it by hand: hold **BOOT**, tap **RESET**, release **BOOT**.
   The pad enumerates as `303a:1001` with no firmware involvement at all, which
   is the path that always works.

### 7.4 Erase, then flash

With the pad sitting in download mode:

**Linux**

```bash
espflash erase-flash -p /dev/ttyACM0 --before no-reset --after no-reset-no-stub
osupadctl flash --full ~/Downloads/osupad-1.0.0/          # or firmware/build
```

**Windows (PowerShell or cmd)**

```powershell
espflash erase-flash -p COM3 --before no-reset --after no-reset-no-stub
osupadctl flash --full C:\Users\you\Downloads\osupad-1.0.0\
```

`--before no-reset` matters: the pad is already in download mode, and letting
espflash try its own DTR/RTS reset over USB-Serial-JTAG only risks dropping it
back into the app. Erasing 16 MB takes the best part of a minute. **Do not
unplug it while it runs.**

`--full` is required rather than optional here, because `erase-flash` took the
bootloader and the partition table with it. It writes all four images in order —
bootloader at `0x0`, partition table at `0x8000`, OTA data at `0xf000`, app at
`0x20000` (`ota_0`, §U-3a) — and then reboots the pad into the app. It accepts
either an unpacked release directory or an ESP-IDF `firmware/build` tree.

### 7.5 Confirm

```bash
osupadctl status
```

A pad that came back correctly reports `ESP32 Device: Connected` and a
`Running Slot` of `ota_0`.

The counters depend on which host you plugged it into. On a machine that has
never seen this pad, expect zero presses at generation 1. On the machine you
erased it from, expect the counts it remembers: its generation now outranks the
pad's blank one, so it restores them and silently re-claims the pad, and the
daemon log says both happened (§7.2).

If the pad does not come back as the app within 15 seconds, `osupadctl flash`
says so. Repeat **7.4's flash step only** — do not erase again. The pad is still
in download mode and re-running the flash is safe.

### 7.6 When flashing is interrupted

If power or the cable is lost part-way through writing the app, the app
partition is incomplete and the pad will not run as a keyboard until it is
flashed again. It is **not** bricked (§7.1). Put it back in download mode with
BOOT + RESET and re-run the flash step.

This failure mode is the reason firmware updates ask for explicit consent every
time (§U-3b), and the reason the two-slot layout exists (§U-3a): once OTA A/B
lands, a failed update rolls back to the slot that was working instead.
