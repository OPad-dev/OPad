//! Version comparison (§U-2).
//!
//! "Different" is not good enough: a manifest that has been rolled back, or a
//! user running a locally built newer binary, must not trigger a downgrade
//! presented as an update. Only strictly newer counts.
//!
//! Dotted numeric parts compare numerically, and a release beats the
//! pre-release of the same numbers — `1.0.0` is newer than `1.0.0-rc`, which
//! is the exact case this project is in right now (§0, "Version").

/// True when `available` is strictly newer than `installed`
pub fn is_newer(available: &str, installed: &str) -> bool {
    ordering(available, installed) == std::cmp::Ordering::Greater
}

fn ordering(a: &str, b: &str) -> std::cmp::Ordering {
    use std::cmp::Ordering;

    let (a_core, a_pre) = split(a);
    let (b_core, b_pre) = split(b);

    let len = a_core.len().max(b_core.len());
    for i in 0..len {
        let x = a_core.get(i).copied().unwrap_or(0);
        let y = b_core.get(i).copied().unwrap_or(0);
        match x.cmp(&y) {
            Ordering::Equal => {}
            other => return other,
        }
    }

    match (a_pre, b_pre) {
        (None, None) => Ordering::Equal,
        // A release outranks any pre-release of the same version
        (None, Some(_)) => Ordering::Greater,
        (Some(_), None) => Ordering::Less,
        (Some(x), Some(y)) => x.cmp(&y),
    }
}

/// `"1.2.3-rc.1"` → `([1, 2, 3], Some("rc.1"))`. An unparseable part counts as
/// 0 rather than making the whole comparison meaningless.
fn split(v: &str) -> (Vec<u64>, Option<String>) {
    let v = v.trim().trim_start_matches('v');
    let (core, pre) = match v.split_once(['-', '+']) {
        Some((core, pre)) => (core, Some(pre.to_string())),
        None => (v, None),
    };
    let parts = core
        .split('.')
        .map(|p| p.parse::<u64>().unwrap_or(0))
        .collect();
    (parts, pre)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newer_versions_are_recognised() {
        assert!(is_newer("1.0.1", "1.0.0"));
        assert!(is_newer("1.1.0", "1.0.9"));
        assert!(is_newer("2.0.0", "1.99.99"));
        assert!(is_newer("v4.2.0", "4.1.0"), "a leading v is cosmetic");
    }

    #[test]
    fn equal_and_older_versions_are_not_updates() {
        assert!(!is_newer("1.0.0", "1.0.0"));
        // Missing trailing parts are zeros: 1.0 and 1.0.0 are one version
        assert!(!is_newer("1.0.0", "1.0"));
        assert!(!is_newer("1.0", "1.0.0"));
        assert!(!is_newer("1.0.0", "1.0.0.0"));
        assert!(!is_newer("1.0.0", "1.0.1"));
        // A rolled-back manifest must not push a downgrade as an update
        assert!(!is_newer("0.9.0", "1.0.0"));
    }

    #[test]
    fn a_release_beats_its_own_release_candidate() {
        // The state this project is in: 1.0.0-rc until W4 passes (§0).
        assert!(is_newer("1.0.0", "1.0.0-rc"));
        assert!(!is_newer("1.0.0-rc", "1.0.0"));
        assert!(is_newer("1.0.0-rc2", "1.0.0-rc1"));
        assert!(is_newer("1.0.1-rc", "1.0.0"));
    }

    #[test]
    fn nonsense_does_not_become_an_update() {
        assert!(!is_newer("", "1.0.0"));
        assert!(!is_newer("not-a-version", "1.0.0"));
        assert!(!is_newer("1.0.0", "1.0.0"));
    }
}
