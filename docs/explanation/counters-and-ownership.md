---
title: Counters and Ownership Model
description: Deep explanation of monotonic lifetime keypress counters, pad ownership pairing, generation sequences, and flash wear mitigation.
---

OPad tracks the player's lifetime keypress counts across physical switches and gaming sessions. These counters serve as verifiable milestones for rhythm game players. To protect counter integrity across different computers, reinstallations, and hardware replacements, OPad couples monotonic counter tracking with an explicit ownership pairing model. <!-- src: /desktop/daemon/src/identity.rs -->

## Monotonic Keypress Counters

Lifetime counts are tracked independently for Key 1, Key 2, and optional Key 3. <!-- src: /firmware/main/counters/counters.c -->

- **64-Bit Monotonic Integers**: Keypress totals (`lifetime_key1`, `lifetime_key2`, `lifetime_key3`) are unsigned 64-bit integers. Counters only advance forward; they can never roll back within the same counter generation. <!-- src: /firmware/main/counters/counter_sync_rules.c#counters_validate_sync_acceptance -->
- **RAM Fast Path**: In the firmware key interrupt service routines (Core 0), presses increment atomic RAM variables with zero lock contention. Counters in RAM advance continuously even during active gameplay. <!-- src: /firmware/main/counters/counters.c -->
- **Flash Checkpointing**: Committing counters to non-volatile SPI flash (NVS) wears flash memory and stalls CPU execution. The firmware mitigates flash wear using lazy checkpointing (`counters_checkpoint_due` in `firmware/main/counters/counter_sync_rules.c`): <!-- src: /firmware/main/counters/counter_sync_rules.c#counters_checkpoint_due -->
  - Checkpoints are strictly forbidden during active gameplay (`PLAYING` / `COOLDOWN`).
  - While idle, checkpoints occur only after keys have remained untouched for a quiescence period, minimizing write cycles.

## Pad Ownership & Identity Model

A pad is frequently plugged into multiple computers (e.g., home PC, tournament laptop, LAN setup). To avoid silently overwriting counter records between different systems, OPad implements a pairing model: <!-- src: /desktop/daemon/src/identity.rs -->

### Installation Identity (`install.id`)
- Every desktop installation generates a persistent UUIDv4 on its initial run, stored in SQLite under `install.id`. <!-- src: /desktop/daemon/src/identity.rs#INSTALL_ID_KEY -->
- The identity is **per-installation, not per-machine**. If a user uninstalls the software with the "delete everything" option, the identity and its SQLite database are deleted. Reinstalling generates a fresh identity. <!-- src: /desktop/daemon/src/identity.rs -->
- The 16-byte binary owner identifier is sent to the pad via `ClaimOwnership`. The pad records this 16-byte owner ID in NVS. <!-- src: /desktop/daemon/src/identity.rs#OWNER_ID_LEN -->

### Ownership States & Conflict Resolution
When a pad is connected to `opad-daemon`, it reports its stored `owner_id` in `HelloAck`: <!-- src: /desktop/crates/opad-device/src/lib.rs#Ownership -->
1. **Unclaimed Pad (`owner_id` is empty)**: The daemon automatically claims the pad, writing its 16-byte install identity to NVS.
2. **Matching Owner (`owner_id == this install`)**: The pad belongs to this PC. Normal bidirectional counter synchronization proceeds automatically.
3. **Mismatched Owner (`owner_id != this install`)**: The pad was previously paired with another PC.
   - If the pad's lifetime counters are greater than or equal to this PC's database records on both keys, the pad is adopted automatically (`take_over_if_pad_is_ahead`), ensuring no progress is lost when moving to a new computer. <!-- src: /desktop/daemon/src/ipc_handlers.rs#take_over_if_pad_is_ahead -->
   - If this PC has higher counts than the pad, an ownership conflict is flagged, prompting the user (`TakeoverPrompt`) in the GUI or `opadctl status`:
     - **Take over and adopt pad counters**: Updates local database with the pad's numbers.
     - **Take over and restore PC counters**: Pushes this PC's higher counter database onto the pad.
     - **Leave alone**: The pad continues operating as an unmanaged 1000 Hz HID keyboard; no sync occurs.

## Generation Sequences & Counter Reconciliation

To handle intentional counter resets or backup restorations without violating monotonicity, counters are governed by a **generation sequence number** (`counter_generation`): <!-- src: /desktop/crates/opad-storage/src/lib.rs#reconcile_counters -->

```rust
pub fn reconcile_counters(pc: &CounterState, esp: &CounterState) -> CounterState {
    if pc.counter_generation > esp.counter_generation {
        pc.clone()
    } else if esp.counter_generation > pc.counter_generation {
        esp.clone()
    } else {
        CounterState {
            lifetime_key1: pc.lifetime_key1.max(esp.lifetime_key1),
            lifetime_key2: pc.lifetime_key2.max(esp.lifetime_key2),
            ...
        }
    }
}
```

### Reconciliation Rules
1. **Newer Generation Wins**: If a user performs an authorized reset (`opadctl reset --confirm`) or restores a backup, `counter_generation` is incremented. Any counter state with a higher generation number unconditionally supersedes older generations, allowing counters to reset cleanly to zero. <!-- src: /desktop/crates/opad-storage/src/lib.rs#reconcile_counters -->
2. **Identical Generation (Superset Rule)**: If both PC and pad share the same generation, the reconciliation algorithm calculates the component-wise maximum:
   $$\text{lifetime\_key1} = \max(\text{pc.k1}, \text{esp.k1})$$
   $$\text{lifetime\_key2} = \max(\text{pc.k2}, \text{esp.k2})$$
   Any keystrokes executed on the pad while disconnected from the PC are seamlessly merged into the desktop database.
3. **Delta Preservation in Firmware**: When the pad receives a counter synchronization command (`CounterSyncRequest`), it does not overwrite its RAM counters with the absolute incoming number. Instead, it computes the delta and applies it:
   $$\text{RAM\_counter} \mathrel{+}= (\text{target\_count} - \text{snapshot\_count})$$
   Any physical keystrokes tapped by the player while the sync frame was traversing the USB serial bus survive intact. <!-- src: /firmware/main/counters/counters.c -->
