//! When an update may be applied (§U-0.1).
//!
//! P1-3 forbids storage writes during PLAYING and COOLDOWN. Updates are the
//! same class of hazard, with more force: replacing a binary, restarting a
//! process or writing flash mid-map is exactly what that rule exists to
//! prevent. A pending update waits; it never interrupts.

use opad_model::RuntimeMode;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeferReason {
    /// A map is in progress
    Playing,
    /// The map ended but the cooldown has not expired; the counters have not
    /// been synced back yet, so this is still hands-off time
    Cooldown,
    /// The daemon is mid-sync
    Syncing,
    /// The user turned this updater off (§U-0.4)
    Disabled,
}

impl fmt::Display for DeferReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            DeferReason::Playing => "waiting until the map ends",
            DeferReason::Cooldown => "waiting for the cooldown to finish",
            DeferReason::Syncing => "waiting for the counter sync to finish",
            DeferReason::Disabled => "this updater is switched off",
        };
        f.write_str(s)
    }
}

/// The single question every updater asks before touching anything.
///
/// IDLE is the only state in which an update may be applied — the same rule
/// the NVS write guard uses, for the same reason.
pub fn may_update_now(mode: RuntimeMode, enabled: bool) -> Result<(), DeferReason> {
    if !enabled {
        return Err(DeferReason::Disabled);
    }
    match mode {
        RuntimeMode::Idle => Ok(()),
        RuntimeMode::Playing => Err(DeferReason::Playing),
        RuntimeMode::Cooldown => Err(DeferReason::Cooldown),
        RuntimeMode::Sync => Err(DeferReason::Syncing),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_idle_allows_an_update() {
        assert!(may_update_now(RuntimeMode::Idle, true).is_ok());
        for mode in [
            RuntimeMode::Playing,
            RuntimeMode::Cooldown,
            RuntimeMode::Sync,
        ] {
            assert!(
                may_update_now(mode, true).is_err(),
                "{mode:?} must defer updates"
            );
        }
    }

    #[test]
    fn a_disabled_updater_defers_even_when_idle() {
        assert_eq!(
            may_update_now(RuntimeMode::Idle, false),
            Err(DeferReason::Disabled)
        );
    }

    #[test]
    fn every_reason_reads_as_a_sentence_for_the_gui() {
        for r in [
            DeferReason::Playing,
            DeferReason::Cooldown,
            DeferReason::Syncing,
            DeferReason::Disabled,
        ] {
            assert!(!r.to_string().is_empty());
        }
    }
}
