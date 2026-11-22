---
title: Counters and Ownership Model
description: How OPad tracks monotonic 64-bit keypress counters, pairs pads to desktop installations, resolves ownership conflicts, and preserves taps during sync.
sidebar:
  order: 5
---

OPad records lifetime keypress counts on both the pad (in NVS flash) and the host PC (in SQLite `opad.db`). To keep counts accurate across disconnects, multiple PCs, and hardware replacements, OPad combines monotonic 64-bit counters, generation numbers, and an explicit installation-ownership model.

## Monotonic Keypress Counters

Lifetime counts are maintained independently for Key 1, Key 2, and (on 3-key V3 modules) Key 3 in [/firmware/main/counters/counters.c](/firmware/main/counters/counters.c):

- **64-bit counters**: `lifetime_key1`, `lifetime_key2`, and `lifetime_key3` are `uint64_t`. Within the same `counter_generation`, counts only move forward ([/firmware/main/counters/counter_sync_rules.c](/firmware/main/counters/counter_sync_rules.c)).
- **Lockless RAM increment**: Accepted key presses on Core 0 increment RAM counters immediately without touching flash.
- **Lazy NVS checkpointing**: To protect latency and limit flash wear, `counters_checkpoint_due` in [/firmware/main/counters/counter_sync_rules.c](/firmware/main/counters/counter_sync_rules.c) blocks NVS writes during `PLAYING` and `COOLDOWN`, and only checkpoints in `IDLE` after a quiescence period (or on USB suspend / explicit sync / pre-restart).

## Pad Ownership & Installation Identity

Because a pad may be moved between computers (home PC, laptop, LAN setup), OPad pairs pads to desktop installations rather than silently overwriting counts ([/desktop/daemon/src/identity.rs](/desktop/daemon/src/identity.rs)):

- **Installation ID (`install.id`)**: Generated as a UUIDv4 on first daemon startup and stored in SQLite `app_state`. It identifies the desktop installation, not the machine: uninstalling OPad removes `opad.db` and `install.id`, so reinstalling creates a fresh identity.
- **On-pad `owner_id`**: The 16-byte binary UUID (`OWNER_ID_LEN = 16`) is written to the pad via `ClaimOwnership` and persisted in NVS `device_config_data_t`.

### Ownership Resolution at Connect

When a pad connects, it reports its 16-byte `owner_id` in `HelloAck` ([/desktop/crates/opad-device/src/lib.rs](/desktop/crates/opad-device/src/lib.rs), [/desktop/daemon/src/ipc_handlers.rs](/desktop/daemon/src/ipc_handlers.rs)):

1. **Unclaimed pad** (`owner_id` all-zero/empty): The daemon automatically claims the pad and saves its 16-byte `install.id` to the pad's NVS (in `IDLE`).
2. **Matching owner** (`owner_id == this install`): Normal bidirectional counter reconciliation proceeds automatically.
3. **Foreign owner** (`owner_id != this install`):
   - **Pad is ahead or equal on both keys**: `take_over_if_pad_is_ahead` in [/desktop/daemon/src/ipc_handlers.rs](/desktop/daemon/src/ipc_handlers.rs) automatically claims the pad keeping the pad's counters, so moving your pad to a new PC or reinstalling keeps every press with zero prompts.
   - **PC is ahead on at least one key**: The daemon sets `pending_takeover` (`TakeoverPrompt`) and `foreign_pad = true`, pausing counter sync and prompting the user in `opad-gui` (via `ResolveTakeover`):
     - Take over keeping the pad's counters (`take_over: true, keep_device_counters: true`).
     - Take over pushing this PC's counters (`take_over: true, keep_device_counters: false`).
     - Leave the pad alone (`take_over: false`): the pad remains an unmanaged 1000 Hz USB keyboard while `foreign_pad` stays set.

## Generation Sequences & Reconciliation

Counter reconciliation in [/desktop/crates/opad-storage/src/lib.rs](/desktop/crates/opad-storage/src/lib.rs) (`reconcile_counters`) uses `counter_generation` to distinguish normal play from intentional resets or backup imports:

```rust
if pc.counter_generation > esp.counter_generation {
    pc.clone()
} else if esp.counter_generation > pc.counter_generation {
    esp.clone()
} else {
    // Same generation: component-wise max(pc, esp) for Key 1 and Key 2
}
```

1. **Higher generation wins**: Resetting counters (`opadctl reset --yes` or **Device** -> **Reset lifetime counters**) or importing a backup increments `counter_generation`, allowing lower or zero counts to legitimately supersede the older generation.
2. **Same generation (superset max)**: `lifetime_key1 = max(pc.k1, esp.k1)` and `lifetime_key2 = max(pc.k2, esp.k2)`, automatically merging offline keystrokes made while disconnected from the PC.
3. **Delta preservation on the pad**: In [/firmware/main/counters/counters.c](/firmware/main/counters/counters.c) (`counters_sync_from_host`), applying a host sync does not blindly clobber RAM counters with an absolute value. `rebase_lifetime_presses` (and `key_event_add_lifetime_key3(*k3 - current.lifetime_key3)`) applies the difference relative to the snapshot taken before validation so any key tapped while the sync was in flight is preserved.
