# osu!pad Counter Reconciliation & Disaster Recovery

## 1. Storage Model (§12, §13)
Both the ESP32-S3 and the host PC maintain lifetime press counters for both physical keys.
- **ESP**: Counters increment in RAM in the ISR/input critical path. Zero flash writes during active keypresses. Checkpointed to NVS during `SYNC/IDLE`.
- **Host**: Stored in SQLite (`device_state` table). Zero disk writes during `PLAYING` and `COOLDOWN`.

## 2. Counter Generation & Reconciliation Rules
Each counter record contains:
- `device_id`: Hardware unique ID.
- `counter_generation`: Monotonically incrementing integer changed whenever the counter is intentionally reset or restored from backup.
- `lifetime_key1`: Total lifetime presses for Key 1.
- `lifetime_key2`: Total lifetime presses for Key 2.

### Rules:
1. **Generational Precedence**: If `generation_A > generation_B`, state A wins completely. This prevents an old backup from overwriting a newer intentional reset.
2. **Same Generation Merging**: If generations match, the maximum valid value for each physical key is retained (`max(pc.k1, esp.k1)` and `max(pc.k2, esp.k2)`).
3. **Power-loss Policy (§12.1)**: Unsaved presses during an active map before cooldown ends may be lost if sudden power loss occurs. This is an explicit architectural trade-off to ensure zero-latency key processing without blocking on flash writes.

## 3. Disaster Scenarios
- **Scenario A: PC Database Lost, ESP Intact**:
  On startup, the daemon connects to the ESP, receives `HelloAck` containing current device lifetime counters, and populates a fresh SQLite database.
- **Scenario B: ESP Reflashed / Replaced, PC Intact**:
  The daemon detects the ESP has 0 presses, restores the recorded lifetime counts from SQLite, and transmits them to the ESP via `CounterSyncRequest`.
- **Scenario C: Manual Portable JSON Restore**:
  User runs `osupadctl import backup.json`. The daemon validates format version, increments `counter_generation`, updates SQLite, and pushes the new baseline to the ESP.
