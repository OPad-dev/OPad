//! Keeps the shared state's tap rate history (issue #2) in step with SQLite.

use parking_lot::Mutex;
use std::sync::Arc;
use tracing::warn;

use opad_storage::Storage;

use crate::runtime::DaemonState;

/// Re-reads the history for the configured period into the shared state. The
/// runtime notices the new value on its next tick and sends it to the pad.
///
/// The two locks are never held together: the state is read, then the
/// storage, then the state written, so this cannot deadlock against a handler
/// that takes them in the other order.
pub fn refresh_history(storage: &Arc<Mutex<Option<Storage>>>, state: &Arc<Mutex<DaemonState>>) {
    let days = state.lock().tap.snapshot.period_days;
    let history = {
        let guard = storage.lock();
        let Some(s) = guard.as_ref() else {
            return;
        };
        match s.tap_history(days, chrono::Utc::now()) {
            Ok(h) => h,
            Err(e) => {
                warn!("Cannot read the tap rate history: {}", e);
                return;
            }
        }
    };
    let mut st = state.lock();
    // The period changed while the storage was being read: that change
    // brought its own history
    if st.tap.snapshot.period_days == days {
        st.tap.snapshot.history = Some(history);
    }
}
