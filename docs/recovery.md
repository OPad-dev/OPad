# osu!pad Counter Reconciliation & Disaster Recovery

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

