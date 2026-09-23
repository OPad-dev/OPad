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
        (Some(x), Some(y)) => pre_release_ordering(x, y),
    }
}

/// `"1.2.3-rc.1+build.5"` → `([1, 2, 3], Some("rc.1"))`. Build metadata after
/// `+` never affects ordering. An unparseable part counts as 0 rather than
/// making the whole comparison meaningless.
fn split(v: &str) -> (Vec<u64>, Option<&str>) {
    let v = v.trim().trim_start_matches('v');
    let v = v.split_once('+').map_or(v, |(v, _build)| v);
    let (core, pre) = match v.split_once('-') {
        Some((core, pre)) => (core, Some(pre)),
        None => (v, None),
    };
    let parts = core
        .split('.')
        .map(|p| p.parse::<u64>().unwrap_or(0))
        .collect();
    (parts, pre)
}

/// Semver pre-release precedence: dot-separated identifiers left to right,
/// numeric ones numerically and below alphanumeric ones, and more identifiers
/// win when all shared ones are equal (`rc.1 < rc.1.1`).
fn pre_release_ordering(a: &str, b: &str) -> std::cmp::Ordering {
    let mut a_ids = a.split('.');
    let mut b_ids = b.split('.');
    loop {
        match (a_ids.next(), b_ids.next()) {
            (None, None) => return std::cmp::Ordering::Equal,
            (None, Some(_)) => return std::cmp::Ordering::Less,
            (Some(_), None) => return std::cmp::Ordering::Greater,
            (Some(x), Some(y)) => match identifier_ordering(x, y) {
                std::cmp::Ordering::Equal => {}
                other => return other,
            },
        }
    }
}

fn identifier_ordering(a: &str, b: &str) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    match (numeric(a), numeric(b)) {
        (Some(x), Some(y)) => x.cmp(&y),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => alphanumeric_ordering(a, b),
    }
}

fn numeric(id: &str) -> Option<u64> {
    if id.is_empty() || !id.bytes().all(|c| c.is_ascii_digit()) {
        return None;
    }
    id.parse().ok()
}

/// Lexical, except that runs of digits compare as numbers, so a dotless
/// `rc10` still follows `rc9` (strict semver would put it first).
fn alphanumeric_ordering(a: &str, b: &str) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    let (mut a, mut b) = (a, b);
    loop {
        let (x, a_rest) = next_run(a);
        let (y, b_rest) = next_run(b);
        let order = match (x, y) {
            ("", "") => return Ordering::Equal,
            ("", _) => Ordering::Less,
            (_, "") => Ordering::Greater,
            (x, y) => match (numeric(x), numeric(y)) {
                (Some(m), Some(n)) => m.cmp(&n).then_with(|| x.cmp(y)),
                _ => x.cmp(y),
            },
        };
        if order != Ordering::Equal {
            return order;
        }
        (a, b) = (a_rest, b_rest);
    }
}

/// Splits off the leading run of digits or of non-digits
fn next_run(s: &str) -> (&str, &str) {
    let digits = s.starts_with(|c: char| c.is_ascii_digit());
    let end = s
        .find(|c: char| c.is_ascii_digit() != digits)
        .unwrap_or(s.len());
    s.split_at(end)
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
    fn pre_release_numbers_compare_as_numbers() {
        assert!(is_newer("1.0.0-rc10", "1.0.0-rc9"));
        assert!(!is_newer("1.0.0-rc9", "1.0.0-rc10"));
        assert!(is_newer("1.0.0-rc.10", "1.0.0-rc.9"));
        assert!(!is_newer("1.0.0-rc.9", "1.0.0-rc.10"));
        // Semver: numeric identifiers sort below alphanumeric ones, and a
        // longer identifier list wins when the shared part is equal
        assert!(is_newer("1.0.0-alpha", "1.0.0-1"));
        assert!(is_newer("1.0.0-rc.1.1", "1.0.0-rc.1"));
        assert!(is_newer("1.0.0-beta", "1.0.0-alpha.9"));
        assert!(is_newer("1.0.0", "1.0.0-rc.10"));
    }

    #[test]
    fn build_metadata_does_not_affect_ordering() {
        assert!(!is_newer("1.0.0+build", "1.0.0"));
        assert!(!is_newer("1.0.0", "1.0.0+build"));
        assert!(!is_newer("1.0.0+build.2", "1.0.0+build.1"));
        assert!(is_newer("1.0.0+build", "1.0.0-rc"));
        assert!(is_newer("1.0.0-rc.2+b1", "1.0.0-rc.1+b9"));
    }

    #[test]
    fn nonsense_does_not_become_an_update() {
        assert!(!is_newer("", "1.0.0"));
        assert!(!is_newer("not-a-version", "1.0.0"));
        assert!(!is_newer("1.0.0", "1.0.0"));
    }
}
