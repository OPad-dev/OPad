---
title: Firmware Counters Subsystem
description: Implementation reference for monotonic keypress tracking, atomic delta application, generation validation, and lazy flash checkpointing.
---

The counters subsystem tracks cumulative lifetime switch actuations across Key 1, Key 2, and Key 3. It enforces strict monotonic progression, prevents flash wear through lazy quiescence checkpointing, and ensures that keypresses tapped during USB synchronization are never dropped. <!-- src: /firmware/main/counters/counters.c -->

## Storage & RAM State (`counters.c`)

- **RAM Variables**: The live counters are maintained in fast atomic 64-bit unsigned integers on Core 0: <!-- src: /firmware/main/counters/counters.c -->
  - `s_generation`: Counter generation number (starts at 1).
  - `s_last_saved_key1`, `s_last_saved_key2`, `s_last_saved_key3`: Checkpointed baseline values.
- **Serialization Lock (`s_lock`)**: A FreeRTOS binary semaphore serializes checkpoint, synchronization, and reset operations between the Core 1 runtime supervisor and the protocol task. Core 0 key ISRs never take this lock. <!-- src: /firmware/main/counters/counters.c -->

## Monotonic Sync Validation (`counter_sync_rules.c`)

When `opad-daemon` transmits a `CounterSyncRequest`, the firmware evaluates the incoming numbers via `counters_validate_sync_acceptance`: <!-- src: /firmware/main/counters/counter_sync_rules.c#counters_validate_sync_acceptance -->

```c
bool counters_validate_sync_acceptance(
    uint32_t current_gen, uint64_t current_k1, uint64_t current_k2,
    uint32_t host_gen, uint64_t host_k1, uint64_t host_k2,
    bool force, char *err_msg, size_t err_msg_len)
```

1. **Stale Generation Rejection**: If `host_gen < current_gen`, the sync is rejected (`"stale generation"`). An outdated PC database can never downgrade the pad's active generation. <!-- src: /firmware/main/counters/counter_sync_rules.c#counters_validate_sync_acceptance -->
2. **Non-Monotonic Rollback Rejection**: If `host_gen == current_gen` and either `host_k1 < current_k1` or `host_k2 < current_k2`, the sync is rejected (`"non-monotonic"`). Counters cannot roll backward within the same generation. <!-- src: /firmware/main/counters/counter_sync_rules.c#counters_validate_sync_acceptance -->
3. **Generational Bumps**: If `host_gen > current_gen` (e.g. an intentional reset or backup import), the higher generation supersedes unconditionally.

## Atomic Delta Application

To prevent losing keypresses tapped by the player while a sync packet is traversing the USB cable: <!-- src: /firmware/main/counters/counters.c -->
- The pad does not overwrite its RAM counter with the host's value.
- Instead, it applies the difference:
  $$\Delta = \text{host\_target} - \text{snapshot\_at\_read}$$
  $$\text{RAM\_counter} \mathrel{+}= \Delta$$
- Any keystroke registered by the hardware ISR between packet reception and lock acquisition survives intact.

## Lazy Quiescence Checkpointing

Flash writes are minimized to protect SPI flash endurance: <!-- src: /firmware/main/counters/counter_sync_rules.c#counters_checkpoint_due -->
- Checkpoints are strictly blocked while the device is in `OPAD_STATE_PLAYING` or `OPAD_STATE_COOLDOWN`.
- While idle, `counters_checkpoint_due` checks whether keys have remained untouched for a quiescence period before committing dirty counters to NVS (`NVS_NS_COUNTERS`).
