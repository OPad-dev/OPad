---
title: Firmware Counters Subsystem
description: Implementation reference for monotonic keypress counters, sync validation rules, atomic delta rebasing, and lazy NVS checkpointing.
---

The counters subsystem persists 64-bit lifetime press counts for Key 1, Key 2, and Key 3 in NVS (`NVS_NS_COUNTERS` / `"opad"`) while keeping all live increments lockless in RAM on Core 0 ([/firmware/main/counters/counters.c](/firmware/main/counters/counters.c), [/firmware/main/counters/counter_sync_rules.c](/firmware/main/counters/counter_sync_rules.c)).

## Storage & RAM State (`counters.c`)

- Live press totals live in atomic `uint64_t` counters inside [/firmware/main/input/key_event.c](/firmware/main/input/key_event.c) (`s_lifetime_presses[KEY_ID_COUNT]`).
- `counters.c` tracks `s_generation`, `s_last_saved_key1`, `s_last_saved_key2`, `s_last_saved_key3`, and `s_nvs_writes`, persisted under NVS keys `"generation"`, `"key1_cnt"`, `"key2_cnt"`, and `"key3_cnt"` (written only when Key 3 is non-zero or was previously saved).
- A FreeRTOS mutex/semaphore `s_lock` serializes NVS checkpoints and host syncs on Core 1; Core 0 ISRs and key tasks never take `s_lock`.

## Sync Validation (`counter_sync_rules.c`)

Incoming `CounterSyncRequest` values are checked by `counters_validate_sync_acceptance` in [/firmware/main/counters/counter_sync_rules.c](/firmware/main/counters/counter_sync_rules.c):

```c
bool counters_validate_sync_acceptance(
    uint32_t current_gen, uint64_t current_k1, uint64_t current_k2,
    uint32_t host_gen, uint64_t host_k1, uint64_t host_k2,
    bool force, char *err_msg, size_t err_msg_len);
```

Unless `force` is true:
1. `host_gen < current_gen` -> rejected (`"stale generation"`).
2. `host_gen == current_gen` and (`host_k1 < current_k1` or `host_k2 < current_k2`) -> rejected (`"non-monotonic"`).
3. `host_gen > current_gen` -> accepted unconditionally (supports authorized resets and backup restores).

`counters_sync_from_host` also refuses syncs when `runtime_get_state() != OPAD_STATE_IDLE` (`"not idle"`) or when NVS failed initialization (`"nvs unavailable"`).

## Atomic Delta Rebasing

To preserve taps pressed while a sync message is in flight, `counters_sync_from_host` in [/firmware/main/counters/counters.c](/firmware/main/counters/counters.c) takes a snapshot before validation and applies only the difference via `rebase_lifetime_presses` (and `key_event_add_lifetime_key3(*k3 - current.lifetime_key3)` modulo `2^64`):

```text
delta = target - snapshot
RAM_counter += delta
```

Any press accepted between `counters_get(&current)` and `rebase_lifetime_presses` remains counted.

## Lazy Quiescence Checkpointing

`checkpoint_locked` refuses writes whenever `runtime_get_state() != OPAD_STATE_IDLE` and skips writing when RAM matches `s_last_saved_key*` (unless `force`). `runtime_supervisor_task` calls `counters_checkpoint_due` ([/firmware/main/counters/counter_sync_rules.c](/firmware/main/counters/counter_sync_rules.c)) in `IDLE` to checkpoint only after a quiescence window, on USB suspend (`counters_request_checkpoint` from `usb_event_handler`), or before a clean restart.
