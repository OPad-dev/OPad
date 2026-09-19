//! The install's own identity (§W3-1).
//!
//! A random UUIDv4, generated on first run and kept in SQLite. It is
//! **per-install, not per-machine**: reinstalling after the "delete
//! everything" uninstall (§W2-3) produces a new identity, and that is correct
//! and intended — the counters the old install owned went with it.
//!
//! This is what a pad records as its owner (§W3-2). Nothing about it is
//! cryptographic: it protects counter integrity, not exclusivity (§0).

use std::sync::{Arc, Mutex};

use opad_storage::Storage;
use tracing::{info, warn};

pub const INSTALL_ID_KEY: &str = "install.id";

/// 16 bytes, the length a pad stores in NVS (§W3-2)
pub const OWNER_ID_LEN: usize = 16;

/// Reads the install identity, generating and saving one the first time.
///
/// Returns `None` when storage is unavailable — the daemon runs degraded
/// rather than not at all (P2-12), and an install with no identity simply
/// claims nothing and prompts about nothing.
pub fn load_or_create(storage: &Arc<Mutex<Option<Storage>>>) -> Option<String> {
    let guard = storage.lock().ok()?;
    let storage = guard.as_ref()?;

    match storage.get_app_state(INSTALL_ID_KEY) {
        Ok(Some(id)) if parse_owner_id(&id).is_some() => Some(id),
        Ok(_) => {
            let id = uuid::Uuid::new_v4().to_string();
            match storage.set_app_state(INSTALL_ID_KEY, &id) {
                Ok(()) => {
                    info!("Generated this install's identity: {}", id);
                    Some(id)
                }
                Err(e) => {
                    // Returning it unsaved would hand out a different identity
                    // on the next start and re-prompt about every pad.
                    warn!("Could not save the install identity: {}", e);
                    None
                }
            }
        }
        Err(e) => {
            warn!("Could not read the install identity: {}", e);
            None
        }
    }
}

/// The 16 bytes a pad stores, from the string form kept in SQLite
pub fn parse_owner_id(install_id: &str) -> Option<[u8; OWNER_ID_LEN]> {
    uuid::Uuid::parse_str(install_id)
        .ok()
        .map(|u| *u.as_bytes())
}

/// True when a pad reports no owner. An absent field reads the same as an
/// all-zero one, so firmware that predates §W3-2 is treated as unclaimed
/// rather than as belonging to someone else.
pub fn is_unclaimed(owner_id: &[u8]) -> bool {
    owner_id.iter().all(|b| *b == 0)
}

/// Whether a pad's recorded owner is this install
pub fn owns(install_id: Option<&str>, owner_id: &[u8]) -> bool {
    match install_id.and_then(parse_owner_id) {
        Some(ours) => owner_id == ours,
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_identity_is_generated_once_and_then_reused() {
        let storage = Arc::new(Mutex::new(Some(Storage::open_in_memory().unwrap())));
        let first = load_or_create(&storage).expect("first run");
        let second = load_or_create(&storage).expect("second run");
        assert_eq!(first, second, "the identity must be stable across restarts");
        assert!(uuid::Uuid::parse_str(&first).is_ok());
    }

    #[test]
    fn two_installs_get_different_identities() {
        // Per-install, not per-machine: a reinstall is a new owner.
        let a = Arc::new(Mutex::new(Some(Storage::open_in_memory().unwrap())));
        let b = Arc::new(Mutex::new(Some(Storage::open_in_memory().unwrap())));
        assert_ne!(load_or_create(&a), load_or_create(&b));
    }

    #[test]
    fn a_daemon_with_no_storage_simply_has_no_identity() {
        let storage: Arc<Mutex<Option<Storage>>> = Arc::new(Mutex::new(None));
        assert_eq!(load_or_create(&storage), None);
    }

    #[test]
    fn a_corrupted_identity_is_replaced_rather_than_used() {
        let storage = Arc::new(Mutex::new(Some(Storage::open_in_memory().unwrap())));
        storage
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .set_app_state(INSTALL_ID_KEY, "not-a-uuid")
            .unwrap();
        let id = load_or_create(&storage).expect("regenerated");
        assert!(uuid::Uuid::parse_str(&id).is_ok());
    }

    #[test]
    fn an_identity_that_cannot_be_saved_is_not_handed_out() {
        // Otherwise every restart would invent a new owner and re-prompt about
        // a pad it already claimed.
        let storage = Arc::new(Mutex::new(Some(Storage::open_in_memory().unwrap())));
        storage
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .set_writes_allowed(false);
        assert_eq!(load_or_create(&storage), None);
    }

    #[test]
    fn an_absent_or_zero_owner_reads_as_unclaimed() {
        assert!(is_unclaimed(&[]));
        assert!(is_unclaimed(&[0u8; OWNER_ID_LEN]));
        assert!(!is_unclaimed(&[1u8; OWNER_ID_LEN]));
    }

    #[test]
    fn ownership_compares_the_full_sixteen_bytes() {
        let id = uuid::Uuid::new_v4().to_string();
        let bytes = parse_owner_id(&id).unwrap();
        assert!(owns(Some(&id), &bytes));

        let mut other = bytes;
        other[15] ^= 1;
        assert!(!owns(Some(&id), &other));

        // An install with no identity owns nothing, and claims nothing
        assert!(!owns(None, &bytes));
        assert!(!owns(Some(&id), &[]));
    }
}
